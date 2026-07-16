//! Public query/snapshot API over [`Dss`] for the golden/test harness
//! (monitor buffers, meter zones, element snapshots, system Y, ...).
//! Split out of `exec/mod.rs`.

use super::*;
use crate::report::export::json::{
    JsonOpts, build as json_build, circuit as json_circuit, serialize as json_serialize,
};

/// A monitor's recorded buffer for the golden/test harness (dss-python
/// `Monitors.Header` / `SampleCount` / `Channel(i)` / `dblHour`).
#[derive(Debug, Clone)]
pub struct MonitorView {
    pub header: Vec<String>,
    pub sample_count: i32,
    /// Per-sample hour values (record slot 0).
    pub dbl_hour: Vec<f64>,
    /// `channels[i]` = the (i+1)-th channel across all samples (f32).
    pub channels: Vec<Vec<f32>>,
}

/// Raw `ElemRef` lists copied out of an [`energymeter::EnergyMeter`] before
/// resolving full names (avoids a long tuple type in [`Dss::meter_zone`]).
struct MeterZoneRefs {
    branches: Vec<ElemRef>,
    ends: Vec<ElemRef>,
    pce: Vec<ElemRef>,
    register_names: Vec<String>,
}

/// An EnergyMeter's zone topology for the test/golden harness (dss-python
/// `Meters.AllBranchesInZone` / `AllEndElements` / `ZonePCE`).
#[derive(Debug, Clone)]
pub struct MeterZoneView {
    /// `AllBranchesInZone`: the zone branches in `SequenceList` order (FullNames).
    pub all_branches_in_zone: Vec<String>,
    /// `AllEndElements`: the feeder-end branches (FullNames).
    pub all_end_elements: Vec<String>,
    /// `ZonePCE`: the zone PC elements (loads/generators), FullNames.
    pub zone_pce: Vec<String>,
    /// `RegisterNames` (length `NumEMRegisters`).
    pub register_names: Vec<String>,
}

/// Per-element snapshot for the golden feeder gate: mirrors dss-python's
/// `CktElement.Powers`/`Currents` over the oracle's `First/Next` iteration
/// (= creation) order.
#[derive(Debug, Clone)]
pub struct ElementSnapshot {
    /// `FullName` (`Class.name`).
    pub name: String,
    /// `Enabled`.
    pub enabled: bool,
    /// `BusNames`: the stored bus spec per terminal (`GetBus(i)`).
    pub bus_names: Vec<String>,
    /// kW/kvar interleaved per conductor and terminal (CAPI
    /// `Alt_CE_Get_Powers`: `GetPhasePower · 0.001`).
    pub powers: Vec<f64>,
    /// Amps, re/im interleaved per conductor and terminal (`Iterminal`).
    pub currents: Vec<f64>,
    /// Element losses (W, var) — `TDSSCktElement.Get_Losses` (the dss-python
    /// `CktElement.Losses` surface): `Σ NodeV[ref]·conj(Iterminal)` over all
    /// conductors, ×3 under positive sequence.
    pub loss_w: (f64, f64),
}

/// `(n, [(row, col, value)])` — the assembled, unfactored system Y as 0-based
/// coordinates, returned by [`Dss::system_y_csc`].
pub type SystemYCsc = (usize, Vec<(usize, usize, num_complex::Complex64)>);

/// A bus's short-circuit results after a FaultStudy solve — the dss-python
/// `Bus.Zsc1`/`Zsc0`/`Isc` surface (`TDSSBus`). `isc`/`vbus` are empty until a
/// FaultStudy has allocated the bus quantities.
#[derive(Debug, Clone)]
pub struct BusScView {
    /// User node numbers on the bus (`Nodes`).
    pub nodes: Vec<i32>,
    /// `Zsc1`: positive-sequence short-circuit impedance.
    pub zsc1: num_complex::Complex64,
    /// `Zsc0`: zero-sequence short-circuit impedance.
    pub zsc0: num_complex::Complex64,
    /// `Isc` / `BusCurrent`: per-node short-circuit current (= `Ysc · VBus`).
    pub isc: Vec<num_complex::Complex64>,
    /// The bus's stored `VBus` — the open-circuit (Voc) voltage captured by
    /// `UpdateVBus` during the study. Note this is **not** dss-python
    /// `Bus.Voltages`, which returns the live `NodeV` (after a FaultStudy that is
    /// the last `ComputeYsc` unit-injection residual, not the Voc).
    pub vbus: Vec<num_complex::Complex64>,
}

