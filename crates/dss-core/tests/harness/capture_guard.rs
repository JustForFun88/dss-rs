//! The "flag set but the oracle returned nothing" guard — the one rail a
//! flag-gated corpus comparator calls **before** it compares
//! (`GOLDEN_REBASE_PLAN.md` WP-G1 sub-step G1.0, §2.A A4).
//!
//! # The failure mode it closes
//!
//! A live corpus comparison is opt-in per case: a manifest `compare_*` flag
//! turns a surface on (`corpus_gate/manifest.rs`), the scheduler may force it
//! for a whole population (`corpus_gate/scheduler.rs::force_properties`, whose
//! forced set is pinned at `FORCED_PROPS_POPULATION` = 442 live non-`large`
//! cases), and the runner then walks *that channel's* capture. Every comparator
//! in this harness is written as "for each item the oracle sent, assert the Rust
//! value" — so an **absent or empty capture makes it compare nothing and pass**.
//! [`compare_all_properties`](super::compare_all_properties) is the shape: it is
//! a `for pc in exp` loop, and an empty `exp` is a green no-op that also records
//! zero compared elements.
//!
//! That turns several unrelated accidents into a silently green gate: a capture
//! the transport did not honor (an unknown/unrequested key on the capi channel,
//! an unserved mode on the r4133 bridge), one channel dropping a surface the
//! other still sends, or a refactor that stops issuing the request. Plan
//! §1.1(f)'s non-vacuity discipline proves the *comparator* rejects a corrupted
//! **Rust** value; it says nothing about there being an **oracle** side to
//! compare against at all. This module is that second half: it fails the case,
//! never skips it.
//!
//! # What it is deliberately not
//!
//! * **Not a count contract.** It asserts "> 0", not "the expected N items". A
//!   partial capture (3 of 40 elements) is the comparator's and the ledger's
//!   business — a rail that guessed an expected count would be a second,
//!   unpinned population fingerprint next to `population.lock.json`.
//! * **Not the process-global guard.**
//!   [`assert_r4133_props_compare_ran`](super::props_norm::assert_r4133_props_compare_ran)
//!   asserts that *somewhere* in a whole-population run the r4133 property walk
//!   happened at all; it is boolean and process-wide, so it cannot see a
//!   per-case or per-channel hole (its own doc records that blind spot). This
//!   rail is per (case, channel, surface) and fires on the first one.
//! * **Not a check that the flag is on.** A flag left OFF compares nothing and
//!   is invisible here; that is what the manifest flag vocabulary's structural
//!   gate and the scheduler's force rules cover.
//!
//! # Why a shared helper instead of an `assert!` per call site
//!
//! Two ad-hoc versions existed (`corpus_gate/runner.rs`, the
//! `compare_all_properties` and `compare_autoadd_log` sites) and neither named
//! the **channel** — on a `both` case the message could not say which of the two
//! oracles went silent. Every WP-G1 surface adds more flag-gated comparators
//! (`compare_derived`, `compare_bus`, `compare_zsc`, …), so the rule is written
//! once, tested once, and reused; the two migrated call sites keep it exercised
//! by the live gate from day one.

/// Shape wording for a capture that arrived but carries no item.
const EMPTY: &str = "empty (0 items)";

/// Shape wording for a capture field the channel did not send at all.
const ABSENT: &str = "absent (the channel sent no such field)";

/// A manifest compare-depth flag is ON for this (case, channel) but the
/// channel's capture for that surface holds **no items** ⇒ the case FAILS.
///
/// `present` is the capture's item count (`cap.len()`), `flag` the manifest
/// field name as the manifest spells it, `channel` the gating channel's tag
/// (`capi_v0145` / `r4133`), `ctx` the caller's usual `"{label} step {i}"`
/// context string. `#[track_caller]` so the panic points at the comparator, not
/// at this rail.
#[track_caller]
pub fn require_capture(flag: &str, channel: &str, present: usize, ctx: &str) {
    assert!(
        present > 0,
        "{}",
        missing_capture_msg(flag, channel, EMPTY, ctx)
    );
}

/// The same rule for an **optional** capture field: `None` ⇒ the case FAILS;
/// `Some(v)` ⇒ `v`, so the guard replaces the call site's `unwrap`/`expect`
/// rather than sitting next to one.
///
/// `T: ?Sized` so the common `Option<String>::as_deref()` shape (`Option<&str>`)
/// passes through unchanged. A *present but empty* payload is not this
/// function's business — a caller that can be empty as well as absent guards the
/// count with [`require_capture`] afterwards.
#[track_caller]
#[must_use]
pub fn require_capture_opt<'a, T: ?Sized>(
    flag: &str,
    channel: &str,
    cap: Option<&'a T>,
    ctx: &str,
) -> &'a T {
    match cap {
        Some(v) => v,
        None => panic!("{}", missing_capture_msg(flag, channel, ABSENT, ctx)),
    }
}

