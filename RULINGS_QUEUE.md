# User rulings of 2026-10-04 and the work queue they create

Each step below runs as its own workflow, and every step that needs a plan runs the
same stages (user decisions of 2026-10-04):

1. plan, with the work cut into parts one agent each can finish
2. plan review: findings only, the reviewer edits nothing
3. plan revise by another agent
4. plan check by a third, fresh agent
5. executor parts in sequence, a part that runs out of context hands the rest over
6. the full gate
7. code audit and test audit in parallel, read-only, several agents per role when the
   change is larger than 1500 lines
8. settlement in parts, the full gate
9. code audit and test audit of the settlement, a second settlement when they find
   something, and a fresh audit of that
10. landing by hand as one squash commit

Every OpenDSS bug a step fixes gets an upstream report under `investigations/to_opendss`.
At most three steps run at once. Steps 2 to 4 were launched on a shorter shape and get
the missing stages after their workflow ends: plan audit, plan revise, plan check, then
stages 7 to 10.

## Rulings

1. **MakePosSequence matrices.** A line with more than three conductors reduces its
   `rmatrix`, `xmatrix` and `cmatrix` with the same formula as a three-conductor line:
   the mean of the diagonal minus the mean over all pairs. Side finds on the path are
   fixed in the same step.
2. **`NVariables` of a DynamicExp** defaults to 20.
3. **A rating that is not set** is its own value, not a number. It prints `none`, input
   accepts `none` and `-1`, Save omits it, and Export Capacity shows 0 % for an unrated
   Line.
4. **LineGeometry.**
   1. `cond=` may come in any order.
   2. Conductor data (`wire=`, `cncable=`, `tscable=`, `x=`, `h=`) without a selected
      conductor is an error. Only an explicit `cond=N` selects one. After `New` and
      after `like=` none is selected. A one-conductor geometry is no exception.
   3. `units=` before the first `cond=` sets the default only and is written into no
      conductor.
   4. `units=` after `cond=N` belongs to conductor N wherever it stands before the next
      `cond=`, and becomes the default for conductors selected later without their own.
   5. Line breaks and `~` mean nothing. One line and several lines read the same.
   6. `cond=` out of range is an error and leaves no conductor selected.
   7. A conductor without data gives one clear error at calculation, naming it.
   8. A unit is never "not set": without a default it is feet. Nothing is read as
      metres silently.
5. **An element that joins a running dynamics run** (created after `Set mode=dynamic`,
   enabled mid-run, enabled again after a disable, phase count changed) makes `Solve`
   refuse before the step. The message names the element and the remedy
   (`set mode=snap`, `solve`, `set mode=dynamic`).
6. **A disabled element in dynamics** is out of the model. It is not integrated, not
   asked for a refusal and not stamped. Its dynamic quantities read as zero (current,
   power, state variables in a mode-3 monitor and in `Show Variables`), finite, never
   NaN. Nominal data and properties stay. Per class the plan separates states from
   settings in the variable list, and a doubtful boundary goes to the user.
7. **Storage in dynamics** gets a full model, no stub: charging and idling run through
   a real current loop, and a discharging unit starts from its power-flow current
   instead of zero.
8. **The mode-3 monitor refusal** reads
   `Monitor.m3: mode 3 records state variables and needs a power conversion element. Line.l1 is not one.`
9. **A meter whose binding fails** is not created. A later edit that would break the
   binding is rejected and the old binding stays. A meter never adds a node to the
   circuit. A valid monitor on a disabled element is not an error and records zeros.
10. **`Open` on a Generator in dynamics** gives the same result as an opened line in
    series with it: zero power, the network as without the generator, the rotor
    accelerating by the swing equation. Measured on all three engines for the series
    line: 60.0125, 60.0375, 60.0625 Hz at 1 ms steps (1500 kW, 1800 kVA, H = 1 s).
    The sibling classes are covered by the same step.