impl Dss {
    /// Snapshot every circuit element's terminal powers and currents in
    /// creation order (the oracle's `First/Next` order). Pascal
    /// `TDSSCktElement.GetPhasePower` / `ComputeIterminal`.
    pub fn snapshot_elements(&mut self) -> Vec<ElementSnapshot> {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_ref().expect("snapshot needs a circuit");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        let positive_seq = ckt.positive_sequence;
        // Per-terminal complex power below is formed as S = V*conj(I)
        // (`node_v[n] * i.conj()`): node voltage times the conjugate of the
        // terminal current -- the IEEE definition of complex power
        // (IEEE Std 1459-2010, 3.1.1.6, p. 5: S = P + jQ = V*I_conj; and
        // J. L. Willems, "The IEEE Standard 1459: What and Why?", sec. III-IV,
        // eq. (3), p. 2: P = V0*I0 + sum_k V_k*I_k*cos(phi_k)) -- with V and I
        // taken at the SAME frequency.
        //
        // In harmonics mode the engine solves each harmonic order h as an
        // independent per-frequency phasor network, V_bus^h = inv(Y_bus^h)*I_bus^h
        // (N.-C. Yang & Y.-W. Hsu, "OpenDSS-based Harmonic Power Flow Analysis for
        // Power Systems with Passive Power Filters", IEEE Access, 2023, sec. IV,
        // eq. (28)), so the meaningful terminal power is the per-harmonic complex
        // power S_h = V_h*conj(I_h) (IEEE 1459-2010, 3.1.2.5, p. 9:
        // P_H = V0*I0 + sum_{h!=1} V_h*I_h*cos(theta_h), where theta_h is "the
        // phase angle between the phasors V_h and I_h" -- the SAME order h for
        // both; cf. 3.1.2.4, p. 9: P1 = V1*I1*cos(theta_1)).
        //
        // A product mixing a voltage at one harmonic with a current at a different
        // harmonic, V_h*conj(I_{h'!=h}), is NOT a power: cross-frequency terms
        // appear only in the non-active instantaneous power p_q, whose average is
        // zero (IEEE 1459-2010, 3.1.2.2, pp. 8-9: the
        // 2*sum_n sum_{m!=n} V_m*I_n*sin(m*w*t-a_m)*sin(n*w*t-b_n) term; the
        // standard states p_q "does not represent a net transfer of energy (i.e.,
        // its average value is nil)"). The root reason is the orthogonality of the
        // harmonic (Fourier) basis over a fundamental period (W. M. Grady,
        // "Understanding Power System Harmonics", Apr. 2012, ch. 2,
        // eqs. (2.1)-(2.2), pp. 2-1..2-3).
        //
        // We therefore call `compute_iterminal` ONCE per element and immediately
        // form V*conj(I) from that single fresh terminal current, so V and I are
        // always the same frequency and the cross-frequency product can never
        // arise. This is a deliberate divergence from the pinned oracle, whose
        // `CktElement.Powers` returns V_h*conj(I_fundamental) when read AFTER
        // `CktElement.Currents` for a Generator/PVSystem/Storage in harmonics mode
        // (an order-dependent, stale-Iterminal engine bug we do NOT reproduce;
        // full analysis + IEEE-1459 proof live in the git-ignored
        // investigations/oracle-powers-currents-harmonic/).
        let mut out = Vec::with_capacity(ckt.ckt_elements.len());
        for &r in &ckt.ckt_elements {
            let class_name = classes[r.cls].props.class_name();
            let obj = &mut classes[r.cls].objects[r.idx];
            let name = format!("{}.{}", class_name, obj.data().name());
            let elem = obj
                .as_ckt_element_mut()
                .expect("ckt_elements refs are circuit elements");
            let yorder = elem.cd().yorder;
            let mut currents = vec![0.0; 2 * yorder];
            let mut powers = vec![0.0; 2 * yorder];
            // Powers (and Losses, below) model the oracle's `Get_Powers` /
            // `Get_Losses`, which route through the cache-aware `ComputeIterminal`;
            // Currents model the fresh `CktElement.Currents` (`GetCurrents`,
            // `CAPI_CktElement.pas`). The two `Iterminal` read paths agree after
            // every fixed-point / direct / harmonic solve — the cache is invalid
            // here so `compute_iterminal` recomputes fresh at the present `NodeV`,
            // and the single-frequency reasoning in the block comment above holds.
            //
            // TODO(compat): after a Newton solve they diverge. `DoNewtonSolution`'s
            // final `SumAllCurrents` stamps `Iterminal` at the pre-final voltage
            // guess `NodeV_{n-1}` (the `NodeV -= dV` update follows it), so the
            // cache-aware path (Powers/Losses) returns a one-step-stale current
            // while `GetCurrents` (Currents) recomputes at the converged `NodeV_n`
            // — a deterministic upstream quirk (`Vsource.pas` `GetCurrents` reads
            // `NodeV` directly, whereas `CktElement.pas` `Get_Powers`/`Get_Losses`
            // reuse `ComputeIterminal`). Clean fix: recompute `Iterminal` at
            // `NodeV_n` for all three reads.
            //
            // GATE NOTE: this staleness is the ONLY feature-sensitive signal that
            // distinguishes `algorithm=Newton` from the normal fixed-point on the
            // `newton.dss` / `newton_feeder.dss` gates — both algorithms converge to
            // the SAME voltages in the SAME iteration count, so only the Powers/
            // Losses channel (this cache split) diverges when Newton silently falls
            // back to `DoNormalSolution`. The de-compat pass that takes the clean
            // fix above MUST add a replacement Newton-specific assertion, else those
            // two decks stop verifying that Newton dispatch is wired at all.
            //
            // REMOVAL is NOT the usual "apply fix + regenerate goldens" (PORTING_PLAN
            // §6): this quirk is pinned by the ALWAYS-ON LIVE oracle (the corpus_live
            // `modes` gate — there is no golden for `newton*`), and the pinned
            // dss-python permanently reports the stale post-Newton Powers. So making
            // Powers fresh would make Rust DIVERGE from the live oracle (~4e-4 kW) →
            // gate red, unregenerable. Eliminating it is a de-compat DECISION, not a
            // local edit. Option (a) "bump the pinned oracle to a rev without the
            // bug" is RULED OUT: the EPRI channel confirms the quirk is present in
            // every vendored official rev incl. the latest — v9.8 (r3723), v10.2
            // (r4088), v11.0 (r4133), all fingerprint 0.478 kVA, checked 2026-07-08.
            // The remaining path (b): convert the `newton*` Powers/Losses compare to
            // a documented live-gate exclusion (Rust intentionally more correct than
            // the oracle, the VSConverter "gate around" pattern) plus the replacement
            // Newton assertion above.
            // See investigations/newton_stale_iterminal_bug_report.md.
            if elem.cd().enabled && !elem.cd().node_ref.is_empty() {
                elem.compute_iterminal(&sys, &node_v);
                let cd = elem.cd();
                for k in 0..yorder {
                    let n = cd.node_ref[k];
                    if n > 0 {
                        // S = V*conj(I) at the present (per-harmonic, in harmonics
                        // mode) solution frequency; see the block comment above.
                        let mut s = node_v[n] * cd.iterminal[k].conj();
                        if positive_seq {
                            // x3: balanced three-phase scaling of the single-phase
                            // power (Willems, "...What and Why?", sec. V.A, p. 3).
                            s *= 3.0;
                        }
                        powers[2 * k] = s.re * 0.001;
                        powers[2 * k + 1] = s.im * 0.001;
                    }
                }
            }
            // The element's own losses path (`Get_Losses`) — the same cache-aware
            // `ComputeIterminal` (stale after Newton), read BEFORE the fresh
            // currents refresh below so it reuses the powers-path cache.
            let loss = elem.losses(&sys, &node_v);
            // Currents: fresh recompute from the converged `NodeV` (oracle
            // `GetCurrents`), overwriting the `Iterminal` cache after Powers/Losses.
            if elem.cd().enabled && !elem.cd().node_ref.is_empty() {
                elem.refresh_iterminal(&sys, &node_v);
                let cd = elem.cd();
                for k in 0..yorder {
                    currents[2 * k] = cd.iterminal[k].re;
                    currents[2 * k + 1] = cd.iterminal[k].im;
                }
            }
            let cd = elem.cd();
            let bus_names = (1..=cd.nterms).map(|i| cd.get_bus(i).to_string()).collect();
            out.push(ElementSnapshot {
                name,
                enabled: cd.enabled,
                bus_names,
                powers,
                currents,
                loss_w: (loss.re, loss.im),
            });
        }
        // Under NCIM the swing source's reported terminal currents are the KCL
        // sum at its bus, not `YPrim·V - Iinj` (Pascal `TVsourceObj.GetCurrents`
        // takes the `NCIM_CalcInjCurrAtBus` branch when `Algorithm = NCIMSOLVE`
        // and `NodeRef[1] = 1`). Every connected element's `Iterminal` is now
        // fresh (refreshed in the loop above), so recompute the swing source's
        // currents/powers/losses from those and overwrite its snapshot entry.
        //
        // The generator overrides are needed for the same reason: after NCIM
        // adjusts a PV/converted generator's reactive power, its `YPrim` (`Yeq`)
        // is stale, so the general `YPrim·V - Iinj` reporting recompute no longer
        // cancels to `-conj(S/V)`. The NCIM solver computed the exact terminal
        // current (`-conj((Pnom + j·deltaQNom)/V)`); reproduce it here.
        if ckt.solution.algorithm == crate::solution::solution::NCIMSOLVE {
            let mut overrides: Vec<(ElemRef, Vec<num_complex::Complex64>)> = Vec::new();
            if let Some(o) = ncim_swing_source_currents(classes, ckt) {
                overrides.push(o);
            }
            overrides.extend(ncim_generator_currents(classes, ckt, &node_v));
            for (r, curr) in overrides {
                let name = format!(
                    "{}.{}",
                    classes[r.cls].props.class_name(),
                    classes[r.cls].objects[r.idx].data().name()
                );
                let Some(snap) = out.iter_mut().find(|s| s.name.eq_ignore_ascii_case(&name)) else {
                    continue;
                };
                let node_ref = classes[r.cls].objects[r.idx]
                    .as_ckt_element()
                    .expect("ncim override target is a ckt element")
                    .cd()
                    .node_ref
                    .clone();
                let mut loss = num_complex::Complex64::ZERO;
                for (k, &i) in curr.iter().enumerate() {
                    snap.currents[2 * k] = i.re;
                    snap.currents[2 * k + 1] = i.im;
                    let n = node_ref.get(k).copied().unwrap_or(0);
                    if n > 0 {
                        let mut s = node_v[n] * i.conj();
                        loss += s; // Get_Losses = Σ V·conj(I) over conductors (W/var)
                        if positive_seq {
                            s *= 3.0;
                        }
                        snap.powers[2 * k] = s.re * 0.001;
                        snap.powers[2 * k + 1] = s.im * 0.001;
                    } else {
                        snap.powers[2 * k] = 0.0;
                        snap.powers[2 * k + 1] = 0.0;
                    }
                }
                snap.loss_w = (loss.re, loss.im);
            }
        }
        out
    }

