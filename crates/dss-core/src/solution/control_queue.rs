//! Port of `Common/ControlQueue.pas` (`TControlQueue`) — the time-ordered list
//! of pending control actions that drives RegControl tap changes and CapControl
//! step switching during the solution's control loop.
//!
//! **Container semantics are ported verbatim** (PHASE5_PLAN §WP5.4): the queue
//! is an ordered `Vec` (Pascal `TList`) into which `push` inserts by action
//! time, and the `do_*`/`pop` paths *linearly scan* for the earliest-time
//! record. The visit order and tie-breaking are observable through the event
//! log and the order in which actions mutate the circuit, so this is **not**
//! interchangeable with a `BinaryHeap`.
//!
//! The Pascal queue stores a live `TControlElem` pointer per record; here it
//! stores the control's stable [`ElemRef`] (PHASE5_PLAN §2.1). Acting on a
//! record routes back through a [`ControlActioner`], which the solution
//! implements (WP5.7) to build the split-borrow `CtrlCtx`, downcast the control
//! element, and invoke its `DoPendingAction`. Because `pop` returns owned
//! (`Copy`) record data, the queue itself is free to be handed to the actioner
//! as `&mut` — so an action may push or delete further records mid-sweep
//! (RegControl EVENTDRIVEN re-arms a one-step tap change this way), exactly like
//! the Pascal `Pop`-then-act loop.

use crate::elements::traits::ElemRef;

/// Pascal `TTimeRec`: an action time as whole hours plus seconds-within-hour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeRec {
    pub hour: i32,
    pub sec: f64,
}

impl TimeRec {
    /// Pascal `TimeRecToTime`: `Hour*3600 + Sec` (absolute seconds).
    pub fn to_time(self) -> f64 {
        self.hour as f64 * 3600.0 + self.sec
    }
}

/// Pascal `TActionRecord` (the queue payload).
#[derive(Debug, Clone, Copy)]
struct ActionRecord {
    action_time: TimeRec,
    action_code: i32,
    action_handle: i32,
    proxy_handle: i32,
    control: ElemRef,
}

/// The record data returned by a pop — Pascal's `var Code, ProxyHdl, Hdl` out
/// params plus the popped control element.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PoppedAction {
    pub(crate) control: ElemRef,
    pub(crate) code: i32,
    #[allow(dead_code)] // RegControl/CapControl ignore the proxy handle
    pub(crate) proxy: i32,
    #[allow(dead_code)] // handle is only consumed by the debug-trace path
    pub(crate) handle: i32,
}

/// The callback the control queue invokes to run a popped action — the
/// solution's bridge to `TControlElem.DoPendingAction` (implemented in WP5.7
/// over the split-borrow `CtrlCtx`). The queue is passed back in so the action
/// can arm/disarm further records.
pub trait ControlActioner {
    /// Pascal `pElem.DoPendingAction(Code, ProxyHdl)` for the control named by
    /// `control`.
    fn do_pending_action(
        &mut self,
        control: ElemRef,
        code: i32,
        proxy: i32,
        queue: &mut ControlQueue,
    );
}

/// Pascal `TControlQueue`.
#[derive(Debug, Clone, Default)]
pub struct ControlQueue {
    /// `ActionList`, kept ordered by ascending action time.
    action_list: Vec<ActionRecord>,
    /// `ctrlHandle`: a monotonically increasing serial number (the action
    /// handle), incremented on every push.
    ctrl_handle: i32,
}

impl ControlQueue {
    /// Pascal `TControlQueue.Init`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pascal `Push(Hour, Sec, Code, ProxyHdl, Owner)` (the integer-code
    /// overload all four collapse to). Normalizes `Sec > 3600` into whole
    /// hours, inserts the record before the first existing record of
    /// equal-or-greater time, and returns the new handle.
    pub fn push(&mut self, hour: i32, sec: f64, code: i32, proxy: i32, control: ElemRef) -> i32 {
        self.ctrl_handle += 1; // just a serial number

        // Normalize the time.
        let mut hr = hour;
        let mut s = sec;
        if s > 3600.0 {
            loop {
                hr += 1;
                s -= 3600.0;
                if s < 3600.0 {
                    break;
                }
            }
        }
        let trec = TimeRec { hour: hr, sec: s };
        let this_time = trec.to_time();

        let rec = ActionRecord {
            action_time: trec,
            action_code: code,
            action_handle: self.ctrl_handle,
            proxy_handle: proxy,
            control,
        };

        // Insert in order of time: before the first record whose time is
        // `>= this_time` (Pascal breaks at the first `ThisActionTime <=`),
        // else append.
        let pos = self
            .action_list
            .iter()
            .position(|a| this_time <= a.action_time.to_time())
            .unwrap_or(self.action_list.len());
        self.action_list.insert(pos, rec);

        self.ctrl_handle
    }