11. **RF-I00-08** is deferred to the end of the plan.
12. **The DSS language specification is the OpenDSS documentation** (2026-10-04,
    `Version8/Distrib/Doc/OpenDSS Documentation.chm` of the r4133 trunk). Input that
    breaks it and that OpenDSS accepts in silence is an OpenDSS bug: the engine refuses
    it, and that is no divergence in the language. For `DynOut` the documentation says a
    generator "requires 2 outputs", speed then angle, "in the same order", so the engine
    refuses an unknown name, more than two names, one name twice and one name for a
    machine. Syntax the documentation does not have (a named `DynOut` form) breaks the
    specification and is not added. No order heuristic, no new warning level.
13. **A host whose equation omits `NVariables`** lists only the named variables
    (2026-10-04).
14. **Rating details** (2026-10-04, refines ruling 3).
    1. A rating holds one of three states: a valid number, a typed number that is no
       rating (`0` or a negative), and `none`. Calculations and reports read the last
       two as not set and print `none`.
    2. Save writes what was typed: `none` and `-1` as `-1` (OpenDSS refuses the word
       `none` in a number, measured on the pinned dss-python), another typed number as
       typed. Nothing typed, nothing written.
    3. A typed value is never overwritten by a derived one.
    4. A Line whose conductors are partly rated is an error that names the conductor.
       All unrated gives not set.
    5. A derived zero (a Capacitor without kvar) is not set.
    6. The kVA ratings of transformers and the ratings of protective devices follow the
       same rules, each in its own step.
    7. `Set %Normal` stays. `none` stays in the overload CSV, CIM omits the rating, JSON
       writes `null`.
    8. After `Set %Normal` (2026-10-10): `?` reads the live rating, because the
       documentation says the in-memory value changes, and Save writes the number
       the user typed, because the change holds only for the duration of the run.
       One rule for every class, a typed Line and a default Line alike.
    9. An input that makes a stamp or a derived rating of an element non-finite
       (2026-10-10, Q12 of step 2): the element is refused at its recalc with a
       message naming the element, the property and the value, and it is not built.
       A Capacitor `kv=0 kvar=100` has an infinite capacitance, so the rating is
       only the symptom. A rated voltage of zero breaks the documentation, so the
       refusal is no divergence. Never read such a rating as not set and never keep
       it. One rule for every class with a rating derived from `kv` (Capacitor,
       Reactor, Load, Generator, Transformer, Storage and the rest). The planner of
       step 10 collects the inputs and classes the rule covers. Known routes beside
       `kv=0` (code audit of step 2 settle 3): a typed non-finite kVA rating
       (`NormHkVA`, `EmergHkVA`, the kVA `Ratings` of Transformer, AutoTrans and
       XfmrCode, which today print `NaN` in `Export Capacity` and write
       `NormHkVA=+Inf` into a Save deck that does not reload), the `RatedCurrent`
       of the protective devices, a conductor `normamps=1.7e308` whose 1.5-times
       emergency rating overflows to `+Inf`, and a finite `Set %Normal=1e10` on
       `emergamps=1e308`. Each of these is refused by the same rule.
15. **The open questions of steps 3 and 4** are decided by a research workflow: physics
    with standards and textbooks for MakePosSequence, the documentation and the needs
    of a future language server for LineGeometry. The verdicts come back to the user.