    /// Read a circuit element's dynamic/state variables — the dss-python
    /// `ActiveCktElement.AllVariableValues` surface (`TPCElement.GetAllVariables`).
    /// `name` is the element full name (`Class.name`, case-insensitive). Returns
    /// `None` if no such element exists; an empty vec for elements with no
    /// variables. The live f64 read the Monitor mode-3 f32 channel hides.
    pub fn element_variables(&mut self, name: &str) -> Option<Vec<f64>> {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_ref()?;
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        for class in classes.iter_mut() {
            let cn = class.props.class_name();
            for obj in class.objects.iter_mut() {
                let full = format!("{}.{}", cn, obj.data().name());
                if full.eq_ignore_ascii_case(name) {
                    let elem = obj.as_ckt_element_mut()?;
                    let n = elem.num_variables();
                    let mut states = vec![0.0; n];
                    elem.get_all_variables(&sys, &node_v, &mut states);
                    return Some(states);
                }
            }
        }
        None
    }

    /// WP8.5b corpus property parity: every property of the named element,
    /// rendered EXACTLY as the `?` executive query does (the choke-point
    /// `refresh_vterminal_if_marked` then [`ClassProps::get_value`] — the
    /// byte-proven WP8.5 Dump surface), as `(name, value)` pairs in
    /// property-index order (`1..=num_properties`). `full_name` is a `Class.name`
    /// (case-insensitive), resolved like [`Dss::do_query_cmd`] (no executive
    /// round-trip). `None` if no such element exists. The oracle side reads
    /// `Properties(p).Val` over `AllPropertyNames` (via `? name.prop`), so the two
    /// compare property-for-property.
    pub fn element_properties(&mut self, full_name: &str) -> Option<Vec<(String, String)>> {
        // Split `Class.name` exactly as `do_query_cmd` does (the @var-aware
        // splitter; a query name needs no other parser work).
        let (class_name, name) = {
            let mut p = Parser::new();
            parse_object_class_and_name(&mut p, &self.vars, full_name)
        };
        let &ci = self.class_by_name.get(&class_name.to_lowercase())?;
        if !self.classes[ci].set_active(&name) {
            return None;
        }
        let oi = self.classes[ci].active.expect("just set active");
        let n = self.classes[ci].props.num_properties();
        let mut out = Vec::with_capacity(n);
        for idx in 1..=n {
            // Same choke point `do_query_cmd` uses: reload Vterminal from the
            // solution for the properties that declare the need before rendering.
            self.refresh_vterminal_if_marked(ci, oi, Some(idx));
            let pname = self.classes[ci].props.property_name(idx).to_string();
            let value = self.classes[ci].props.get_value(
                self.classes[ci].objects[oi].as_ref(),
                idx,
                &self.enums,
            );
            out.push((pname, value));
        }
        Some(out)
    }

