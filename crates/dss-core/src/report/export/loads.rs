//! `Export Loads` (Pascal `ExportLoads`, `ExportResults.pas:2404`): the present
//! load allocation view — one row per enabled load with its connected kVA,
//! allocation factor, phase count, kW/kvar base, nominal PF, and load model.
//! Unlike the register dumps this is a **fresh** file (no append) with a fixed
//! header. A pure read of the Load fields (no solve state), walking
//! `Circuit.loads` in creation order.

use crate::elements::pc::load::Load;
use crate::elements::traits::ElemRef;
use crate::exec::registry::DssClass;

/// Format the `Export Loads` report. Pascal writes an unconditional `FSWriteln`
/// per load (a blank line for a disabled one); the golden's line reader drops
/// blank lines on both sides, so emitting only the enabled rows is equivalent.
pub(crate) fn export_loads(classes: &[DssClass], refs: &[ElemRef]) -> String {
    let mut out =
        String::from("Load, Connected KVA, Allocation Factor, Phases, kW, kvar, PF, Model\n");
    for &r in refs {
        let obj = &classes[r.cls].objects[r.idx];
        let Some(load) = obj.as_any().downcast_ref::<Load>() else {
            continue;
        };
        if !load.cd.enabled {
            continue;
        }
        // Pascal `WriteStr(sout, AnsiUpperCase(Name), Sep, ConnectedkVA:8:1,
        // Sep, FkVAAllocationFactor:5:3, Sep, NPhases:0, Sep, kWBase:8:1, Sep,
        // kvarBase:8:1, Sep, PFNominal:5:3, Sep, Ord(FLoadModel):0)`.
        out.push_str(&format!(
            "{}, {:8.1}, {:5.3}, {}, {:8.1}, {:8.1}, {:5.3}, {}\n",
            obj.data().name().to_uppercase(),
            load.connected_kva,
            load.kva_allocation_factor,
            load.cd.nphases,
            load.kw_base,
            load.kvar_base,
            load.pf_nominal,
            load.load_model as i32,
        ));
    }
    out
}
