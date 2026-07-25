//! `Export AllocationFactors` (Pascal `Common/Utilities.pas`
//! `DumpAllocationFactors` (:784)): the per-load allocation/`C`-factor as an
//! assignable DSS command line — `Load.<name>.AllocationFactor=<f>` for a
//! connected-kVA-spec load, `Load.<name>.CFactor=<f>` for a kWh-spec load. Loads
//! specified by kW/kvar/kVA emit no line. A pure read of the Load spec fields.

use crate::circuit::Circuit;
use crate::elements::pc::load::{Load, LoadSpec};
use crate::exec::registry::DssClass;
use crate::report::format;

/// Build the `Export AllocationFactors` body (Pascal `DumpAllocationFactors`).
/// Walks **every** load (Pascal `for pLoad in Loads` — no `Enabled` filter);
/// only the `ConnectedkVA_PF` and `kwh_PF` spec types write a line. The name is
/// emitted in its native case (Pascal `'Load.' + pLoad.Name`), the factor
/// `%-.5g`. The literal `Load.` class prefix is upstream-hardcoded.
pub(crate) fn export_alloc_factors(classes: &[DssClass], ckt: &Circuit) -> String {
    let mut s = String::new();
    for &r in &ckt.loads {
        let obj = &classes[r.cls].arena[r.idx];
        let Some(load) = obj.as_any().downcast_ref::<Load>() else {
            continue;
        };
        let name = obj.data().name();
        match load.load_spec_type {
            LoadSpec::ConnectedKvaPf => {
                s.push_str(&format!(
                    "Load.{name}.AllocationFactor={}\n",
                    format::g(load.kva_allocation_factor, 5)
                ));
            }
            LoadSpec::KwhPf => {
                s.push_str(&format!(
                    "Load.{name}.CFactor={}\n",
                    format::g(load.c_factor, 5)
                ));
            }
            _ => {}
        }
    }
    s
}
