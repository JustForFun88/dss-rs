//! A-Diakoptics engine (`DIAKOPTICS_PSTCALC_PLAN.md` WP-AD.3).
//!
//! Behavioral spec = **official r3723 Delphi** (plan D10):
//! `.inputs/electricdss-code-r3723-trunk/Version8/Source/Common/Diakoptics.pas`
//! (whole unit) + `Common/Solution.pas` AD members. The vendored dss_capi
//! rewrite is the `TDSSContext` structure map.
//!
//! Ownership follows plan **D3** ("children without threads"): the coordinator
//! is the main [`Dss`] (Pascal `ActiveCircuit[1]`), which owns
//! `ad_children: Vec<Dss>` (Pascal `ActiveCircuit[2..NumOfActors]`). The Pascal
//! actor message loop (`Solution.pas:3156` `TSolver.Execute`) is mirrored by the
//! [`AdMsg`] enum and a synchronous method-call loop — no threads, no
//! `Arc<Mutex>`, so the bulk-synchronous algorithm is bit-for-bit deterministic.
//! Every Pascal `DSS.Parent.…`/`ActiveCircuit[1].…` child deref becomes an
//! explicit argument passed by the coordinator.
//!
//! The coordinator's AD matrices (`Contours`/`ZLL`/`ZCT`/`ZCC`/`Y4`/`Ic`) live on
//! `circuit.ad` ([`crate::circuit::AdTearing`]); the per-child index maps
//! (`LocalBusIdx`/`AD_IBus`/`AD_ISrcIdx`/`VIndex`) live on each child's
//! `circuit`/`solution` (WP-AD.3 solve stage). Pascal `Node_dV`/`Ic_Local`
//! (`Ymatrix.pas:430–434`) and `V_0` (`SendIdx2Actors`, only read by the dead
//! `Notify_Main`) are deliberately absent — allocated-but-never-read scaffolding
//! (plan D5).

mod matrices;

use crate::elements::traits::CktElement;
use crate::exec::registry::DssClass;

/// The A-Diakoptics actor message set (`Solution.pas:80–86` `TActorMessage`,
/// AD subset). Under plan D3 "sending a message" is a synchronous coordinator
/// method call over `ad_children`, not a thread signal — the enum keeps the 1:1
/// mapping to the Pascal `TSolver.Execute` `case MsgType of` dispatch
/// (`Solution.pas:3230–3264`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // variants land as the solve stage wires each SendCmd2Actors site.
pub(crate) enum AdMsg {
    /// `INIT_ADIAKOPTICS` (=2): `if ActorID>2 then Start_Diakoptics; IndexBuses`.
    InitAdiakoptics,
    /// `SOLVE_AD1` (=3): `SolveAD(ActorID, Initialize=True)` — source (+PC) inj.
    SolveAd1,
    /// `SOLVE_AD2` (=4): `SolveAD(ActorID, Initialize=False)` — `UpdateISrc`.
    SolveAd2,
    /// `DO_CTRL_ACTIONS`: child samples + runs queued control actions.
    DoCtrlActions,
    /// `GETCTRLMODE`: child copies the coordinator's control mode/iters.
    GetCtrlMode,
}

/// Look up a circuit element by `Class.Name` (Pascal `SetElementActive(myName)`
/// then read `ActiveCktElement`). Case-insensitive; `None` on a miss.
///
/// NOTE(upstream): Pascal `SetElementActive` on a miss leaves `ActiveCktElement`
/// at its prior value (a stale read). The AD callers only ever pass link-branch
/// names that resolve (a link is a real `Line` in the interconnected model), so
/// the miss path is unreachable; we return `None` rather than reproduce the
/// stale-state read (plan D5).
#[allow(dead_code)] // wired in by the init state machine (next staged commit).
pub(crate) fn ad_find_element<'a>(
    classes: &'a [DssClass],
    full_name: &str,
) -> Option<&'a dyn CktElement> {
    let lower = full_name.to_lowercase();
    let (cls_name, obj_name) = match lower.split_once('.') {
        Some((c, n)) => (Some(c), n),
        None => (None, lower.as_str()),
    };
    for class in classes {
        if class.kind.is_none() {
            continue;
        }
        if let Some(cn) = cls_name
            && !class.props.class_name().eq_ignore_ascii_case(cn)
        {
            continue;
        }
        if let Some(&oi) = class.name_to_idx.get(obj_name) {
            return class.objects[oi].as_ckt_element();
        }
    }
    None
}
