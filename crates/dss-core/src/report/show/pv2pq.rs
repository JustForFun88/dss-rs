//! `Show PV2PQ_Conversions` (Pascal `Common/ShowResults.pas` `ShowPV2PQGen`): the
//! list of generators that were converted from a PV bus to a PQ bus during the
//! last NCIM solution (`Flg.NCIM_ExPV` — the port's `Generator.ncim_expv`).

use crate::circuit::Circuit;
use crate::elements::pc::generator::Generator;
use crate::exec::registry::DssClass;

/// The fixed banner (Pascal `ShowPV2PQGen`): a 78-dash rule, the title, another
/// rule, then two blank lines (`FSWriteLn(F, rule)` ×3 then `FSWriteLn(F)` ×2),
/// followed by one `FullName` per converted generator. Emitted with `\n`; the
/// report comparator normalizes the oracle's CRLF.
const RULE: &str = "------------------------------------------------------------------------------";

/// Build the `Show PV2PQ_Conversions` text. Walks `Generators` in creation order;
/// each **enabled** generator carrying the `NCIM_ExPV` flag prints its `FullName`
/// (Pascal `if (Flg.NCIM_ExPV in pGen.Flags) then FSWriteLn(F, pGen.FullName())`).
/// A circuit that never ran NCIM (or converted no generator) yields just the
/// banner. Read-only (PHASE8_PLAN §2.1).
pub(crate) fn show_pv2pq_gen(classes: &[DssClass], ckt: &Circuit) -> String {
    // Pascal: rule, title, rule, blank, blank → `...\n...\n...\n\n\n`.
    let title = "LIST OF GENERATORS CONVERTED FROM PV TO PQ BUS DURING THE LAST SOLUTION (NCIM)";
    let mut s = format!("{RULE}\n{title}\n{RULE}\n\n\n");
    for &r in &ckt.generators {
        let obj = &classes[r.class_ord()].arena[r.index()];
        let Some(g) = classes[r.class_ord()].arena.get::<Generator>(r.index()) else {
            continue;
        };
        if !g.cd.enabled {
            continue;
        }
        if g.ncim_expv {
            s.push_str(&format!(
                "{}.{}\n",
                classes[r.class_ord()].props.class_name(),
                obj.data().name()
            ));
        }
    }
    s
}
