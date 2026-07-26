//! The torn sub-circuit file emission: `Save_SubCircuits`, `Format_SubCircuits`,
//! `AppendIsources`, `Disable_All_DER` (`DIAKOPTICS_PSTCALC_PLAN.md` WP-AD.2
//! Stage B, deliverable 2).
//!
//! Behavioral spec = **official r3723 Delphi** (plan D10):
//! `.inputs/electricdss-code-r3723-trunk/Version8/Source/Common/Circuit.pas`
//! (`Save_SubCircuits` 1150, `Format_SubCircuits` 959, `AppendIsources` 920,
//! `Disable_All_DER` 1463).
//!
//! `NOTE(subst-metis)`: the file layout is produced by *our* round-trip-faithful
//! [`Dss::do_save_circuit`](super::Dss::do_save_circuit) (`exec/save_circuit.rs`),
//! whose emitted `Master.dss` casing/header differs from the official DSS `Save`
//! (our master carries a leading comment + `Set DefaultBaseFreq`/`Set EarthModel`
//! lines the official one omits; verified only by re-compile, never by byte-match
//! — see that file's header). `Format_SubCircuits`'s line filter is therefore
//! matched **case-insensitively** and its "skip the header" cut is anchored on the
//! `New Circuit` line rather than the official fixed index 2, so it targets our
//! actual save output while preserving the algorithm's intent 1:1. Part II has no
//! oracle; the contract is the round-trip compile/solve gate (plan D8).

use super::*;

/// The six substrings that mark a "global/support" line in the saved master
/// (`Format_SubCircuits` `Reference`, Circuit.pas:975–976), lowercased for the
/// case-insensitive match (see the module `NOTE(subst-metis)`).
const REFERENCE: [&str; 6] = [
    "redirect energym",
    "redirect monitor",
    "makebu",
    "redirect busvolta",
    "buscoords busco",
    "redirect zone",
];

/// Case-insensitive `ansipos(needle, haystack) <> 0` (needle already lowercase).
fn contains_ci(haystack: &str, needle_lower: &str) -> bool {
    haystack.to_ascii_lowercase().contains(needle_lower)
}

impl Dss {
    /// Pascal `TDSSCircuit.Save_SubCircuits(AddISrc)` (Circuit.pas:1150): create a
    /// fresh `<CurrentDir>/Torn_Circuit`, `save circuit` the whole (now-torn)
    /// model into it, then reshape the emitted tree into independent OpenDSS
    /// sub-projects via [`Self::format_sub_circuits`].
    pub(super) fn save_sub_circuits(&mut self, add_isrc: bool) {
        let fileroot = self.current_dir.join("Torn_Circuit");

        // `CreateDir` + `DelFilesFromDir(Fileroot,'*',True)` (Circuit.pas:1157–1158)
        // — start from an empty directory. Scoped to the just-computed
        // `Torn_Circuit` path (never a junction); safe recursive clear.
        if fileroot.is_dir() {
            let _ = std::fs::remove_dir_all(&fileroot);
        }
        if let Err(e) = std::fs::create_dir_all(&fileroot) {
            self.errors.push(format!(
                "Could not create \"{}\" for the torn circuit. {e}",
                fileroot.display()
            ));
            return;
        }

        // `DssExecutive.Command := 'save circuit Dir="..."'` (Circuit.pas:1159).
        let root_str = fileroot.to_string_lossy().to_string();
        self.do_save_circuit(&root_str);

        // `Format_SubCircuits(FileRoot, length(Locations), AddISrc)`
        // (Circuit.pas:1161).
        let num_ckts = self
            .circuit
            .as_ref()
            .map(|c| c.ad.locations.len())
            .unwrap_or(0);
        self.format_sub_circuits(&fileroot, num_ckts, add_isrc);
    }

