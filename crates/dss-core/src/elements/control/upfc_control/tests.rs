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