    /// AltDSS single-object JSON dump — Pascal `Obj_ToJSON_`
    /// (`CAPI_Obj.pas:762-784`). `full_name` is a `Class.name` (case-insensitive,
    /// `@var`-aware like [`Dss::element_properties`]); `None` if no such object
    /// exists. `opts` selects the sweep (default filled-only vs `Full`), the key
    /// naming, and the compact/pretty layout.
    ///
    /// The default sweep dumps only *set* properties, which are all pure reads of
    /// the parsed model — safe before any solve. `Full` additionally renders
    /// read-only function properties; the handful flagged `READS_VTERMINAL`
    /// (Transformer/AutoTrans `WdgCurrents`) read the element's `Vterminal`
    /// cache, so under `Full` after a solve they reflect whatever that cache last
    /// held. Unlike [`Dss::element_properties`] this `&self` method cannot run the
    /// `refresh_vterminal_if_marked` choke point; reproducing those solve-state
    /// strings byte-exactly is deferred (the "Full transformer WdgCurrents JSON"
    /// follow-up — the Stage-A goldens never render them, `skip_full`). No
    /// default-mode output is affected.
    pub fn obj_to_json(&self, full_name: &str, opts: JsonOpts) -> Option<String> {
        let (class_name, name) = {
            let mut p = Parser::new();
            parse_object_class_and_name(&mut p, &self.vars, full_name)
        };
        let &ci = self.class_by_name.get(&class_name.to_lowercase())?;
        let &oi = self.classes[ci].name_to_idx.get(&name.to_lowercase())?;
        let json = json_build::obj_to_json_data(
            &self.classes[ci].props,
            self.classes[ci].objects[oi].as_ref(),
            &self.enums,
            opts,
        );
        Some(json_serialize(&json, opts))
    }

    /// AltDSS class-batch JSON dump — Pascal `Batch_ToJSON` over every object of
    /// a class (the `IActiveClass.ToJSON` oracle surface, `CAPI_Obj.pas:1201-
    /// 1254`). `class` is the class name (case-insensitive); `None` if unknown.
    /// An empty class serializes to `[]`.
    pub fn class_batch_to_json(&self, class: &str, opts: JsonOpts) -> Option<String> {
        let &ci = self.class_by_name.get(&class.to_lowercase())?;
        let json = json_build::batch_to_json(
            &self.classes[ci].props,
            &self.classes[ci].objects,
            &self.enums,
            opts,
        );
        Some(json_serialize(&json, opts))
    }

    /// AltDSS whole-circuit JSON dump — Pascal `Obj_Circuit_ToJSON_`
    /// (`CAPI_Obj.pas:2513-2672`). Returns `None` when no circuit exists
    /// (`New circuit.` has not run). The circuit is **always** serialized pretty
    /// (`FormatJSON()`, indent 2), regardless of `opts.PRETTY`; `opts` selects the
    /// timestamp/bus/default-object filtering and the per-object sweep (default
    /// vs `Full`). Every embedded object is rendered by the same
    /// `obj_to_json_data` as [`Dss::obj_to_json`].
    pub fn circuit_to_json(&self, opts: JsonOpts) -> Option<String> {
        let ckt = self.circuit.as_ref()?;
        Some(json_circuit::circuit_to_json(
            ckt,
            &self.classes,
            &self.class_by_name,
            &self.enums,
            self.default_base_freq,
            self.default_earth_model,
            opts,
        ))
    }

    /// Read a bus's short-circuit results after a FaultStudy solve — the
    /// dss-python `Bus.Zsc1`/`Zsc0`/`Isc` surface. `name` is the bus name
    /// (case-insensitive). `None` if no such bus exists.
    pub fn bus_short_circuit(&self, name: &str) -> Option<BusScView> {
        let ckt = self.circuit.as_ref()?;
        let idx = ckt.bus_list.find(name)?;
        let b = &ckt.buses[idx];
        Some(BusScView {
            nodes: b.nodes.clone(),
            zsc1: b.get_zsc1(),
            zsc0: b.get_zsc0(),
            isc: b.bus_current.clone(),
            vbus: b.vbus.clone(),
        })
    }

    /// Read a monitor's recorded data — the dss-python `Monitors.Header` /
    /// `SampleCount` / `Channel(i)` / `dblHour` surface (tests/goldens).
    /// `name` may be `"m1"` or `"Monitor.m1"` (case-insensitive). `None` if no
    /// such monitor exists.
    pub fn monitor_view(&self, name: &str) -> Option<MonitorView> {
        let bare = name
            .strip_prefix("Monitor.")
            .or_else(|| name.strip_prefix("monitor."))
            .unwrap_or(name);
        for class in &self.classes {
            for obj in &class.objects {
                if let Some(m) = obj.as_any().downcast_ref::<monitor::Monitor>()
                    && m.med.cd.obj.name().eq_ignore_ascii_case(bare)
                {
                    let nch = m.num_channels();
                    return Some(MonitorView {
                        header: m.header().to_vec(),
                        sample_count: m.sample_count(),
                        dbl_hour: m.dbl_hour(),
                        channels: (1..=nch).map(|i| m.channel(i)).collect(),
                    });
                }
            }
        }
        None
    }