16. **Research verdicts taken** (2026-10-04, the user delegated the choice: as close to
    physics as possible). Rating, closing ruling 14: a typed negative other than `-1` is
    stored, read as not set and raises no error.
    MakePosSequence:
    1. An element becomes the positive-sequence self term of what it stamps on its real
       nodes, `Y1 = (1/3) v^H Y v` with `v = (1, a^2, a)` on the phases it touches and 0
       on ground. Rated power divides by 3 for every phase count.
    2. A one-phase series element: impedance times 3, capacitance divided by 3. A
       two-phase Line: `Z1 = (3/4)(Z11 + Z22 - Z12)`, the same balanced-voltage
       excitation as every other element. A series Reactor or Capacitor takes the
       impedance form of the Line for every phase count: the form follows the node map
       of ruling 17, not the class.
    3. Replaced by ruling 17. A transposition section and a shared neutral are no
       longer refused. A deck with nine cores on nodes `1.2.3.1.2.3.1.2.3` stays the pin.
    4. A delta Reactor matrix holds the leg quantities. A delta Capacitor `cmatrix` is
       the nodal matrix the documentation names, `C1 = (1/3) v^H C v`, for three phases
       and more. For one and two phases it stays refused.
    5. A Reactor given one of `rmatrix`, `xmatrix`: the other part is zero, no message.
    6. A two-phase Vsource has its poles 180 degrees apart and becomes one phase of the
       pole voltage behind `1.5 (Zs - Zm)`. A one- or two-phase Fault and a non-passive
       `gmatrix` have no positive-sequence form and stop the command (ruling 17.13).
    7. `normamps` and `emergamps` of a one- or two-phase Line scale by N/3. Merged
       parallel cores are rated at the smallest of rating over current share.
    8. A matrix whose order is not the phase count is refused at the solve. Dump, `?`
       and Save print it at its stored order.
    9. On a circuit already reduced the command changes nothing. A `phases=2` coupled
       element is never read as a two-phase lateral. A one-phase Vsource keeps its kV
       and its impedance follows item 2.
    10. A delta Capacitor or Reactor has N + 1 conductors for N = 1 and 2 and N for
        N >= 3, in either order of `conn=` and `phases=`.
    LineGeometry:
    11. Without a selected conductor, and for an `x` or `h` never given, `x` and `h`
        read empty. `cond` reads 0.
    12. A unit is one exact documented word, `mm` included. No prefixes.
    13. `like=` copies the default unit. Equivalent distances store their own unit.
    14. A property before the first `nconds=` is an error. Every `nconds=` clears the
        selection and returns the default unit to feet.
    15. A conductor without `x` or `h` is the rule 7 error, also in a one-conductor
        geometry. Save writes `X=` and `h=` only when given.
    16. `wires=` takes its count rule from the data, never from the selection.
    17. The help text of `cond` states that there is no default.
    18. The probe decks stay out of the corpus. One pin holds a capacitance value.
    19. (coordinator, 2026-10-10, under the delegation "вопросы ко мне решай сам") The
        earth-return depth constant of the simple Carson model stays at the engine's
        658.8530451057239 sqrt(rho/f) in this step: Carson's series gives 658.8716, the
        gap is 2.8e-5 relative, and the change would move every Line golden in both
        lanes. It is a precision item for the parity teardown, with its own step then.
        The code comment no longer calls the constant the precise value.
    20. (coordinator, 2026-10-10, same delegation) The tape-shield resistance keeps the
        engine's lap correction sqrt((100 - lap)/50) in this step. At lap = 100 the factor
        is 0 and the resistance vanishes, which no physics gives, and Kersting's
        rho/(pi d T) has no lap factor, so the correct factor is derived in its own
        queue step (step 19) from the geometry of a lapped tape, measured on both
        oracles, and reported upstream if the engine's factor is wrong. The LineGeometry
        pins hold the unit placement under the engine's tape model and do not depend on
        the factor.
    21. (coordinator, 2026-10-10, same delegation) The nine open questions of the
        MakePosSequence row of DIVERGENCES.md (step 3) are not put to the user. Question
        7 keeps the I²R label of a Parallel=yes matrix Reactor as built, since the
        branch carries the terminal current and its loss is I²R by meaning, no-load only
        after the reduction writes it as rp. Questions 1 to 6, 8 and 9 are answered by
        rulings 16 and 17 and by the MakePosSequence plan (MAKEPOSSEQ_PLAN.md), which
        states each with its candidates and numbers. The choices marked taken in the row
        stand until that plan lands them.
    22. (coordinator, 2026-10-10, same delegation, from the fresh audit of settle 4) Three
        items it left are not put to the user. The delta and cmatrix filter stamp of the
        Capacitor scales the leg diagonal by 1.000001 before inverting, which both oracles
        do too, so the bank draws 6.7e-7 more than physics and gains a zero-sequence shunt
        of 1.29e-9 S. The exact form Y_d (I + zl Y_d)^-1 replaces it in its own queue step
        (step 20) with a pin against the wye equivalent, since no corpus deck reaches the
        arm. The reading of a phase-to-phase reactor after the reduction (total 0 with a
        negative I²R on both oracles) belongs to the MakePosSequence plan with the other
        sources of the row. The Exclusion paragraph of the row says no golden holds the
        rp reactor where tests/golden/props/reactor.json reads back one without a solve,
        so the first plan step that touches the row writes "reads the loss of".