    /// Pascal `TDSSCircuit.Format_SubCircuits(Path, NumCkts, AddISrc)`
    /// (Circuit.pas:959): reshape the `save circuit` output into
    /// `Master_Interconnected.dss` (full model, support lines moved to the end) +
    /// a filtered zone-1 `Master.dss` + one `zone_k/Master.dss` and `VSource.dss`
    /// per sub-circuit `k = 2..NumCkts`.
    fn format_sub_circuits(&mut self, path: &Path, num_ckts: usize, add_isrc: bool) {
        let master_path = path.join("Master.dss");
        let file_struc: Vec<String> = match std::fs::read_to_string(&master_path) {
            Ok(text) => text.lines().map(|l| l.to_string()).collect(),
            Err(e) => {
                self.errors
                    .push(format!("Format_SubCircuits: cannot read Master.dss. {e}"));
                return;
            }
        };

        // --- Master_Interconnected.dss: non-support lines first, support (Xtra)
        // lines appended (Circuit.pas:992–1017).
        let mut interconnected = String::new();
        let mut xtra: Vec<&String> = Vec::new();
        for line in &file_struc {
            let is_support = REFERENCE.iter().any(|r| contains_ci(line, r));
            if is_support {
                xtra.push(line);
            } else {
                interconnected.push_str(line);
                interconnected.push('\n');
            }
        }
        for line in &xtra {
            interconnected.push_str(line);
            interconnected.push('\n');
        }
        if let Err(e) = std::fs::write(path.join("Master_Interconnected.dss"), &interconnected) {
            self.errors
                .push(format!("Format_SubCircuits: write interconnected. {e}"));
            return;
        }

        // --- Rewrite Master.dss (zone 1): drop the zone / EnergyMeter / Monitor
        // redirects (Circuit.pas:1019–1036).
        let mut zone1 = String::new();
        for line in &file_struc {
            if contains_ci(line, "redirect zone")
                || contains_ci(line, "redirect energym")
                || contains_ci(line, "redirect monitor")
            {
                continue;
            }
            zone1.push_str(line);
            zone1.push('\n');
        }
        if let Err(e) = std::fs::write(&master_path, &zone1) {
            self.errors
                .push(format!("Format_SubCircuits: rewrite Master.dss. {e}"));
            return;
        }

        // --- ISources at the zone-1 link edge, if requested (Circuit.pas:1039).
        if add_isrc {
            let link1 = self
                .circuit
                .as_ref()
                .and_then(|c| c.ad.link_branches.get(1).cloned())
                .unwrap_or_default();
            self.append_isources(&master_path, 1, &link1);
        }

        // --- Copy the support (pre-`Redirect zone`) files into each zone dir
        // (Circuit.pas:1042–1064; the copy is `{$IFNDEF FPC}`-gated upstream but
        // required for the zones to compile standalone here).
        self.copy_support_files_to_zones(path, &file_struc, num_ckts);

        // --- Per-zone Master.dss for k = 2..NumCkts (Circuit.pas:1065–1106).
        self.write_zone_masters(path, &file_struc, num_ckts, add_isrc);

        // --- Per-zone VSource.dss with the measured PConn voltages
        // (Circuit.pas:1107–1140).
        self.write_zone_vsources(path, num_ckts);
    }

    /// The index just past the `New Circuit` header line in the saved master —
    /// the start of the reusable global/support section. Our save master's header
    /// differs from the official fixed `[Clear, New Circuit]` prefix (see module
    /// `NOTE(subst-metis)`), so anchor on the `New Circuit` line instead of the
    /// official literal index 2 (Circuit.pas:1072).
    fn zone_global_start(file_struc: &[String]) -> usize {
        file_struc
            .iter()
            .position(|l| contains_ci(l, "new circuit"))
            .map(|i| i + 1)
            .unwrap_or(0)
    }