    /// An EnergyMeter's zone topology — the dss-python `Meters.AllBranchesInZone`
    /// / `AllEndElements` / `ZonePCE` / `CountBranches` surface (tests/goldens).
    /// `name` may be `"m1"` or `"EnergyMeter.m1"` (case-insensitive). `None` if
    /// no such meter exists.
    pub fn meter_zone(&self, name: &str) -> Option<MeterZoneView> {
        let bare = name
            .strip_prefix("EnergyMeter.")
            .or_else(|| name.strip_prefix("energymeter."))
            .unwrap_or(name);
        // Locate the meter, copy out its ElemRef lists, then resolve full names.
        let mut lists: Option<MeterZoneRefs> = None;
        for class in &self.classes {
            for obj in &class.objects {
                if let Some(em) = obj.as_any().downcast_ref::<energymeter::EnergyMeter>()
                    && em.data().name().eq_ignore_ascii_case(bare)
                {
                    lists = Some(MeterZoneRefs {
                        branches: em.sequence_list().to_vec(),
                        ends: em.zone_end_elements(),
                        pce: em.zone_pce().to_vec(),
                        register_names: em.register_names().to_vec(),
                    });
                }
            }
        }
        let lists = lists?;
        let full_name = |r: ElemRef| -> String {
            let cn = self.classes[r.cls].props.class_name();
            format!(
                "{}.{}",
                cn,
                self.classes[r.cls].objects[r.idx].data().name()
            )
        };
        Some(MeterZoneView {
            all_branches_in_zone: lists.branches.iter().map(|&r| full_name(r)).collect(),
            all_end_elements: lists.ends.iter().map(|&r| full_name(r)).collect(),
            zone_pce: lists.pce.iter().map(|&r| full_name(r)).collect(),
            register_names: lists.register_names,
        })
    }

    /// An EnergyMeter's register values paired with names — the dss-python
    /// `Meters.RegisterValues` / `RegisterNames` surface (tests/goldens).
    /// `name` may be `"m1"` or `"EnergyMeter.m1"` (case-insensitive).
    pub fn meter_registers(&self, name: &str) -> Option<Vec<(String, f64)>> {
        let bare = name
            .strip_prefix("EnergyMeter.")
            .or_else(|| name.strip_prefix("energymeter."))
            .unwrap_or(name);
        for class in &self.classes {
            for obj in &class.objects {
                if let Some(em) = obj.as_any().downcast_ref::<energymeter::EnergyMeter>()
                    && em.data().name().eq_ignore_ascii_case(bare)
                {
                    return Some(
                        em.register_names()
                            .iter()
                            .cloned()
                            .zip(em.registers().iter().copied())
                            .collect(),
                    );
                }
            }
        }
        None
    }

    /// A load's `(kWbase, FAllocationFactor)` by name — the oracle's
    /// `Loads.kW` / `Loads.AllocationFactor` (test API for `allocateloads`).
    pub fn load_alloc(&self, name: &str) -> Option<(f64, f64)> {
        for class in &self.classes {
            for obj in &class.objects {
                if let Some(ld) = obj.as_any().downcast_ref::<load::Load>()
                    && ld.data().name().eq_ignore_ascii_case(name)
                {
                    return Some((ld.kw_base, ld.allocation_factor()));
                }
            }
        }
        None
    }

    /// A generator's `(kWbase, kvarBase)` by name — the oracle's
    /// `Generators.kW` / `Generators.kvar` (test API for GenDispatcher
    /// redispatch).
    pub fn generator_kw_kvar(&self, name: &str) -> Option<(f64, f64)> {
        for class in &self.classes {
            for obj in &class.objects {
                if let Some(g) = obj.as_any().downcast_ref::<generator::Generator>()
                    && g.data().name().eq_ignore_ascii_case(name)
                {
                    return Some((g.kw_base, g.kvar_base));
                }
            }
        }
        None
    }

    /// The named generator's *present* (solved) `(kW, kvar)` output — Pascal
    /// `Get_PresentkW`/`Get_Presentkvar` (`Pnominalperphase`/`Qnominalperphase`
    /// times `nphases/1000`). Unlike [`Self::generator_kw_kvar`] (the input
    /// bases), this reflects the solved per-phase power, so under NCIM it tracks
    /// the PV-bus `deltaQNom` — the channel dss-python's `Generators.kvar` reads.
    pub fn generator_present_kw_kvar(&self, name: &str) -> Option<(f64, f64)> {
        for class in &self.classes {
            for obj in &class.objects {
                if let Some(g) = obj.as_any().downcast_ref::<generator::Generator>()
                    && g.data().name().eq_ignore_ascii_case(name)
                {
                    return Some((g.present_kw(), g.present_kvar()));
                }
            }
        }
        None
    }

    /// Test API for Pascal `TSensorObj.TakeSample`: drive the named sensor
    /// against the solved circuit and return its `(CalculatedCurrent,
    /// CalculatedVoltage)` per phase. (`TakeSample` is otherwise dead in the
    /// snapshot path — `SensorClass.SampleAll` is only invoked by the
    /// state-estimation API, which is a later phase — so this is the only gate
    /// that exercises the offset/`RotatePhases` math.)
    pub fn sensor_sample(
        &mut self,
        name: &str,
    ) -> Option<(Vec<num_complex::Complex64>, Vec<num_complex::Complex64>)> {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_ref()?;
        let sensor_ref = ckt.sensors.iter().copied().find(|r| {
            classes[r.cls].objects[r.idx]
                .data()
                .name()
                .eq_ignore_ascii_case(name)
        })?;
        let metered = classes[sensor_ref.cls].objects[sensor_ref.idx]
            .as_any()
            .downcast_ref::<sensor::Sensor>()?
            .metered_element()?;
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        let mut store = ClassStore { classes };
        let (s_obj, m_obj) = store.pair_mut(sensor_ref, metered);
        let m_ce = m_obj
            .as_ckt_element_mut()
            .expect("metered element is a circuit element");
        let s = s_obj
            .as_any_mut()
            .downcast_mut::<sensor::Sensor>()
            .expect("sensors holds Sensor objects");
        s.take_sample(m_ce, &sys, &node_v);
        let nph = s.med.cd.nphases;
        Some((
            s.med.calculated_current[..nph].to_vec(),
            s.med.calculated_voltage[..nph].to_vec(),
        ))
    }