    /// Pascal `Push(Delay, Code, ProxyHdl, Owner)`: the action time is the
    /// current simulation time (`intHour`, `t`) plus `delay` seconds.
    pub fn push_delay(
        &mut self,
        int_hour: i32,
        t: f64,
        delay: f64,
        code: i32,
        proxy: i32,
        control: ElemRef,
    ) -> i32 {
        self.push(int_hour, t + delay, code, proxy, control)
    }

    /// Pascal `Clear`.
    pub fn clear(&mut self) {
        self.action_list.clear();
    }

    /// Pascal `IsEmpty`.
    pub fn is_empty(&self) -> bool {
        self.action_list.is_empty()
    }

    /// Pascal `Get_QueueSize` / `QueueSize`.
    pub fn queue_size(&self) -> usize {
        self.action_list.len()
    }

    /// Pascal `Delete(Hdl)`: remove the record with the given handle (the
    /// `DeleteFromQueue(i, popped := FALSE)` "by control device" path).
    pub fn delete(&mut self, handle: i32) {
        if let Some(i) = self
            .action_list
            .iter()
            .position(|a| a.action_handle == handle)
        {
            self.action_list.remove(i);
        }
    }

    /// Pascal `Pop_Time(ActionTime, …, var ATime, keepIn)`: like `pop`, but
    /// also returns the record's absolute action time, and with
    /// `keep_in = true` leaves the record in the queue (the `DoMultiRate`
    /// peek). Returns `(record, action_time_seconds)`.
    pub(crate) fn pop_time(
        &mut self,
        action_time: TimeRec,
        keep_in: bool,
    ) -> Option<(PoppedAction, f64)> {
        let t = action_time.to_time();
        let i = self
            .action_list
            .iter()
            .position(|a| a.action_time.to_time() <= t)?;
        let rec = self.action_list[i];
        if !keep_in {
            self.action_list.remove(i);
        }
        Some((
            PoppedAction {
                control: rec.control,
                code: rec.action_code,
                proxy: rec.proxy_handle,
                handle: rec.action_handle,
            },
            rec.action_time.to_time(),
        ))
    }

    /// Pascal `Pop(ActionTime, …)`: remove and return the earliest-listed
    /// record whose action time is `<= action_time`.
    fn pop(&mut self, action_time: TimeRec) -> Option<PoppedAction> {
        let t = action_time.to_time();
        let i = self
            .action_list
            .iter()
            .position(|a| a.action_time.to_time() <= t)?;
        let rec = self.action_list.remove(i);
        Some(PoppedAction {
            control: rec.control,
            code: rec.action_code,
            proxy: rec.proxy_handle,
            handle: rec.action_handle,
        })
    }

    /// Pascal `DoAllActions`: run every queued action (in list order), then
    /// clear. Re-reads the live length each step so actions appended mid-sweep
    /// are visited too (the FPC `for..in TList` enumerator semantics).
    pub fn do_all_actions(&mut self, act: &mut dyn ControlActioner) {
        let mut i = 0;
        while i < self.action_list.len() {
            let a = self.action_list[i];
            act.do_pending_action(a.control, a.action_code, a.proxy_handle, self);
            i += 1;
        }
        self.clear();
    }

    /// Pascal `DoNearestActions(var Hour, var Sec)`: take the time of the first
    /// queued record, set `hour`/`sec` to it, and run every record at `<=` that
    /// time (records pushed mid-sweep with a *later* time are left in place).
    /// Returns whether any action ran.
    pub fn do_nearest_actions(
        &mut self,
        hour: &mut i32,
        sec: &mut f64,
        act: &mut dyn ControlActioner,
    ) -> bool {
        let mut result = false;
        if let Some(first) = self.action_list.first() {
            let t = first.action_time;
            *hour = t.hour;
            *sec = t.sec;
            while let Some(p) = self.pop(t) {
                act.do_pending_action(p.control, p.code, p.proxy, self);
                result = true;
            }
        }
        result
    }

    /// Pascal `DoActions(Hour, Sec)`: run every record with action time `<= t`.
    pub fn do_actions(&mut self, hour: i32, sec: f64, act: &mut dyn ControlActioner) -> bool {
        let mut result = false;
        if !self.action_list.is_empty() {
            let t = TimeRec { hour, sec };
            while let Some(p) = self.pop(t) {
                act.do_pending_action(p.control, p.code, p.proxy, self);
                result = true;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
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
}