    /// The saved-master lines strictly between `Clear` and `New Circuit` — our
    /// round-trip-fidelity `Set DefaultBaseFreq` (a `NOTE(subst-metis)` addition
    /// the official `SaveMasterFile` omits, save_circuit.rs). These configure the
    /// global DSS state that `New Circuit` reads at creation
    /// (`TDSSCircuit.Create` sets `Fundamental := DefaultBaseFreq`,
    /// Circuit.pas:416), so a zone master must emit them **before** its own `New
    /// Circuit.Zone_k` — otherwise zones 2+ default to 60 Hz while zone-1 and
    /// `Master_Interconnected.dss` (which keep the full pre-header from the
    /// filtered file) run at the deck frequency. Empty on the official fixed
    /// prefix (`Clear` immediately followed by `New Circuit`).
    fn zone_pre_header(file_struc: &[String]) -> &[String] {
        let clear = file_struc
            .iter()
            .position(|l| l.trim().eq_ignore_ascii_case("clear"));
        let new_ckt = file_struc
            .iter()
            .position(|l| contains_ci(l, "new circuit"));
        match (clear, new_ckt) {
            (Some(c), Some(n)) if n > c + 1 => &file_struc[c + 1..n],
            _ => &[],
        }
    }

    /// Copy every support file redirected *before* the first `Redirect zone` line
    /// (linecodes, `Vsource.dss`, …) into each `zone_k` directory so the zone
    /// masters resolve them (Circuit.pas:1042–1064).
    fn copy_support_files_to_zones(&mut self, path: &Path, file_struc: &[String], num_ckts: usize) {
        for line in file_struc {
            if contains_ci(line, "redirect zone") {
                break; // stop at the first zone redirect (Circuit.pas:1063)
            }
            let Some(fname) = redirect_target(line) else {
                continue;
            };
            let src = path.join(&fname);
            for k in 2..=num_ckts {
                let zone_dir = path.join(format!("zone_{k}"));
                let _ = std::fs::create_dir_all(&zone_dir);
                let _ = std::fs::copy(&src, zone_dir.join(&fname));
            }
        }
    }

    /// Write `zone_k/Master.dss` for `k = 2..NumCkts` (Circuit.pas:1065–1106):
    /// `Clear` + `New Circuit.Zone_k` + the global/support section (up to the
    /// first `Redirect zone`) + this zone's own files (each `Redirect zone_k\<f>`
    /// with the `zone_k\` prefix stripped).
    fn write_zone_masters(
        &mut self,
        path: &Path,
        file_struc: &[String],
        num_ckts: usize,
        add_isrc: bool,
    ) {
        let global_start = Self::zone_global_start(file_struc);
        for k in 2..=num_ckts {
            let zone_dir = path.join(format!("zone_{k}"));
            let _ = std::fs::create_dir_all(&zone_dir);
            let mut out = String::new();
            out.push_str("Clear\n");
            // Global state read at circuit creation (`Set DefaultBaseFreq`) must
            // precede `New Circuit.Zone_k` so the zone inherits the deck
            // frequency (see `zone_pre_header`).
            for line in Self::zone_pre_header(file_struc) {
                out.push_str(line);
                out.push('\n');
            }
            out.push_str(&format!("New Circuit.Zone_{k}\n"));

            // Global/support section: from just past `New Circuit` up to (not
            // including) the first `Redirect zone` line.
            for line in file_struc.iter().skip(global_start) {
                if contains_ci(line, "redirect zone") {
                    break;
                }
                out.push_str(line);
                out.push('\n');
            }

            // This zone's own files: `Redirect zone_k\<f>` → `Redirect <f>`.
            let zone_tag = format!("redirect zone_{k}");
            let zone_prefix = format!("zone_{k}\\");
            for line in file_struc {
                if contains_ci(line, &zone_tag) {
                    out.push_str(&strip_prefix_ci(line, &zone_prefix));
                    out.push('\n');
                }
            }

            let zm = zone_dir.join("Master.dss");
            if let Err(e) = std::fs::write(&zm, &out) {
                self.errors
                    .push(format!("Format_SubCircuits: write zone_{k} master. {e}"));
                continue;
            }

            // ISources at this zone's link edges (Circuit.pas:1095–1104).
            if add_isrc {
                let (link_a, link_b) = {
                    let lb = self.circuit.as_ref().map(|c| &c.ad.link_branches);
                    (
                        lb.and_then(|l| l.get(k - 1).cloned()).unwrap_or_default(),
                        lb.and_then(|l| l.get(k).cloned()).unwrap_or_default(),
                    )
                };
                self.append_isources(&zm, 2, &link_a);
                let has_next = self
                    .circuit
                    .as_ref()
                    .is_some_and(|c| c.ad.link_branches.len() > k);
                if has_next {
                    self.append_isources(&zm, 1, &link_b);
                }
            }
        }
    }

