# Roadmap

Where dss-rs is going. Rewrite this file when a milestone lands or the direction changes: a
landed milestone becomes one line under **Done**, the rest stays where it is. No dates, no
status lines, no history: `git log` holds that.

## Stage
The engine reads the DSS language and solves the network: snapshot and time-series power flow,
harmonics, dynamics, fault studies, A-Diakoptics, reliability, with the OpenDSS reports and
exports. The vendored OpenDSS test corpus checks it case by case against two OpenDSS engines,
and every divergence is a ledger entry with a test (`tests/corpus/`). The work now makes the
engine robust on any input, then makes it faster and extends it past OpenDSS.

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
- No panic on any input. An error is a result, and input numbers are checked where they enter:
  the DSS script and the files it names.
- Data access that cannot read stale or foreign state, so a forgotten check is a compile error.
  Objects are stored apart from their class descriptions, one typed vector per class.
- Tests pin the engine's own behaviour. Docs and comments state what the code does, briefly.
- The known defects below are fixed first.

## Known defects
- A solve with NaN voltages reads as converged.
- The IndMach012 admittance in dynamics and harmonics ignores the machine's connection.
- After a bus rebuild the node vectors can be shorter than the bus list.
- A Monitor keeps the shape its element had when the Monitor was last edited.
- Save and the JSON export drop `bus2` of a Capacitor or Reactor made with `like=`.
- A huge `phases=` aborts the process on allocation instead of being refused.

## Next
1. **Multithreading.** An engine per thread, parallel work inside one solve, then parallel
   sparse factorization. It builds on the object storage of hardening.
2. **Resonance.** Iterative refinement of the solution, resonance analysis, diagnostics.
3. **A-Diakoptics in full.** Aggregate profiles, and the torn subnetworks solved on threads once
   a solve can run in parallel.
4. **Own goldens.** The goldens become snapshots of this engine, and the FPC print kernels leave
   both builds.
5. **The next OpenDSS version.** The 0.15-only properties stop being hidden, together with the
   `Line.Wires` name the exports still give as `Conductors`.
