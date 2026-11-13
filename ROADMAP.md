# Roadmap

Where dss-rs is going. Rewrite this file when a milestone lands or the direction changes: a
landed milestone becomes one line under **Done**, the rest stays where it is. No dates, no
status lines, no history: `git log` holds that.

## Direction
dss-rs grows into a suite for the studies of power-system design: short circuit by IEC 60909 and
by GOST, protection coordination, arc flash, motor starting and electromagnetic transients (the
transient recovery voltage of circuit breakers first), on one project model and with a GUI.
Each study follows its standards, and their worked examples test it. The DSS language stays one
way into a project, and the OpenDSS engines check that part.

## Stage
The engine reads the DSS language and solves the network: snapshot and time-series power flow,
harmonics, dynamics, fault studies, A-Diakoptics, reliability, with the OpenDSS reports and
exports. The OpenDSS test corpus checks it case by case against two OpenDSS engines, and every
divergence is a ledger entry with a test (`tests/corpus/`). The work now makes the engine robust
on any input.

## Done
- The DSS language with every element class and solution mode of OpenDSS r4133, its reports and
  exports (text, JSON, CIM).
- The corpus gate on two OpenDSS channels: dss_capi 0.14.5 through the pinned dss-python, and
  the EPRI r4133 binary.
- Two builds: the product, and a precision-compat build that matches the OpenDSS goldens byte
  for byte (`dss-compat`).
- User-written models run as sandboxed WebAssembly (`dss-usermodel`), not as native libraries.
- A-Diakoptics: graph partitioning (`dss-metis`), tearing and the torn solve.
- An engine instance moves between threads (`Send`), and benchmarks of the solver and the Y
  build exist (`crates/dss-core/benches`).

## Now: hardening
- Ownership and structure by `CLAUDE.md` C2 and C5, with priority over the rest of this stage:
  each piece of state has one owner, and an operation is a method of the smallest type that
  holds its data. Today types gather the operations of their parts, functions stand free over
  the whole circuit, and borrow conflicts are worked around with copies and values moved out and
  back. A target architecture is designed first, and the code then moves to it.
- Data access that cannot read stale or foreign state, so a forgotten check is a compile error.
  Objects are stored apart from their class descriptions, one typed vector per class.
- No panic on any input. An error is a result, and input numbers are checked where they enter:
  the DSS script and the files it names.
- The DSS parser is rewritten and moved into code of its own.
- Tests pin the engine's own behaviour. Docs and comments state what the code does, briefly.

## Next
1. **Own goldens.** The goldens become snapshots of this engine. The precision quirks and the FPC
   print kernels leave, and with them the parity build (`dss-compat`): the gate runs one lane.
2. **The project model and short circuit by IEC 60909.** A typed model of the project that every
   study reads: the network, the equipment data the studies need beyond the DSS language, the
   diagram. IEC 60909-0 is its first study and decides what it holds first. DSS and CIM (CGMES)
   are ways into it and out of it.
3. **Short circuit by GOST**: ГОСТ Р 52735 above 1 kV, ГОСТ 28249 below, on the same model.
4. **GUI**: the single-line diagram, editing the project model, the results on the diagram.
5. **Protection coordination**: protective devices with their settings and time-current curves
   (IEC 60255-151, IEEE C37.112), selectivity checked on the short-circuit currents.
6. **Arc flash** by IEEE 1584, from the short-circuit currents and the clearing times of the
   protection.
7. **Motor starting**, static and dynamic (IEEE 3002.7).
8. **Electromagnetic transients.** A time-domain solver beside the phasor one: lumped and
   distributed-parameter lines, switches and circuit-breaker models, modelled as IEC TR 60071-4
   describes. Its first study is the transient recovery voltage of a circuit breaker against the
   envelopes of IEC 62271-100 (application: IEC TR 62271-306).
9. **Multithreading.** An engine per thread, parallel work inside one solve, then parallel
   sparse factorization. It builds on the object storage of hardening.
10. **Resonance.** Iterative refinement of the solution, resonance analysis, diagnostics.
11. **A-Diakoptics in full.** Aggregate profiles, and the torn subnetworks solved on threads once
    a solve can run in parallel.
12. **The next OpenDSS version.** The 0.15-only properties stop being hidden, together with the
    `Line.Wires` name the exports still give as `Conductors`.
