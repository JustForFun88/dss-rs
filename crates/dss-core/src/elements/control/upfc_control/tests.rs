//! Unit tests for the UPFCControl fleet build + Sample OR-accumulation against a
//! mock [`UpfcDispatchEnv`] (the live oracle gate is `exec/tests/upfc.rs`).

use super::*;
use crate::elements::traits::ElemRef;

/// A mock fleet: each entry's `check_status` result is scripted, and uploads are
/// counted. `all_enabled_upfcs` returns the refs in order.
struct MockEnv {
    refs: Vec<ElemRef>,
    status: Vec<bool>,
    checked: Vec<ElemRef>,
    uploaded: Vec<ElemRef>,
}

impl UpfcDispatchEnv for MockEnv {
    fn find_enabled_upfc(&self, _name: &str) -> Option<ElemRef> {
        None
    }
    fn all_enabled_upfcs(&self) -> Vec<ElemRef> {
        self.refs.clone()
    }
    fn check_status(&mut self, u: ElemRef) -> bool {
        self.checked.push(u);
        let i = self.refs.iter().position(|&r| r == u).unwrap();
        self.status[i]
    }
    fn upload_currents(&mut self, u: ElemRef) {
        self.uploaded.push(u);
    }
}

fn r(idx: usize) -> ElemRef {
    ElemRef { cls: 0, idx }
}

#[test]
fn sample_builds_fleet_and_ors_checkstatus() {
    let mut ctrl = UpfcControl::new("c");
    let mut env = MockEnv {
        refs: vec![r(1), r(2)],
        status: vec![false, true],
        checked: Vec::new(),
        uploaded: Vec::new(),
    };
    // First sample builds the list (list_size becomes 2) and ORs the statuses.
    let update = ctrl.sample(&mut env);
    assert!(update, "second UPFC needs an update");
    assert_eq!(ctrl.list_size, 2);
    assert_eq!(env.checked, vec![r(1), r(2)]);

    // DoPendingAction uploads on every UPFC in the fleet.
    ctrl.do_pending_action(&mut env);
    assert_eq!(env.uploaded, vec![r(1), r(2)]);
}

#[test]
fn sample_short_circuits_after_first_true() {
    let mut ctrl = UpfcControl::new("c");
    let mut env = MockEnv {
        refs: vec![r(1), r(2)],
        status: vec![true, true],
        checked: Vec::new(),
        uploaded: Vec::new(),
    };
    let update = ctrl.sample(&mut env);
    assert!(update);
    // `||` short-circuits: once the first UPFC returns true, the second is not
    // polled (matching FPC default Boolean evaluation).
    assert_eq!(env.checked, vec![r(1)]);
}

#[test]
fn empty_fleet_yields_no_update() {
    let mut ctrl = UpfcControl::new("c");
    let mut env = MockEnv {
        refs: vec![],
        status: vec![],
        checked: Vec::new(),
        uploaded: Vec::new(),
    };
    assert!(!ctrl.sample(&mut env));
    assert_eq!(ctrl.list_size, 0);
}

#[cfg(test)]
mod make_pos_seq_tests {
    use super::super::*;
    use crate::elements::pos_seq::{PosSeqCtx, PosSeqElemInfo};
    use crate::elements::traits::{CktElement, ElemRef};

    /// Pascal `TUPFCControlObj.MakePosSequence` (UPFCControl.pas:179) is the same
    /// NIL-deref hazard as GenDispatcher (Access violation #303). Safe-skip when
    /// the always-NIL ControlledElement is unresolved.
    #[test]
    fn crash_config_element_set_is_safe_skip() {
        let mut uc = UpfcControl::new("uc1");
        uc.ccd.monitored_element = Some(ElemRef { cls: 1, idx: 0 });
        let (np, nc) = (uc.ccd.cd.nphases, uc.ccd.cd.nconds);
        let bus = uc.ccd.cd.get_bus(1).to_string();
        let ctx = PosSeqCtx {
            monitored: Some(PosSeqElemInfo {
                nphases: 1,
                nconds: 1,
                bus_names: vec!["b1".into()],
                ..Default::default()
            }),
            controlled: None,
            ..Default::default()
        };
        let plan = uc.make_pos_sequence(&ctx);
        assert_eq!((uc.ccd.cd.nphases, uc.ccd.cd.nconds), (np, nc));
        assert_eq!(uc.ccd.cd.get_bus(1), bus);
        assert!(plan.run_base);
        assert_eq!(uc.monitored_element_ref(), Some(ElemRef { cls: 1, idx: 0 }));
    }
}