/// The refusal message itself, over **injected** values.
///
/// Split out for the reason `props_norm::check_r4133_props_compare_ran` is: the
/// rule is then provable offline, without unwinding, and the message contract
/// (it names the flag, the channel, the case context and which of the two
/// shapes fired) is pinned directly by
/// [`tests::the_message_names_the_flag_the_channel_the_context_and_the_shape`].
fn missing_capture_msg(flag: &str, channel: &str, shape: &str, ctx: &str) -> String {
    format!(
        "{ctx}: manifest flag `{flag}` is ON, but the `{channel}` capture for that surface is \
         {shape}. A flag-gated comparator over an empty capture compares nothing and passes \
         vacuously, so the case FAILS here instead (GOLDEN_REBASE_PLAN.md §1.1(f); the G1.0 \
         rail). Either the capture request was not honored on this channel (capi_v0145: \
         `tools/oracle/oracle_server.py`; r4133: `crates/dss-epri/src/capture.rs`), or this \
         case must not carry `{flag}`."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run `f` and return the panic message — the guards are proven to fire, not
    /// merely assumed to (`corpus_gate/runner.rs::panic_msg` is the same
    /// extractor; it lives in another test binary, so this module keeps its own).
    fn panic_message(f: impl FnOnce() + std::panic::UnwindSafe) -> String {
        let payload = std::panic::catch_unwind(f).expect_err("the guard must panic");
        if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic payload>".to_string()
        }
    }

    /// Every refusal names all four moving parts, so a red gate says *which*
    /// case, *which* flag, *which* oracle went silent and *how*.
    fn assert_names_everything(msg: &str, shape: &str) {
        for token in ["compare_zsc", "r4133", "asymmetric:x/y.dss step 0", shape] {
            assert!(
                msg.contains(token),
                "message {msg:?} does not name {token:?}"
            );
        }
    }

    /// A non-empty capture is a no-op — the rail must not cost a green case.
    #[test]
    fn a_present_capture_passes() {
        require_capture("compare_zsc", "capi_v0145", 1, "modes:a/b.dss step 0");
        require_capture("compare_zsc", "r4133", 40, "modes:a/b.dss step 3");
        let log = String::from("Bus, kW\n");
        assert_eq!(
            require_capture_opt(
                "compare_autoadd_log",
                "capi_v0145",
                Some(log.as_str()),
                "ctx"
            ),
            "Bus, kW\n"
        );
        let rows = vec![1u8, 2, 3];
        assert_eq!(
            require_capture_opt("compare_zsc", "r4133", Some(&rows), "ctx"),
            &rows
        );
    }

    /// Non-vacuity, shape 1: the flag is on and the channel's capture arrived
    /// **empty** — an empty `Vec` is exactly what makes every comparator below
    /// compare nothing, so it must be a failure and not a skip.
    #[test]
    fn an_empty_capture_fails() {
        let cap: Vec<String> = Vec::new();
        let msg = panic_message(|| {
            require_capture(
                "compare_zsc",
                "r4133",
                cap.len(),
                "asymmetric:x/y.dss step 0",
            )
        });
        assert_names_everything(&msg, EMPTY);
    }

    /// Non-vacuity, shape 2: the channel did not send the field at all.
    #[test]
    fn an_absent_capture_fails() {
        let msg = panic_message(|| {
            let _ = require_capture_opt::<str>(
                "compare_zsc",
                "r4133",
                None,
                "asymmetric:x/y.dss step 0",
            );
        });
        assert_names_everything(&msg, ABSENT);
        // The same for a non-`str` payload, so the `?Sized` generic is covered
        // in both shapes it is used in.
        let msg = panic_message(|| {
            let _ = require_capture_opt::<Vec<u8>>(
                "compare_zsc",
                "r4133",
                None,
                "asymmetric:x/y.dss step 0",
            );
        });
        assert_names_everything(&msg, ABSENT);
    }

    /// The message contract, offline: both shapes, all four parts, and the two
    /// shapes distinguishable from each other.
    #[test]
    fn the_message_names_the_flag_the_channel_the_context_and_the_shape() {
        let empty = missing_capture_msg("compare_zsc", "r4133", EMPTY, "asymmetric:x/y.dss step 0");
        let absent =
            missing_capture_msg("compare_zsc", "r4133", ABSENT, "asymmetric:x/y.dss step 0");
        assert_names_everything(&empty, EMPTY);
        assert_names_everything(&absent, ABSENT);
        assert!(
            !empty.contains(ABSENT),
            "{empty:?} conflates the two shapes"
        );
        assert!(
            !absent.contains(EMPTY),
            "{absent:?} conflates the two shapes"
        );
    }
}