17. **One procedure for every MakePosSequence matrix question** (2026-10-04, two
    research runs: literature, derivation, DSS representation, a skeptic each. The
    choices in items 6, 12 and 13 and in 16.4, 16.6, 16.7 were taken by the coordinator
    under the delegation of ruling 16 and are open to the user).
    1. Principle. The reduced circuit keeps the complex power of every balanced state
       `V = W U`. `W` holds the true phasor of every port node. No class has a rule of
       its own.
    2. Node meaning. Node 0 is earth. Line conductors and transformer windings carry
       phasors from the source, conductor by conductor, and a winding adds the
       displacement and polarity of its connection. A source's own angle never enters
       the map.
    3. Lumped conductors. A node without a phasor that two-terminal conductors of
       Reactors, Capacitors, Faults, Loads and Generators reach with one phasor becomes
       a port node of that phasor. A node they reach with different phasors, and every
       node left over, is an internal node.
    4. Ports. Phase nodes bound by one multi-phase terminal are one port. Other phase
       nodes follow the conductor order of the Line that reaches them, three by three.
       A port holds each phasor once.
    5. Internal nodes obey the current law and are eliminated with the passive elements
       on them. Inside one element: a conductor earthed at both ends is Kron-reduced in
       `Z` and struck in `C`, a conductor on private nodes is struck in `Z` and
       Kron-reduced in `C`, a conductor earthed at one end only is struck in `Z` and
       struck in `C`, a star node is Kron-reduced in the nodal matrix. A star node
       made of several elements is eliminated jointly.
    6. Chains. Internal nodes that join the non-phase conductors of several Lines
       (cable screens, cross-bonding) are eliminated jointly, each Line receiving the
       share of its own section.
    7. Parallel conductors between the same two port nodes merge before the projection:
       `Zb = (A^T Z^-1 A)^-1`, `Cb = A^T C A`.
    8. Projection. A coupled set whose conductors all join port nodes of equal phasor
       takes `Z1 = 3 G^-1 (W^H Zb W) G^-1`, `G = W^H W`. Every other set, and every
       `cmatrix` of a Line, takes `Y1 = (1/3) W^H Y W` on its nodal matrix. Instances:
       three phases `(1/3) v^H Z v`, two phases at 120 degrees `(3/4)(Z11 + Z22 - Z12)`,
       two poles `(3/4)(Z11 + Z22 - 2 Z12)`, one phase `3 Z` and `C/3`, a branch between
       two phases `y`.
    9. Sources. A Vsource becomes `E1 = (1/N) w^H E` behind its impedance in the
       impedance form. An Isource becomes `(1/3) W^H I`.
    10. Element written. One element of the same class with `phases=M` and M x M
        matrices, M the number of circuits, so the inter-circuit mutual impedance and
        capacitance stay. M = 1 is the scalar form.
    11. Buses. Port j of a bus becomes node j. Internal nodes and earth become node 0.
        An element left with every conductor on node 0 is disabled.
    12. Refusals, one criterion: the balanced pattern cannot be written, or the result
        is not one reciprocal passive element of the class. Two different phasors
        brought to one node by Lines or windings, a multi-phase terminal not on one port
        in positive order, two elements that partition the phase nodes of a bus
        differently, a Line conductor from a port node to a node of another phasor, a
        coupled set that mixes conductors between ports with conductors to earth, a
        matrix of the wrong order, a non-passive `gmatrix`, a one- or two-phase Fault.
    13. A refusal stops the command with the element named and changes nothing. A
        multi-phase element is never left on a reduced circuit.
    14. Stated limits, pinned and not refused. Only the symmetric part of an
        inter-circuit term is written. It is exact when the circuits have the same or
        the mirrored phase order at a displacement of 0 or 180 degrees. Otherwise the
        reduced element loses reactive power `6 A12 Im(conj(i1) i2)`, with `A12` half
        the difference of the two directions, and part of the voltage effect: for a
        displacement other than 0 or 180 degrees, for circuits of different phase count
        and for circuits on one structure whose phase positions are rolled against each
        other. The active power is kept. A one- or two-phase element on a three-phase
        network is off by `|Z2 + Z0| / |3z|`. A two-phase Line that serves a load
        between its phases is off by
        `|Z11 + Z22 - 2 Z12 - (3/4)(Z11 + Z22 - Z12)| / |z_load|`. Unbalanced loads
        have no positive-sequence form. A three-phase fault behind an unsymmetric
        series element is off by the gap between the impedance form and the admittance
        form, 1 to 4 per cent on an untransposed line or a flat cable. A fault at the
        end of a one- or two-phase lateral is off by more than 10 per cent.
    15. Literature check (2026-10-04, Dommel, Tleis, Kersting, Clarke, Anderson, Hase,
        Ametani, Дмитриев, Grainger, IEEE Std 551, a skeptic re-opened the quotations).
        Confirmed by the books: items 4, 5, 7, 10, the three-phase and two-pole
        instances of item 8, the chain rule of item 6 (Tleis Example 3.5: the book gives
        0.017 + j0.138 ohm/km, joint elimination 0.0167 + j0.1379, zero volts per
        section 0.0429 + j0.1284), the Fault refusal. Corrected from the books: the
        limits of item 14 and the one-end rule of item 5. Without a published source,
        this project's own derivation: items 2, 3, 9 (Isource), 11, 12, 13, the
        two-phase instance `(3/4)(Z11 + Z22 - Z12)`, the base factor `3/N`, `C/3`,
        `Cb = A^T C A`, the static form rule, the ratings of 16.7.
        User's word (2026-10-04): no objection to the delegated choices of items 6, 12,
        13 and of 16.4 and 16.6, the non-reciprocal term follows the books, and the
        planner of the MakePosSequence plan takes the final decision on each. The rating
        of merged cores (16.7, the smallest of rating over current share) is confirmed
        by the user.
        Open to the user: a two-phase Line whose downstream elements all lie between
        its phases could take the loop form `Z11 + Z22 - 2 Z12` (IEEE Std 551 section
        3.7, exact there) in place of the stated limit. An inter-circuit term with
        rolled phase positions could be refused above some share in place of the
        symmetric part (Kersting Example 4.2: the antisymmetric part is 1.73 times the
        symmetric part, reactive power off by up to 8.1e-3).