    /// Write `VSource.dss` for zone 1 (`<path>/VSource.dss`) and every zone
    /// `k = 2..NumCkts` (`zone_k/VSource.dss`) from the measured `PConn` voltages
    /// (Circuit.pas:1107–1140): three single-phase sources per zone — an `Edit
    /// Vsource.source` for phase 1 (retargeting the copied master source) plus
    /// `New Vsource.Vph_2`/`Vph_3` — each `basekv`/`angle` from
    /// `PConn_Voltages` via FPC `floattostrF(…, ffGeneral, 8, 3)` = `fmt_g(v, 8)`.
    ///
    /// NOTE(upstream-quirk): the boundary source is written to `VSource.dss`
    /// (capital `S`, 1:1 with official `Format_SubCircuits`, Circuit.pas:1114)
    /// while the copied support redirect names `Vsource.dss` (lowercase `s`, from
    /// the saved Vsource-class file). These resolve to the *same* file — the
    /// boundary source overwriting the copied full source — only on a
    /// case-insensitive filesystem (Windows/NTFS, the official DSS + this project
    /// platform, D10). On a case-sensitive FS the zone master's `Redirect
    /// Vsource.dss` would instead pick up the copied full 3-phase source. The
    /// case-insensitivity assumption is inherited verbatim from upstream and not
    /// "fixed" (changing the emitted case would diverge from official).
    fn write_zone_vsources(&mut self, path: &Path, num_ckts: usize) {
        let (pconn_names, pconn_voltages) = match self.circuit.as_ref() {
            Some(c) => (c.ad.pconn_names.clone(), c.ad.pconn_voltages.clone()),
            None => return,
        };
        let mut vidx = 0usize; // flat index into PConn_Voltages (FS_Idx1)
        for k in 1..=num_ckts {
            let file = if k == 1 {
                path.join("VSource.dss")
            } else {
                path.join(format!("zone_{k}")).join("VSource.dss")
            };
            let pconn_bus = pconn_names.get(k - 1).cloned().unwrap_or_default();

            let mut out = String::new();
            for phase in 1..=3 {
                let (name, verb) = if phase == 1 {
                    ("source".to_string(), "Edit ")
                } else {
                    (format!("Vph_{phase}"), "New ")
                };
                let basekv = pconn_voltages.get(vidx).copied().unwrap_or(0.0);
                let angle = pconn_voltages.get(vidx + 1).copied().unwrap_or(0.0);
                out.push_str(&format!(
                    "{verb}Vsource.{name} bus1={pconn_bus}.{phase} phases=1 pu=1.0 \
                     basekv={} angle={} R1=0 X1=0.001 R0=0 X0=0.001\n",
                    crate::util::fmt_g(basekv, 8),
                    crate::util::fmt_g(angle, 8),
                ));
                vidx += 2;
            }
            if let Err(e) = std::fs::write(&file, &out) {
                self.errors.push(format!(
                    "Format_SubCircuits: write VSource for zone {k}. {e}"
                ));
            }
        }
    }