    /// Per-transformer winding taps in creation order, keyed by name —
    /// mirrors the oracle's `Transformers` loop (`tr.Wdg = i; tr.Tap`).
    pub fn transformer_taps(&self) -> Vec<(String, Vec<f64>)> {
        let Some(ckt) = self.circuit.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(ckt.transformers.len());
        for &r in &ckt.transformers {
            let obj = &self.classes[r.cls].objects[r.idx];
            let tr = obj
                .as_any()
                .downcast_ref::<transformer::Transformer>()
                .expect("transformers list holds Transformers");
            // The oracle's `Transformers.First/.Next` walk
            // (`Generic_CktElement_Get_First/Next`) SKIPS disabled elements
            // unless `DSS_CAPI_ITERATE_DISABLED = 1` (default 0); mirror that, or
            // a deck that disables a transformer (e.g. `MakePosSequence`'s
            // off-phase-1 winding disable, `makeposseq_xfmr.dss`) compares one
            // extra tap row vs the oracle. Same rule as `regcontrol_tap_numbers`.
            if !tr.cd().enabled {
                continue;
            }
            let n = tr.num_windings() as usize;
            let taps = (1..=n).map(|w| tr.present_tap(w)).collect();
            out.push((obj.data().name().to_string(), taps));
        }
        out
    }

    /// Per-RegControl `TapNumber` in creation order (the oracle's
    /// `RegControls` API).
    pub fn regcontrol_tap_numbers(&self) -> Vec<(String, i32)> {
        let Some(ckt) = self.circuit.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for &r in &ckt.controls {
            let obj = &self.classes[r.cls].objects[r.idx];
            let Some(rc) = obj.as_any().downcast_ref::<reg_control::RegControl>() else {
                continue;
            };
            // The oracle's `RegControls.First/.Next` iterator SKIPS disabled
            // control elements (C-API `Get_First`/`Get_Next` walk the list with
            // `if pelem.Enabled`; verified live — a disabled RegControl yields
            // `First = 0`). Mirror that, or a deck that opens with
            // `BatchEdit RegControl..* enabled=False` compares 12 taps vs 0.
            if !rc.ccd.cd.enabled {
                continue;
            }
            // Pascal `Get_TapNum` reads the controlled transformer's *live*
            // `PresentTap[TapWinding]`; resolve it here so a direct
            // `Transformer.X.Taps=` edit (which bypasses the control's snapshot)
            // is reflected. Fall back to the cached `TapNum` if unresolved.
            let num = rc
                .controlled_ref()
                .and_then(|tref| {
                    // Either member of the Transformer/AutoTrans proxy.
                    transformer::as_controlled_transformer(
                        &*self.classes[tref.cls].objects[tref.idx],
                    )
                    .map(|tr| rc.tap_num_live(tr))
                })
                .unwrap_or_else(|| obj.get_i32(reg_control::prop::TAPNUM));
            out.push((obj.data().name().to_string(), num));
        }
        out
    }

    /// Per-capacitor `States` array in creation order (the oracle's
    /// `Capacitors` API — every Capacitor object, not just the shunt list).
    pub fn capacitor_states(&self) -> Vec<(String, Vec<i32>)> {
        let Some(cls) = self
            .classes
            .iter()
            .find(|c| c.props.class_name().eq_ignore_ascii_case("Capacitor"))
        else {
            return Vec::new();
        };
        cls.objects
            .iter()
            .map(|obj| {
                let states = obj
                    .get_i32_array(capacitor::prop::STATES)
                    .map(|s| s.to_vec())
                    .unwrap_or_default();
                (obj.data().name().to_string(), states)
            })
            .collect()
    }

    /// Terminal-1 closed flag of a capacitor by name (test API for the `Reset`
    /// controls path: `CapControl.Reset` drives the bank back to `InitialState`
    /// via `ControlledElement.Closed[0]`).
    pub fn capacitor_closed(&self, name: &str) -> Option<bool> {
        let cls = self
            .classes
            .iter()
            .find(|c| c.props.class_name().eq_ignore_ascii_case("Capacitor"))?;
        cls.objects
            .iter()
            .find(|o| o.data().name().eq_ignore_ascii_case(name))
            .and_then(|o| o.as_ckt_element())
            .map(|e| e.cd().all_conductors_closed())
    }

    /// CAPI `Circuit_Get_TotalPower`: the sum of every source's terminal-1
    /// power, in kW/kvar (negative of the power delivered to the circuit).
    pub fn total_power(&mut self) -> (f64, f64) {
        use num_complex::Complex64;
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_ref().expect("total_power needs a circuit");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        let positive_seq = ckt.positive_sequence;
        let mut total = Complex64::ZERO;
        for &r in &ckt.sources {
            let elem = classes[r.cls].objects[r.idx]
                .as_ckt_element_mut()
                .expect("sources are circuit elements");
            if !elem.cd().enabled || elem.cd().node_ref.is_empty() {
                continue;
            }
            elem.compute_iterminal(&sys, &node_v);
            let cd = elem.cd();
            // Pascal Get_Power(1): sum over terminal-1 conductors.
            let mut s = Complex64::ZERO;
            for i in 0..cd.nconds {
                let n = cd.node_ref[i];
                if n > 0 {
                    s += node_v[n] * cd.iterminal[i].conj();
                }
            }
            if positive_seq {
                s *= 3.0;
            }
            total += s;
        }
        (total.re * 0.001, total.im * 0.001)
    }

    /// CAPI `Circuit_Get_Losses`: total circuit losses in W/var (sum over
    /// enabled PD elements).
    pub fn losses(&mut self) -> (f64, f64) {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_mut().expect("losses needs a circuit");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let mut store = ClassStore { classes };
        let total = ckt.losses(&mut store, &sys);
        (total.re, total.im)
    }

