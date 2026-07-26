use super::*;

fn units() -> DssEnum {
    let reg = EnumRegistry::new();
    reg.get(reg.units).clone()
}

#[test]
fn units_exact_and_abbreviated() {
    let u = units();
    assert_eq!(u.string_to_ordinal("none").unwrap(), 0);
    assert_eq!(u.string_to_ordinal("mi").unwrap(), 1);
    assert_eq!(u.string_to_ordinal("kft").unwrap(), 2);
    assert_eq!(u.string_to_ordinal("km").unwrap(), 3);
    assert_eq!(u.string_to_ordinal("m").unwrap(), 4);
    assert_eq!(u.string_to_ordinal("meter").unwrap(), 4); // alias
    assert_eq!(u.string_to_ordinal("miles").unwrap(), 1); // alias
    assert_eq!(u.string_to_ordinal("ft").unwrap(), 5);
    assert_eq!(u.string_to_ordinal("in").unwrap(), 6);
    assert_eq!(u.string_to_ordinal("cm").unwrap(), 7);
    assert_eq!(u.string_to_ordinal("mm").unwrap(), 8);
}

#[test]
fn unmatched_falls_back_to_default_or_error() {
    let u = units();
    // UnitsEnum has DefaultValue = 0
    assert_eq!(u.string_to_ordinal("zz").unwrap(), 0);

    let mut nodefault = DssEnum::new("X", true, 1, 1, &["a", "b"], &[1, 2]);
    assert!(nodefault.string_to_ordinal("q").is_err());
    nodefault.default_value = 2;
    assert_eq!(nodefault.string_to_ordinal("q").unwrap(), 2);
}

#[test]
fn ordinal_to_string_roundtrip() {
    let u = units();
    assert_eq!(u.ordinal_to_string(0), "none");
    assert_eq!(u.ordinal_to_string(4), "m");
    assert_eq!(u.ordinal_to_string(8), "mm");
    assert_eq!(u.ordinal_to_string(99), ""); // out of range, not hybrid
}

#[test]
fn hybrid_parses_numbers() {
    let mut e = DssEnum::new(
        "Monitored Phase",
        true,
        1,
        2,
        &["min", "max", "avg"],
        &[-3, -2, -1],
    );
    e.hybrid = true;
    assert_eq!(e.string_to_ordinal("max").unwrap(), -2);
    assert_eq!(e.string_to_ordinal("2").unwrap(), 2);
    // hybrid_min clamps from below (Pascal Max(HybridMin, Result))
    assert_eq!(e.string_to_ordinal("0").unwrap(), 1);
    assert!(e.string_to_ordinal("xy").is_err());
    assert!(e.is_ordinal_valid(7));
    assert!(e.is_ordinal_valid(-3));
    assert!(!e.is_ordinal_valid(-7));
    assert_eq!(e.ordinal_to_string(12), "12");
}

#[test]
fn shorter_names_are_skipped_without_allow_longer() {
    // value longer than a name can never match it unless AllowLonger
    let u = units();
    assert_eq!(u.string_to_ordinal("kilometer").unwrap(), 0); // default, no match
}

#[test]
fn joined_lists_names() {
    let e = DssEnum::new("X", true, 1, 1, &["a", "b"], &[1, 2]);
    assert_eq!(e.joined(), "[a,b]");
}