    /// Pascal `TDSSCircuit.AppendIsources(myPath, BusNum, LinkBranch)`
    /// (Circuit.pas:920): append one `New ISource.<BusNum>_<k> phases=1
    /// bus1=<bus>.<k> amps=0.000001 angle=0` per node of the link branch's bus
    /// `BusNum` (dot-stripped), used by the A-Diakoptics `AddISrc=TRUE` path.
    fn append_isources(&mut self, file_path: &Path, bus_num: usize, link_branch: &str) {
        if link_branch.is_empty() {
            return;
        }
        // `SetElementActive(LinkBranch); ActiveCktElement.GetBus(BusNum)`.
        let bus = match element_bus(&self.classes, link_branch, bus_num) {
            Some(b) => b,
            None => return,
        };
        // Strip the node dots (Circuit.pas:938–939).
        let bus_name = bus.split('.').next().unwrap_or(&bus).to_string();
        let num_nodes = self
            .circuit
            .as_ref()
            .and_then(|c| c.bus_list.find(&bus_name).map(|i| &c.buses[i]))
            .map(|b| b.num_nodes_this_bus())
            .unwrap_or(0);

        let mut appended = String::new();
        for kk in 1..=num_nodes {
            appended.push_str(&format!(
                "New ISource.{bus_num}_{kk} phases=1 bus1={bus_name}.{kk} amps=0.000001 angle=0\n"
            ));
        }
        if appended.is_empty() {
            return;
        }
        // `Append(myFile)`: append to the existing file.
        let mut existing = std::fs::read_to_string(file_path).unwrap_or_default();
        if !existing.is_empty() && !existing.ends_with('\n') {
            existing.push('\n');
        }
        existing.push_str(&appended);
        if let Err(e) = std::fs::write(file_path, existing) {
            self.errors.push(format!(
                "AppendIsources: write {}. {e}",
                file_path.display()
            ));
        }
    }

    /// Pascal `TDSSCircuit.Disable_All_DER` (Circuit.pas:1463): disable every
    /// `PVSystem`, `Generator`, and `Storage` element. Ported verbatim; consumed
    /// by the A-Diakoptics init flow (WP-AD.3) — `Tear_Circuit` itself leaves it
    /// commented out (Circuit.pas:1663), so no non-test caller exists yet.
    /// Exercised by the unit test below.
    #[allow(dead_code)] // WP-AD.3 (ADiakopticsInit) is the caller.
    pub(crate) fn disable_all_der(&mut self) {
        const DER_CLASSES: [&str; 3] = ["pvsystem", "generator", "storage"];
        for der in DER_CLASSES {
            let Some(&ci) = self.class_by_name.get(der) else {
                continue;
            };
            let arena = &mut self.classes[ci].arena;
            for i in 0..arena.len() {
                if let Some(ce) = arena.try_ckt_elem_mut(i) {
                    ce.cd_mut().set_enabled(false);
                }
            }
        }
    }
}