    /// `DSS.ActiveCircuit.Solution.EventLog`: the accumulated event-log lines
    /// (empty when no circuit exists). The control loop (WP5.7) and the
    /// controls' `AppendToEventLog` (WP5.5/5.6) populate it.
    pub fn event_log(&self) -> &[String] {
        match &self.circuit {
            Some(ckt) => ckt.solution.event_log.entries(),
            None => &[],
        }
    }

    /// The pending control-action queue as the dss-python `CtrlQueue.Queue`
    /// rows (Pascal `TControlQueue.QueueItem`, `ControlQueue.pas:557`:
    /// `Format('%d, %d, %.9g, %d, %d, %s ', [handle, hour, sec, code, proxy,
    /// ControlElement.Name])` — bare device name, trailing space). Empty after
    /// a drained snapshot; time/dynamics modes leave future-scheduled actions
    /// (e.g. recloser reclose shots) pending between steps.
    pub fn control_queue_rows(&self) -> Vec<String> {
        let Some(ckt) = &self.circuit else {
            return Vec::new();
        };
        ckt.solution
            .control_queue
            .queue_rows()
            .into_iter()
            .map(|(handle, hour, sec, code, proxy, ctrl)| {
                let name = self.classes[ctrl.cls].objects[ctrl.idx].data().name();
                format!(
                    "{handle}, {hour}, {}, {code}, {proxy}, {name} ",
                    crate::util::fmt_g(sec, 9)
                )
            })
            .collect()
    }

    /// Coordinate dump of the **assembled, unfactored** system Y matrix:
    /// `(n, [(row, col, value)])`, 0-based, where row `i` corresponds to node
    /// `i + 1` (so `node_name(row + 1)` names the row). The values are
    /// pre-equilibration — the matrix exactly as stamped from element YPrims —
    /// so they line up with the oracle's `YMatrix.getYSparse(factor=False)`.
    /// `None` if no system Y has been built. Test/golden API (the assembled-model
    /// checkpoint of `golden_checkpoints.rs`).
    ///
    /// Reads the **active** handle (`Solution.active_y`), modelling Pascal's `hY`
    /// pointer: `BuildYMatrix` sets `hY := hYsystem` for `WHOLEMATRIX` and `hY :=
    /// hYseries` for `SERIESONLY`/`PDE_ONLY` (`YMatrix.pas` l.394/402), and
    /// `getYSparse` reads that pointer. So after an NCIM solve (`PDE_ONLY`) the
    /// reported system Y is the PDE-only network Y — no load `Yeq` — exactly as
    /// the oracle reports it.
    pub fn system_y_csc(&mut self) -> Option<SystemYCsc> {
        use crate::solution::solution::ActiveY;
        let ckt = self.circuit.as_mut()?;
        let y = match ckt.solution.active_y {
            ActiveY::System => ckt.solution.y_system.as_mut(),
            ActiveY::Series => ckt.solution.y_series.as_mut(),
        }?;
        let n = y.size();
        let (rows, cols, vals) = y.coo_entries().ok()?;
        let coords = rows
            .into_iter()
            .zip(cols)
            .zip(vals)
            .map(|((r, c), v)| (r, c, v))
            .collect();
        Some((n, coords))
    }

    /// An element's primitive admittance matrix `Yprim` as a **column-major**
    /// `yorder × yorder` flat array (`out[col * yorder + row]`) — the exact
    /// layout of the oracle's `CktElement.Yprim` (Pascal `TcMatrix`, column-major)
    /// and of `CMatrix`'s own storage, so the two compare without any transpose.
    /// `name` is a full `Class.name` (e.g. `"Transformer.reg1"`) when it
    /// contains a dot, else a bare object name matched across all classes
    /// (case-insensitive). `None` if no such element exists or it has no Yprim.
    /// Test/golden API (the selected-element checkpoint of `golden_checkpoints.rs`).
    pub fn element_yprim(&self, name: &str) -> Option<(usize, Vec<num_complex::Complex64>)> {
        let want_full = name.contains('.');
        for class in &self.classes {
            let cn = class.props.class_name();
            for obj in &class.objects {
                let matches = if want_full {
                    format!("{}.{}", cn, obj.data().name()).eq_ignore_ascii_case(name)
                } else {
                    obj.data().name().eq_ignore_ascii_case(name)
                };
                if !matches {
                    continue;
                }
                let Some(ce) = obj.as_ckt_element() else {
                    continue;
                };
                let cd = ce.cd();
                let yorder = cd.yorder;
                let yprim = cd.yprim.as_ref()?;
                let mut out = Vec::with_capacity(yorder * yorder);
                for col in 0..yorder {
                    for row in 0..yorder {
                        out.push(yprim.get(row, col));
                    }
                }
                return Some((yorder, out));
            }
        }
        None
    }

    /// The node injection-current vector the solver last used (`Solution.Currents`,
    /// the RHS of `Y·V = I`), length `num_nodes + 1` with slot 0 = ground —
    /// the oracle's `YMatrix.getI()` surface. Empty when no circuit exists.
    /// Test/golden API.
    pub fn node_injection_currents(&self) -> Vec<num_complex::Complex64> {
        match &self.circuit {
            Some(ckt) => ckt.solution.currents.clone(),
            None => Vec::new(),
        }
    }
}

