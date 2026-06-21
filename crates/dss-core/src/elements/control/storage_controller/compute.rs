//! Parse-time behavior: `MakeFleetList` and the `RecalcElementData` subset
//! (`StorageController.pas`). The live `Sample`/dispatch path is NOT_PORTED →
//! Phase 7 (see the module doc); in Phase 6 the fleet is always empty.

use super::StorageController;

impl StorageController {
    /// Pascal `TStorageControllerObj.MakeFleetList`. In Phase 6 there is no
    /// Storage class, so the fleet is always empty and the function returns
    /// `false` (the caller turns that into error 37201).
    ///
    /// The `FleetListChanged := FALSE` clear at the tail of Pascal `MakeFleetList`
    /// (l.1927) is reached by the **default** branch and by a **fully-resolved
    /// named** branch, but **not** by the early `Exit` taken when a named element
    /// is missing (l.1889). Reproducing that distinction is what stops a later
    /// `RecalcElementData` (a second `Edit`) from re-emitting 37201 every time:
    /// only the missing-name path leaves the rebuild pending.
    pub(super) fn make_fleet_list(&mut self) -> bool {
        if self.element_list_specified {
            // Named list. The first name never resolves (no Storage class), so a
            // *non-empty* list errors and Pascal `Exit`s with FleetListChanged
            // still TRUE.
            if let Some(first) = self.storage_name_list.first() {
                self.ccd
                    .cd
                    .obj
                    .push_error(format!("Error: Storage Element \"{first}\" not found."));
                return false; // FleetListChanged stays true (Pascal Exits here)
            }
            // Empty named list: the resolve loop never runs and Pascal falls
            // through to the shared tail below.
        } else {
            // Default branch: scan all enabled Storage (none exist) → empty fleet.
            self.storage_name_list.clear();
            self.fleet_size = 0;
            self.weights.clear();
        }
        // Pascal `FleetListChanged := FALSE` (l.1927).
        self.fleet_list_changed = false;
        false // FleetPointerList is always empty in Phase 6 → Result stays FALSE
    }

    /// Pascal `TStorageControllerObj.RecalcElementData` (parse-time subset):
    /// validate the monitored element, attach the control's single terminal to
    /// the monitored terminal's bus, then (re)build the fleet list.
    pub(super) fn recalc(&mut self) {
        let Some(mon) = self.mon_snap.clone() else {
            // Pascal `DoSimpleMsg('Monitored Element in %s is not set', 372)`.
            self.ccd.cd.obj.push_error(format!(
                "Monitored Element in StorageController.{} is not set",
                self.ccd.cd.obj.name()
            ));
            return;
        };

        if self.ccd.element_terminal > mon.nterms as i32 {
            // Pascal `DoErrorMsg(... 'Terminal no. "%d" Does not exist.' 371)`.
            self.ccd.cd.obj.push_error(format!(
                "StorageController: \"{}\": Terminal no. \"{}\" Does not exist. Re-specify terminal no.",
                self.ccd.cd.obj.name(),
                self.ccd.element_terminal
            ));
        } else {
            // Pascal: FNphases := MonitoredElement.Nphases; NConds := FNphases;
            // the control adopts the monitored element's phase count (so a later
            // MonPhase edit validates against the right number of phases).
            self.ccd.cd.nphases = mon.nphases;
            self.ccd.cd.set_nconds(mon.nphases);

            // Set the name of the control's 1st terminal's connected bus.
            let t = self.ccd.element_terminal;
            let bus = if t >= 1 && (t as usize) <= mon.buses.len() {
                mon.buses[(t - 1) as usize].clone()
            } else {
                String::new() // Pascal GetBus(i) out of range yields ''
            };
            self.ccd.cd.set_bus(1, &bus);
        }

        if self.fleet_list_changed && !self.make_fleet_list() {
            self.ccd.cd.obj.push_error(format!(
                "No unassigned Storage Elements found to assign to StorageController.{}",
                self.ccd.cd.obj.name()
            ));
        }
    }
}