/// `stringreplace('Redirect ', '', [rfReplaceAll, rfIgnoreCase])` on a master
/// redirect line — the redirected filename (Circuit.pas:1052). `None` when the
/// line is not a `Redirect`.
fn redirect_target(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let pos = lower.find("redirect ")?;
    let after = &line[pos + "redirect ".len()..];
    // Take the filename token (up to whitespace or a trailing comment).
    let token = after.split_whitespace().next().unwrap_or("").trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

/// Case-insensitive `stringreplace(line, prefix, '', [rfReplaceAll, rfIgnoreCase])`
/// (Circuit.pas:1089): remove every occurrence of `prefix` (e.g. `zone_2\`) from
/// `line`.
fn strip_prefix_ci(line: &str, prefix_lower: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut rest = line;
    let plen = prefix_lower.len();
    while !rest.is_empty() {
        let lower = rest.to_ascii_lowercase();
        if let Some(pos) = lower.find(prefix_lower) {
            result.push_str(&rest[..pos]);
            rest = &rest[pos + plen..];
        } else {
            result.push_str(rest);
            break;
        }
    }
    result
}

/// `SetElementActive(full_name); ActiveCktElement.GetBus(bus_num)` (1-based
/// terminal) — the bus name at a PDE's terminal, or `None` when unresolved.
fn element_bus(classes: &[DssClass], full_name: &str, bus_num: usize) -> Option<String> {
    let lower = full_name.to_ascii_lowercase();
    let (cls_name, obj_name) = match lower.split_once('.') {
        Some((c, n)) => (Some(c), n),
        None => (None, lower.as_str()),
    };
    for class in classes {
        if class.kind.is_none() {
            continue;
        }
        if let Some(cn) = cls_name
            && !class.props.class_name().eq_ignore_ascii_case(cn)
        {
            continue;
        }
        if let Some(&oi) = class.name_to_idx.get(obj_name) {
            let bus = class.arena.try_ckt_elem(oi)?.cd().get_bus(bus_num);
            return Some(bus.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::Dss;

    #[test]
    fn redirect_target_extracts_filename() {
        assert_eq!(
            redirect_target("Redirect Line.dss").as_deref(),
            Some("Line.dss")
        );
        // Case-insensitive verb + trailing comment ignored.
        assert_eq!(
            redirect_target("redirect BusVoltageBases.dss  ! set voltage bases").as_deref(),
            Some("BusVoltageBases.dss")
        );
        assert_eq!(redirect_target("MakeBusList"), None);
        assert_eq!(redirect_target("BusCoords BusCoords.dss"), None);
    }

    #[test]
    fn strip_prefix_removes_zone_dir() {
        assert_eq!(
            strip_prefix_ci("Redirect zone_2\\Branches.dss", "zone_2\\"),
            "Redirect Branches.dss"
        );
        // Case-insensitive, all occurrences.
        assert_eq!(strip_prefix_ci("ZONE_3\\a ZONE_3\\b", "zone_3\\"), "a b");
        assert_eq!(
            strip_prefix_ci("no match here", "zone_9\\"),
            "no match here"
        );
    }

    #[test]
    fn contains_ci_is_case_insensitive() {
        assert!(contains_ci("BusCoords BusCoords.dss", "buscoords busco"));
        assert!(contains_ci("Redirect Zone_2\\x.dss", "redirect zone"));
        assert!(!contains_ci("Redirect Line.dss", "redirect zone"));
    }

    #[test]
    fn zone_global_start_anchors_on_new_circuit() {
        let master = [
            "! Saved by dss-rs".to_string(),
            "Clear".to_string(),
            "Set DefaultBaseFreq=60".to_string(),
            "New Circuit.foo".to_string(),
            "".to_string(),
            "Redirect LineCode.dss".to_string(),
        ];
        // Global section starts right after the `New Circuit` line (index 4),
        // not the official fixed index 2 (our master has extra header lines).
        assert_eq!(Dss::zone_global_start(&master), 4);
    }

    #[test]
    fn zone_pre_header_carries_default_base_freq() {
        let master = [
            "! Saved by dss-rs".to_string(),
            "Clear".to_string(),
            "Set DefaultBaseFreq=50".to_string(),
            "New Circuit.foo".to_string(),
            "".to_string(),
        ];
        // The line(s) between `Clear` and `New Circuit` — must be emitted before
        // the zone's own `New Circuit.Zone_k` so zones inherit the base freq.
        assert_eq!(Dss::zone_pre_header(&master), ["Set DefaultBaseFreq=50"]);

        // Official fixed prefix (Clear immediately followed by New Circuit): none.
        let official = ["Clear".to_string(), "New Circuit.foo".to_string()];
        assert!(Dss::zone_pre_header(&official).is_empty());
    }

    #[test]
    fn disable_all_der_disables_generators() {
        // Build a tiny circuit with a generator, then disable all DER.
        let mut dss = Dss::new();
        for cmd in [
            "clear",
            "new circuit.der basekv=12.47 phases=3 bus1=sb",
            "new line.l1 bus1=sb bus2=b1 r1=0.1 x1=0.2 c1=0",
            "new generator.g1 bus1=b1 phases=3 kv=12.47 kw=100 pf=1",
            "set voltagebases=[12.47]",
            "calcv",
        ] {
            dss.command(cmd);
        }
        dss.disable_all_der();
        // The generator is now disabled.
        let ci = dss.class_by_name["generator"];
        let enabled = dss.classes[ci]
            .arena
            .try_ckt_elem(0)
            .map(|e| e.cd().enabled)
            .unwrap_or(true);
        assert!(!enabled, "generator should be disabled by Disable_All_DER");
    }
}