/// Pascal `TVsourceObj.NCIM_CalcInjCurrAtBus` (`PCElements/vsource.pas` l.1225):
/// the swing source's NCIM-reported terminal currents. NCIM holds the swing bus
/// at the ideal EMF, so `YPrim·V - Iinj` is ~0 there; instead the source's
/// terminal current is the Kirchhoff sum at its bus — **minus** every connected
/// PD-element terminal current, **plus** every other connected PC-element terminal
/// current. Returns `(swing-source ElemRef, its yorder-long terminal-current
/// vector)` — the swing source is the [`VSource`] whose first node is the global
/// slack (`NodeRef[0] == 1`) — or `None` if there is none.
///
/// Reads each connected element's `Iterminal` cache, which
/// [`Dss::snapshot_elements`] has just refreshed at the converged `NodeV` (Pascal
/// recomputes each `ce.GetCurrents` fresh; the values are identical). PD elements
/// use the Pascal `Round(Yorder/2)` conductors-per-terminal stride (its 2-terminal
/// assumption); PC elements use `NPhases`.
fn ncim_swing_source_currents(
    classes: &[DssClass],
    ckt: &Circuit,
) -> Option<(ElemRef, Vec<num_complex::Complex64>)> {
    use num_complex::Complex64;

    // The swing VSource: a source whose first node is the global slack node 1.
    let src_ref = ckt.sources.iter().copied().find(|&r| {
        let obj = &classes[r.cls].objects[r.idx];
        obj.as_any()
            .downcast_ref::<crate::elements::pc::vsource::VSource>()
            .is_some()
            && obj
                .as_ckt_element()
                .is_some_and(|ce| ce.cd().node_ref.first() == Some(&1))
    })?;

    let src_cd = classes[src_ref.cls].objects[src_ref.idx]
        .as_ckt_element()?
        .cd();
    let src_bus = src_cd.terminals.first()?.bus_ref;
    let nphases = src_cd.nphases;
    let yorder = src_cd.yorder;
    let mut curr = vec![Complex64::ZERO; yorder];

    // The 0-based terminal of `cd` connected to the source bus, if any (Pascal
    // `BusName = StripExtension(ce.GetBus(j))`).
    let my_term = |cd: &crate::elements::ckt::CktElementData| -> Option<usize> {
        (0..cd.nterms).find(|&t| cd.terminals.get(t).map(|x| x.bus_ref) == Some(src_bus))
    };

    // TODO(compat): the `+ 1` on every `iterminal` index below reproduces an
    // upstream off-by-one. Pascal `NCIM_CalcInjCurrAtBus` fills a **0-based**
    // dynamic `ElmCurrents: array of Complex` via `ce.GetCurrents`, then indexes
    // it as `ElmCurrents[(myTerm*stride) + j]` with `j := 1..NPhases` — a 1-based
    // index into a 0-based array, so it reads each connected element's conductor
    // shifted by one (the swing source's reported phase-A current is actually the
    // negated phase-B branch current, etc.; the last read lands on the array's
    // unwritten, zero-initialized tail slot). It is deterministic and in-range
    // (never OOB — `SetLength(ElmCurrents, Yorder+1)`), so — per CLAUDE.md — it is
    // reproduced 1:1 (the `.get(..).map` returns 0 for the tail slot the port's
    // `Yorder`-length `iterminal` lacks, matching the zero slot). Clean fix: drop
    // the `+ 1`. NCIM affects only the *reported* swing-source current; node
    // voltages are unaffected.
    let clip = |v: Option<&num_complex::Complex64>| v.copied().unwrap_or(Complex64::ZERO);

    // PD elements (+ faults) at the bus: subtract their terminal currents. Pascal
    // stride `Round(ce.Yorder / 2)` (its 2-terminal conductors-per-terminal).
    for &r in ckt.pd_elements.iter().chain(ckt.faults.iter()) {
        let Some(ce) = classes[r.cls].objects[r.idx].as_ckt_element() else {
            continue;
        };
        let cd = ce.cd();
        if !cd.enabled {
            continue;
        }
        let Some(t) = my_term(cd) else { continue };
        let stride = ((cd.yorder as f64) / 2.0).round() as usize;
        for (i, c) in curr.iter_mut().enumerate().take(nphases) {
            *c -= clip(cd.iterminal.get(t * stride + i + 1));
        }
    }
    // PC elements (+ other sources) at the bus, excluding the source itself: add
    // their terminal currents (stride `ce.NPhases`).
    for &r in ckt.pc_elements.iter().chain(ckt.sources.iter()) {
        if r == src_ref {
            continue;
        }
        let Some(ce) = classes[r.cls].objects[r.idx].as_ckt_element() else {
            continue;
        };
        let cd = ce.cd();
        if !cd.enabled {
            continue;
        }
        let Some(t) = my_term(cd) else { continue };
        for (i, c) in curr.iter_mut().enumerate().take(nphases) {
            *c += clip(cd.iterminal.get(t * cd.nphases + i + 1));
        }
    }
    Some((src_ref, curr))
}

/// NCIM-reported terminal currents for every generator the NCIM solver touched
/// (Pascal `NCIM_UpdateGenQ` l.772: `Iterminal[j+1] := -cong(cmplx(Pnom,
/// deltaQNom[j])/V)`). After NCIM adjusts a generator's reactive power its
/// `YPrim`/`Yeq` is stale, so the general `YPrim·V - Iinj` reporting recompute
/// (which [`Dss::snapshot_elements`]'s main loop applies) no longer collapses to
/// `-conj(S/V)`; this reproduces the exact terminal current the solver computed.
/// Returns `(generator ElemRef, its yorder-long terminal-current vector)` for
/// every enabled generator carrying NCIM state (`delta_q_nom` non-empty).
fn ncim_generator_currents(
    classes: &[DssClass],
    ckt: &Circuit,
    node_v: &[num_complex::Complex64],
) -> Vec<(ElemRef, Vec<num_complex::Complex64>)> {
    use crate::elements::pc::generator::Generator;
    use num_complex::Complex64;

    let mut out = Vec::new();
    for &r in &ckt.generators {
        let obj = &classes[r.cls].objects[r.idx];
        let Some(g) = obj.as_any().downcast_ref::<Generator>() else {
            continue;
        };
        if !g.cd.enabled || g.delta_q_nom.is_empty() {
            continue;
        }
        let mut curr = vec![Complex64::ZERO; g.cd.yorder];
        let p = g.p_nominal_per_phase;
        for (j, c) in curr.iter_mut().enumerate().take(g.cd.nphases) {
            let nr = g.cd.node_ref[j];
            if nr == 0 {
                continue;
            }
            let q = g.delta_q_nom.get(j).copied().unwrap_or(g.delta_q_nom[0]);
            *c = -(Complex64::new(p, q) / node_v[nr]).conj();
        }
        out.push((r, curr));
    }
    out
}