## Queue

| # | step | rulings | state |
|---|---|---|---|
| 0 | wave 9: props evidence lock, schema property ranks, strict ledger load | - | landed |
| 1 | `NVariables` default 20 | 2 | landed |
| 2 | rating "not set" | 3 | settle 3 finished 2026-10-10 at 2389d1f4 (non-finite rating refused), gate green, both fresh audits done (code: no major, one minor folded into ruling 14.9 for step 10, tests: no major, one minor T-1 (a seasonal file count assertion that cannot fail) and five notes, recorded in the record at the landing), the landing after step 3 with a rebase onto update and the full gate, Q12 decided as ruling 14.9 |
| 3 | MakePosSequence matrices | 1, 16.21, 16.22 | landed 2026-10-10 as b6517a73 on update (squash of lane-m d9eaeb29, gate reused, record bullet decided by 16.21), fresh audit of settle 4: no major, A-1 wording and three notes handled by 16.22 |
| 4 | LineGeometry rules | 4, 16.13, 16.19, 16.20 | settle 2 parts 1 to 3 at ad1fdf3a, part 4 (record counts) and the `like=` default unit of 16.13 being committed 2026-10-10, then the full gate, one fresh audit, the landing after steps 3 and 2. Open questions of the step decided: Q8 and Q9 by 16.13 and 16.14, Q4 by 16.12, the earth constant and the tape lap by 16.19 and 16.20 |
| 5 | elements without dynamic state: disabled, joining, NaN never converges, VCCS without curves refuses | 5, 6 | queued |
| 6 | monitors and meters: mode 3 accepts WindGen, VSConverter and GICLine, no added node, failed binding, a mode-3 monitor on a WindGen and a `like=` Storage make the dynamics Y singular | 8, 9 | queued |
| 7 | Storage dynamics model | 7 | queued |
| 8 | `Open` on a machine in dynamics | 10 | queued, after step 5 |
| 9 | `DynamicEq` re-link: calculated values keep their rows by name, the equation memory survives, Save writes `DynamicEq` before `DynOut`, a machine refuses a `DynOut` that names one variable twice, a host with `NVariables` omitted lists only the named variables, upstream reports: a new one for the `DynOut` that names one variable twice (measured on r4133, with the documentation quote), the documentation quote added to 85 and 86, the r4133 measurements still missing in 84, 87, 89 | 12, 13 | queued |
| 10 | rating details: three states, typed `0` and negatives, Save writes `-1` or the typed number, no overwrite of a typed value, partly rated Line is an error, derived zero, derived negatives, `?` and Save after `Set %Normal`, refusal of inputs that make a stamp or rating non-finite | 14 | queued, after step 2 lands |
| 11 | transformer kVA ratings not set | 14 | queued, after step 10 |
| 12 | protective-device ratings not set | 14 | queued, after step 10 |
| 13 | MakePosSequence by physics: the node map, elimination of internal nodes and chains, merge of parallel cores, the projection in its two forms, coupled `phases=M` elements, sources, refusals that stop the command, matrix order, repeated command, ratings | 16.1-16.9, 17 | queued, after step 3 lands and its audits are compared with the verdicts |
| 14 | LineGeometry additions: empty readback, equivalent distances with their own unit, `nconds=` order, conductor without position, `wires=` count rule, help text, capacitance pin, `mm` (the `like=` unit of 16.13 and the `nconds=` reset of 16.14 are done in step 4) | 16.11-16.18 | queued, after step 4 lands and its audits are compared with the verdicts |
| 15 | delta Capacitor and Reactor: conductor count in either order of `conn=` and `phases=`, the delta Reactor matrix stamp | 16.4, 16.10 | queued, corpus scan with continuation lines first |
| 16 | MakePosSequence of PVSystem, Transformer, Storage and the other power conversion classes by the same rule | 16.1 | queued, after step 13 |
| 17 | exact unit words in every class with a unit property, the `nconds=` rule in LineSpacing | 16.12, 16.14 | queued, after step 14 |
| 18 | overhead wire height above its radius, the finite full-Carson limit at zero height | - | queued, after step 14 |
| 19 | TSData tape-lap correction by physics: derive the resistance factor of a lapped tape from its geometry, check against Kersting 3rd ed. section 4.2 (tape-shield example) and manufacturer data, measure both oracles, pin, upstream report if the engine's factor is wrong | 16.20 | queued, after step 14 |
| 20 | Capacitor delta and cmatrix filter stamp by the exact form Y_d (I + zl Y_d)^-1 in place of the 1.000001 diagonal scaling, pinned against the wye equivalent, both oracles diverge by 6.7e-7 | 16.22 | queued, after step 14, disjoint from the MakePosSequence plan steps unless one touches the Capacitor stamp |

## Open items that need no ruling now

- Upstream reports for OpenDSS: disabled elements integrated, a joining element crashes
  the solve, the Storage idling branch (units and sign), VCCS without curves, floating
  nodes from meters, the LineGeometry cursor and crash, `Open` on a generator. Each is
  written inside its step.
- `Save circuit` writes the controls of one transformer in another order than r4133
  after one of them is disabled. To be put to the user with measurements.