/// dss_capi 0.15.x LineType enum-width fix (DSSClass.pas:1071, `4 -> 5`).
/// The eight `swt_*` names collapse to the ambiguous 4-char prefix `swt_`; with
/// the old MaxChars=4 a 5-char abbreviation like `swt_l` fell back to the
/// default `oh` (ordinal 1). With MaxChars=5 the 5-char prefix disambiguates.
/// Probed on capi015 2026-07-16 (probe_abbr.py): `swt_l -> swt_ldbrk`, etc.
#[test]
fn line_type_abbreviations_widened_to_five_chars() {
    let reg = EnumRegistry::new();
    let lt = reg.get(reg.line_type).clone();
    // 5-char abbreviations now disambiguate the swt_ group (were `oh`=1 at MaxCh=4)
    assert_eq!(lt.string_to_ordinal("swt_l").unwrap(), 5); // swt_ldbrk
    assert_eq!(lt.string_to_ordinal("swt_f").unwrap(), 6); // swt_fuse
    assert_eq!(lt.string_to_ordinal("swt_s").unwrap(), 7); // swt_sect
    assert_eq!(lt.string_to_ordinal("swt_r").unwrap(), 8); // swt_rec
    assert_eq!(lt.string_to_ordinal("swt_d").unwrap(), 9); // swt_disc
    assert_eq!(lt.string_to_ordinal("swt_b").unwrap(), 10); // swt_brk
    assert_eq!(lt.string_to_ordinal("swt_e").unwrap(), 11); // swt_elbow
    // Full names always matched (exact-name shortcut, MaxChars-independent).
    assert_eq!(lt.string_to_ordinal("swt_ldbrk").unwrap(), 5);
    assert_eq!(lt.string_to_ordinal("busbar").unwrap(), 12);
    // 4-char-disambiguable names unaffected by the widening.
    assert_eq!(lt.string_to_ordinal("oh").unwrap(), 1);
    assert_eq!(lt.string_to_ordinal("ug_t").unwrap(), 3); // ug_ts
    assert_eq!(lt.string_to_ordinal("ug_c").unwrap(), 4); // ug_cn
}

/// The `DssEnum`-registry ↔ typed-enum coupling (DE_PASCALIZE §P1).
///
/// Each P1 field enum claims, in its doc comment, that its discriminants are
/// exactly the value list of a specific `DssEnum`. Until this module the claim
/// was prose only: a later edit to a `registry/*.rs` value array (or to a
/// discriminant) would not fail any unit test — the mismatch would surface far
/// from its cause, as a corpus-gate/dump difference, and the
/// `from_ordinal(v).unwrap_or(current)` write path would silently swallow it.
/// Here the coupling is machine-checked against the **live** registry: every
/// ordinal the registry can produce must resolve, and must round-trip.
///
/// Not covered here (deliberate, they have no registry entry): the
/// `ControlQueue` action codes (`InvPendingChange`, `ExpPendingChange`,
/// `StorageCtrlAction`, `RegControlAction`), which are class-private queue
/// codes, `VarMode` (`PVsystem.pas:32-33`), an internal field with no
/// `DssEnum`, and the four *derived* spec codes of W3.2 (`ReactorSpecType`,
/// `CapacitorSpecType`, `VsourceZSpec`, `OcpDeviceType`) — no property writes
/// them, so they have no registry entry either. All are pinned by their own
/// Pascal-literal tests. (The shared `ControlAction` channel IS registry-backed
/// and is covered here; its out-of-registry `Keep`/`Other` values are pinned in
/// `control_elem.rs`.)
#[cfg(test)]
mod registry_enum_coupling {
    use super::EnumRegistry;

    use crate::elements::control::control_elem::ControlAction;
    use crate::elements::control::espvl_control::EspvlControlType;
    use crate::elements::control::inv_control::{
        InvCombiMode, InvControlMode, InvControlModel, RateOfChangeMode, ReacPowerRef,
        VoltWattYAxis, VoltageCurveXRef,
    };
    use crate::elements::control::mon_phase::MonPhase;
    use crate::elements::control::storage_controller::StorageCtrlMode;
    use crate::elements::general::load_shape::LoadShapeInterp;
    use crate::elements::pc::generator::GenDispatchMode;
    use crate::elements::pc::source_seq::{ScanType, SequenceType};
    use crate::elements::pc::storage::StorageState;
    use crate::elements::pc::vs_converter::VscMode;
    use crate::obj::dss_enum::EnumId;
    use crate::solution::{ControlMode, LoadSolutionModel, RandomType};

    /// Assert that every ordinal the named registry enum declares resolves
    /// through `from_ordinal` and round-trips back to the same number.
    fn check(reg: &EnumRegistry, id: EnumId, from: fn(i32) -> Option<i32>) {
        let e = reg.get(id);
        assert!(!e.ordinals.is_empty(), "{} has no ordinals", e.name);
        for &ord in &e.ordinals {
            let back = from(ord).unwrap_or_else(|| {
                panic!("{}: registry ordinal {ord} does not resolve", e.name);
            });
            assert_eq!(back, ord, "{}: ordinal {ord} did not round-trip", e.name);
        }
    }

