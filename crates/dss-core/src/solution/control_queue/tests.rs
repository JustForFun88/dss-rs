use super::*;

fn elem(idx: usize) -> ElemRef {
    ElemRef { cls: 0, idx }
}

/// A follow-up action to push when control `on_idx` fires (the EVENTDRIVEN
/// re-arm path).
#[derive(Clone, Copy)]
struct ReArm {
    on_idx: usize,
    hour: i32,
    sec: f64,
    code: i32,
    proxy: i32,
    control: ElemRef,
}

/// Records every `(control, code, proxy)` it is asked to act on; can be
/// programmed to push a follow-up action when a given control fires.
#[derive(Default)]
struct Recorder {
    seen: Vec<(usize, i32, i32)>,
    rearm: Option<ReArm>,
}

impl ControlActioner for Recorder {
    fn do_pending_action(
        &mut self,
        control: ElemRef,
        code: i32,
        proxy: i32,
        queue: &mut ControlQueue,
    ) {
        self.seen.push((control.idx, code, proxy));
        if let Some(r) = self.rearm
            && control.idx == r.on_idx
        {
            self.rearm = None; // arm once
            queue.push(r.hour, r.sec, r.code, r.proxy, r.control);
        }
    }
}

#[test]
fn push_returns_increasing_handles_and_orders_by_time() {
    let mut q = ControlQueue::new();
    let h1 = q.push(0, 30.0, 6, 0, elem(1));
    let h2 = q.push(0, 10.0, 6, 0, elem(2));
    let h3 = q.push(0, 20.0, 6, 0, elem(3));
    assert_eq!((h1, h2, h3), (1, 2, 3));
    assert_eq!(q.queue_size(), 3);

    // do_actions at a time covering all three pops them in time order.
    let mut rec = Recorder::default();
    assert!(q.do_actions(1, 0.0, &mut rec));
    assert_eq!(
        rec.seen,
        vec![(2, 6, 0), (3, 6, 0), (1, 6, 0)] // 10s, 20s, 30s
    );
    assert!(q.is_empty());
}

#[test]
fn equal_time_pushes_insert_before_earlier_ones() {
    // Pascal inserts before the first record with `ThisActionTime <=`, so a
    // later push of an equal time lands *ahead* of the earlier equal-time
    // record.
    let mut q = ControlQueue::new();
    q.push(0, 10.0, 0, 0, elem(1));
    q.push(0, 10.0, 0, 0, elem(2));
    q.push(0, 10.0, 0, 0, elem(3));
    let mut rec = Recorder::default();
    q.do_actions(0, 10.0, &mut rec);
    assert_eq!(
        rec.seen.iter().map(|s| s.0).collect::<Vec<_>>(),
        vec![3, 2, 1]
    );
}

#[test]
fn sec_over_3600_normalizes_into_hours() {
    let mut q = ControlQueue::new();
    q.push(0, 3700.0, 0, 0, elem(1)); // -> Hour 1, Sec 100
    // Not due at hour 0 / 200s.
    let mut rec = Recorder::default();
    assert!(!q.do_actions(0, 200.0, &mut rec));
    assert_eq!(q.queue_size(), 1);
    // Due at hour 1 / 100s.
    assert!(q.do_actions(1, 100.0, &mut rec));
    assert_eq!(rec.seen, vec![(1, 0, 0)]);
}

#[test]
fn delete_by_handle_removes_only_that_record() {
    let mut q = ControlQueue::new();
    let _ = q.push(0, 10.0, 0, 0, elem(1));
    let h2 = q.push(0, 20.0, 0, 0, elem(2));
    let _ = q.push(0, 30.0, 0, 0, elem(3));
    q.delete(h2);
    assert_eq!(q.queue_size(), 2);
    let mut rec = Recorder::default();
    q.do_actions(1, 0.0, &mut rec);
    assert_eq!(rec.seen.iter().map(|s| s.0).collect::<Vec<_>>(), vec![1, 3]);
}

#[test]
fn do_nearest_actions_runs_only_the_earliest_time_bucket() {
    let mut q = ControlQueue::new();
    q.push(0, 10.0, 0, 0, elem(1));
    q.push(0, 10.0, 0, 0, elem(2));
    q.push(0, 25.0, 0, 0, elem(3)); // later — must remain
    let (mut hour, mut sec) = (-1, -1.0);
    let mut rec = Recorder::default();
    assert!(q.do_nearest_actions(&mut hour, &mut sec, &mut rec));
    assert_eq!((hour, sec), (0, 10.0));
    assert_eq!(rec.seen.len(), 2);
    assert_eq!(q.queue_size(), 1); // elem 3 still queued
}

#[test]
fn action_can_rearm_a_later_record_mid_sweep() {
    // do_nearest_actions fixes the bucket time at 10s; the re-armed action
    // at 30s is NOT swept now (EVENTDRIVEN single-step semantics).
    let mut q = ControlQueue::new();
    q.push(0, 10.0, 6, 0, elem(1));
    let mut rec = Recorder {
        rearm: Some(ReArm {
            on_idx: 1,
            hour: 0,
            sec: 30.0,
            code: 6,
            proxy: 0,
            control: elem(1),
        }),
        ..Default::default()
    };
    let (mut hour, mut sec) = (0, 0.0);
    q.do_nearest_actions(&mut hour, &mut sec, &mut rec);
    assert_eq!(rec.seen, vec![(1, 6, 0)]);
    assert_eq!(q.queue_size(), 1); // the re-armed 30s action remains

    // A do_actions covering 30s now fires it.
    let mut rec2 = Recorder::default();
    q.do_actions(0, 30.0, &mut rec2);
    assert_eq!(rec2.seen, vec![(1, 6, 0)]);
    assert!(q.is_empty());
}

#[test]
fn do_all_actions_runs_everything_then_clears() {
    let mut q = ControlQueue::new();
    q.push(5, 0.0, 1, 0, elem(1)); // out-of-time-order on purpose
    q.push(0, 0.0, 2, 0, elem(2));
    let mut rec = Recorder::default();
    q.do_all_actions(&mut rec);
    // List order (time-sorted): elem2 @0, elem1 @5h.
    assert_eq!(rec.seen, vec![(2, 2, 0), (1, 1, 0)]);
    assert!(q.is_empty());
}
