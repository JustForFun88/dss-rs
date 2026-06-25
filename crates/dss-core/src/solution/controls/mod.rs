//! The control loop — Pascal `Solution.pas` `Sample_DoControlActions` /
//! `SampleControlDevices` / `DoControlActions` (l.1941–2008) plus
//! `Utilities.DoResetControls` and `ControlQueue.DoMultiRate`.
//!
//! Pascal reaches the control's controlled/monitored elements through live
//! object pointers; here every dispatch resolves the control's [`ElemRef`](crate::elements::traits::ElemRef)s
//! against the executive's class registry and splits the mutable borrows
//! (PHASE5_PLAN §2.1): the control object, its controlled transformer or
//! capacitor, and (CapControl only) the monitored element are borrowed at once
//! via `ElemStore::pair_mut`/`ElemStore::triple_mut` — RegControl and
//! Transformer are different classes, so the split always succeeds.
//!
//! The control queue is `std::mem::take`n out of the solution for the duration
//! of a sweep (Pascal's queue is a separate object, so actions pushing or
//! deleting further records mid-sweep work identically), and put back at the
//! end.
//!
//! Split into submodules (no behavioral change): the per-mode entry points and
//! `Reset` sweep live in [`sampling`], the queue-action dispatchers in
//! [`actions`], `DoMultiRate` in [`multi_rate`], and the borrow-splitting
//! dispatch core (plus the GenDispatcher environment) in [`dispatch`].

mod actions;
mod dispatch;
mod multi_rate;
mod sampling;

pub(crate) use dispatch::update_all_inv_controls;
pub(crate) use sampling::{reset_all_controls, sample_do_control_actions};

/// What the dispatch should invoke on the control element.
#[derive(Clone, Copy)]
pub(super) enum ControlOp {
    /// `TControlElem.Sample`.
    Sample,
    /// `TControlElem.DoPendingAction(Code, ProxyHdl)`.
    Action { code: i32 },
    /// `TControlElem.Reset` (the `Reset` command / `Set mode=` side effect).
    Reset,
}