    #[test]
    fn every_retyped_family_covers_its_registry_value_list() {
        let reg = EnumRegistry::new();

        check(&reg, reg.control_mode, |v| {
            ControlMode::from_ordinal(v).map(|m| m.ordinal())
        });
        check(&reg, reg.random_mode, |v| {
            RandomType::from_ordinal(v).map(|m| m.ordinal())
        });
        check(&reg, reg.default_load_model, |v| {
            LoadSolutionModel::from_ordinal(v).map(|m| m.ordinal())
        });

        // The two hybrid phase-selection enums share `MonPhase` (whose
        // `from_ordinal` is total — the hybrid fallback is a phase number).
        check(&reg, reg.mon_phase, |v| {
            Some(MonPhase::from_ordinal(v).ordinal())
        });
        check(&reg, reg.reg_control_phase, |v| {
            Some(MonPhase::from_ordinal(v).ordinal())
        });

        check(&reg, reg.invcontrol_mode, |v| {
            InvControlMode::from_ordinal(v).map(|m| m.ordinal())
        });
        check(&reg, reg.invcontrol_combi, |v| {
            InvCombiMode::from_ordinal(v).map(|m| m.ordinal())
        });
        check(&reg, reg.invcontrol_voltage_curvex, |v| {
            VoltageCurveXRef::from_ordinal(v).map(|m| m.ordinal())
        });
        check(&reg, reg.invcontrol_voltwatt_yaxis, |v| {
            VoltWattYAxis::from_ordinal(v).map(|m| m.ordinal())
        });
        check(&reg, reg.invcontrol_roc, |v| {
            RateOfChangeMode::from_ordinal(v).map(|m| m.ordinal())
        });
        check(&reg, reg.invcontrol_reac_power, |v| {
            ReacPowerRef::from_ordinal(v).map(|m| m.ordinal())
        });
        check(&reg, reg.invcontrol_model, |v| {
            InvControlModel::from_ordinal(v).map(|m| m.ordinal())
        });

        check(&reg, reg.espvl_control_type, |v| {
            EspvlControlType::from_ordinal(v).map(|m| m.ordinal())
        });
        // One shared field, two registry enums (discharge + charge modes).
        check(&reg, reg.storage_ctrl_discharge_mode, |v| {
            StorageCtrlMode::from_ordinal(v).map(|m| m.ordinal())
        });
        check(&reg, reg.storage_ctrl_charge_mode, |v| {
            StorageCtrlMode::from_ordinal(v).map(|m| m.ordinal())
        });

        check(&reg, reg.load_shape_interp, |v| {
            LoadShapeInterp::from_ordinal(v).map(|m| m.ordinal())
        });
        check(&reg, reg.gen_disp_mode, |v| {
            GenDispatchMode::from_ordinal(v).map(|m| m.ordinal())
        });
        // `StorageState`'s channel is open (`Fstate := Trunc(Value)`), so its
        // `from_ordinal` is total by construction.
        check(&reg, reg.storage_state, |v| {
            Some(StorageState::from_ordinal(v).ordinal())
        });

        // The two shared source phase-rotation selectors: one registry entry
        // each, three classes each (VSource / Isource / GICLine).
        check(&reg, reg.scan_type, |v| {
            ScanType::from_ordinal(v).map(|m| m.ordinal())
        });
        check(&reg, reg.sequence, |v| {
            SequenceType::from_ordinal(v).map(|m| m.ordinal())
        });
        check(&reg, reg.vsc_mode, |v| {
            VscMode::from_ordinal(v).map(|m| m.ordinal())
        });

        // The shared `EControlAction` channel: eight registry entries across
        // four classes map onto one enum (`Action` and `State` per class, and
        // SwtControl's `Normal` reuses its `State` entry). `from_ordinal` is
        // total here too — the control-queue code channel is open.
        for id in [
            reg.swt_control_action,
            reg.swt_control_state,
            reg.fuse_action,
            reg.fuse_state,
            reg.recloser_action,
            reg.recloser_state,
            reg.relay_action,
            reg.relay_state,
        ] {
            check(&reg, id, |v| Some(ControlAction::from_ordinal(v).ordinal()));
        }
    }
}
