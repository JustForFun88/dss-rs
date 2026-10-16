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

1. **MakePosSequence matrices** (2026-10-04). Landed as queue step 3 (b6517a73). The
   rule and the procedure that replaced it are stated in `MAKEPOSSEQ_PLAN.md`:
   section 1.5 and decision D21, with its row in section 1.14.

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
    10. (coordinator, 2026-10-10, under the delegation "вопросы ко мне решай сам", from the
        plan of step 10) The questions the plan put to the user are decided. The narrow
        paths taken in the plan stand for Q-A (refused at the recalc, no YPrim, the Y build
        gives one error, the element and its typed properties stay), Q-C (a later
        `linecode=`, `geometry=` or `spacing=` definition applies and clears the Line's
        typed rating), Q-D, Q-E (the partly rated Line logs the error and is built with that
        rating not set), Q-G (a typed non-finite kVA or device rating is refused at its
        token, as step 2 refuses a typed current rating), Q-H (a `Set %Normal` that
        overflows refuses the whole command), Q-I, Q-J, Q-L (a zero size `kvar=0` is an open
        device with a not-set derived rating, a zero base `kv=0` is refused), Q-M (a
        dynamics or harmonics quantity is checked when its mode first computes it) and
        Q-N (14.4 reads the phase conductors). Three alternatives are taken instead:
        Q-B, a derived negative rating other than the `-1` sentinel loads against its
        magnitude, as the user decided for Transformer and AutoTrans on 2026-10-03, since
        the current of a 300 kvar inductive bank flows whatever the sign of `kvar`. Q-F,
        the rating a LineGeometry derives is the weakest phase conductor's, as the spacing
        form already does, since every phase conductor carries the line current, and the
        gated cases that move (three asymmetric decks reading 230 where the oracles read
        530) are excluded field by field and pinned, with an upstream report. Q-K, a
        typed non-finite rated voltage or kVA base is refused at its token as well, since
        the documentation gives those as positive numbers, beside the recalc refusal of a
        finite input with a non-finite result.
    11. (coordinator, 2026-10-10, same delegation, from the plan check of step 10, F1 and
        S9) Each rating of a spacing or geometry Line is derived on its own at the end of
        the Line's edit from the typed record: a rating typed after the conductors stops
        only its own derivation, the untyped partner takes the weakest-phase reading and
        the 14.4 check, and `Seasons=` or `Ratings=` typed after the conductors no longer
        stop the scalar derivations. This is 14.3 and 14.4 applied, no typed value is
        overwritten and no derived rating escapes the partly rated check. The same deck
        reads the same whether written on one line or split with `~`.
    12. (coordinator, 2026-10-10, same delegation) The side finds of the plan that lie in
        classes without a rating go to their own queue step (step 22): `Vsource basekv=0`
        solving with finite voltages and no message, `AutoTrans kvs=[0 6.9]` giving a
        negative series kV with a finite solve, `GICTransformer r1=0 r2=0`. Each is a zero
        base that leaves the data without meaning and is refused at the recalc by the rule
        of 14.9. The inline RPN inside a list property (S4) and a conductor whose emergency
        rating lies below its normal rating (S8) stay open items without a ruling.
    13. (coordinator, 2026-10-10, same delegation, from the second plan check of step 10,
        finding A) A typed transformer current rating is kept under 14.3 also where it
        hides an overload the oracles report from the derived rating: on the gated
        `StoCtrl_SeasonTarget/Run_example.dss` the typed 400 A on `xfm1` writes no
        `DI_Overloads` row where both channels write thirteen from 76.3 A. The demand
        interval comparer compares the row count of every file unconditionally, so the
        harness gains one element-scoped row exclusion for `DI_Overloads` files only (the
        ledger names the case, the channel, the file and the element, the matching rows
        leave both sides before the count and the cells, an entry nothing matched fails as
        stale). Every other file keeps the unconditional row count. The exclusion is pinned
        by a harness unit test, described in `TESTING.md`, and the divergence by the pin of
        the kept typed rating with a `DI_Overloads` leg (the typed transformer writes no
        row, its untyped twin writes one). The upstream report states that the typed rating
        is silently replaced by the derived one. Minor findings B, C and D and the notes of
        that check are fixed in the plan by its third revision, none needs a ruling.
    14. (coordinator, 2026-10-10, same delegation, from the third plan check of step 10,
        finding M1) `normamps` and `emergamps` of a Transformer and an AutoTrans are read
        only. The documentation says "(Read only) Normal rated current. Use normhkVA to
        specify transformer normal rating." and the same for the emergency current with
        `emerghkVA`, the r4133 property help says the same, and IEC 60076-1 3.4.7 derives a
        transformer's rated current from its rated power and voltage, so the element has no
        current rating of its own: on `xfm1` of `StoCtrl_SeasonTarget/Run_example.dss` the
        typed 400 A at 4.16 kV would be 2882 kVA on a 500 kVA transformer and hides a
        measured loading of 113.6 % of the rated current. A typed value is refused at its
        token (`like=` included) with a message naming the element, the property and the
        property that sets the rating, the derived rating stays and nothing typed is
        recorded. 14.3 covers the ratings the documentation lets the user write and is not
        engaged here. 14.13 is superseded: no row exclusion for `DI_Overloads`, no ledger
        entry for the hidden overload, the readings equal both oracles'. Save and the JSON
        set-order sweep write neither property for the two classes, since the documentation
        gives them no written form, so a saved deck reloads without the refusal, and every
        golden byte that moves is listed by the plan. The gated decks that type the property
        declare the message in `expect_warnings` and their population-lock cells move. The
        upstream report states that a read-only property accepts a value in silence. The
        notes of that check: a zero winding kVA is a zero size by physics (`Z = z U^2/S`),
        refused as the narrow path because the documentation gives a rated power as a
        positive number, and the plan's wording and report N1 say so (N1). A derived `-1` A
        follows 14.10 (N3). The seasonal `Ratings` of an untyped geometry stay as the third
        revision has them, the per-season weakest reading is an open item without a ruling
        (N4). The wording of m2 (the neutral has no reader, its thermal duty is unchecked)
        and N2 (the weakest-phase rating never misses an overload, it is not the exact check)
        goes into the plan, the pins and the reports. The population lock formats no
        `expect_warnings` and gets no `warn=` field: the declared message is checked by
        the gate alone (fourth check, N1).
15. **The open questions of steps 3 and 4** are decided by a research workflow: physics
    with standards and textbooks for MakePosSequence, the documentation and the needs
    of a future language server for LineGeometry. The verdicts come back to the user.
16. **Research verdicts taken** (2026-10-04, the user delegated the choice: as close to
    physics as possible). Rating, closing ruling 14: a typed negative other than `-1` is
    stored, read as not set and raises no error.
    MakePosSequence, items 1 to 10 (2026-10-04): moved on 2026-10-17 into
    `MAKEPOSSEQ_PLAN.md`, which states each in full and names the book or the
    derivation behind it. The self term and the rated power (item 1) in sections 1.13
    and 1.14 with decision D17, the one- and two-phase forms (2) in 1.8 and 1.14, the
    replaced item 3 in 1.7 and 1.14, the delta matrices, the one-matrix Reactor and
    the delta conductor count (4, 5, 10) in 1.14, decision D4 and step MPS-07, the
    two-pole Vsource and the Fault refusal (6) in 1.9, 1.12 and decision D2, the
    ratings (7) in 1.10, the matrix order (8) in 1.12 and 1.14, the repeat rule (9) in
    1.14 and decision D8. Items 11 to 27 keep their numbers.
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
        after the reduction writes it as rp. The others are answered, with their
        candidates and numbers, by decision D19 of `MAKEPOSSEQ_PLAN.md`. The choices
        marked taken in the row stand until that plan lands them.
    22. (coordinator, 2026-10-10, same delegation, from the fresh audit of settle 4) Three
        items it left are not put to the user. The delta and cmatrix filter stamp of the
        Capacitor scales the leg diagonal by 1.000001 before inverting, which both oracles
        do too, so the bank draws 6.7e-7 more than physics and gains a zero-sequence shunt
        of 1.29e-9 S. The exact form Y_d (I + zl Y_d)^-1 replaces it in its own queue step
        (step 20) with a pin against the wye equivalent, since no corpus deck reaches the
        arm. The reading of a phase-to-phase reactor after the reduction and the "reads
        the loss of" wording of the row's Exclusion paragraph are decision D19 of
        `MAKEPOSSEQ_PLAN.md`.
    23. (coordinator, 2026-10-10, same delegation, from the fresh audit of LineGeometry
        settle 2) A Line's own `spacing=` reads the LineSpacing's unit the way the
        geometry path does: a unit word the documentation gives for LineSpacing is
        honoured and converted, and a spacing the geometry path refuses (no unit, a word
        outside the documentation) is refused by the Line too, with a message naming the
        Line, the spacing and the unit. Reading feet or no unit as metres without a
        message is an upstream bug (r4133 shares it) and is fixed in step 4 itself, since
        the module is this step's and rule 8 is this step's rule. Every corpus deck the
        fix moves is excluded field by field and pinned, and report 83 gains the case.
    24. (coordinator, 2026-10-10, same delegation) `nphases` of a LineGeometry defaults to
        the documented 3, so `nconds=4` with three cable entries and no `nphases=` builds
        the geometry. The refusal "the geometry has no phases" stays for an explicit
        `nphases=0` and is pinned and stated in the row. A `cond=` value that is not a
        whole number (`cond=1.5`) or lies outside 1 to `nconds` (`cond=1e10`) is refused
        with a message naming the written value, since the documentation gives `cond` as
        an integer index. A bare `cond=N` changes no unit: the conductor takes the
        geometry's default unit at its first selection, unless `units=` is written for it
        (corrected 2026-10-10 after settle 3 measured the tree and both oracles doing
        this, X21 0.5831588 against 0.8185604 for a default read when the coordinates
        are written). The pin goes into queue step 17 with the unit words, and the Line
        refusal naming `units=none` for a written `feet` is fixed there too, since a
        LineSpacing stores only the unit code today.
    25. (coordinator, 2026-10-10, same delegation) A `like=` copy onto a geometry that
        already holds conductors takes the source's default unit as well (r4133 gives
        m m m there and so does the port) and gets its own pin. The decided sentence on
        unit words says only what the tree does: `feet`, `Meters`, `inches` and
        `kilometer` are refused and pinned, `metre`, `meter` and `miles` are still read
        and go to the exact-word step (step 17). The `TODO(compat)` comments at the
        Kxg sites no longer call 658.8530451057239 the corrected value (16.19).
    26. (coordinator, 2026-10-10, same delegation, from settle 3) The documentation gives
        `nconds` of a LineGeometry the default 3 where the tree and r4133 hold 0. The
        documentation is the language spec, so the default becomes 3 in its own queue step
        (step 21) with the Save, `like=` and readback consequences measured on both
        oracles, not in step 4. The port clamps only the phase count the calculation
        uses to the conductor count while r4133 clamps the stored `nphases`. No oracle
        surface compares the stored value, so the port's choice stands and is recorded.
    27. (coordinator, 2026-10-10, same delegation, from the fresh audit of settle 3) The
        audit's minors do not stop the landing of step 4. A wrong dss_capi figure in the
        row (case 17 reads 0.8054776 on the deck named, not 0.7944461) is corrected at
        the landing, since a false measurement is never committed knowingly. The pin
        `a_broken_geometry_beside_a_refused_stamp_names_both` asserting the system Y
        where the voltage-base pass builds the series Y, the doc comment that moved from
        `set_object_ref` to `refuse_object_ref`, the out-of-range `cond=` message naming
        the text as written with one code for both paths, and the dss_capi readings of
        report 98 are recorded in the record as left for queue step 17. An explicit
        `nphases` above `nconds` keeps the clamp of 16.26 until step 21 decides the
        refusal together with the `nconds` default.
    28. (coordinator, 2026-10-17, same delegation, from the fresh audit of step 14
        settle 1) A LineSpacing whose `x=` or `h=` list is shorter than `nconds` places
        the missing conductor at 0 in silence, on both oracles too, and a geometry or a
        Line on it solves with mutual impedances for a position nobody wrote. The fix
        belongs to queue step 17 with the LineSpacing `nconds=` rule: the spacing keeps
        which coordinates were given, a geometry copies the gaps, rule 7 names the
        conductor, `?` reads empty and Save writes no `X=` or `h=`, pinned in both lanes,
        with the case added to report 83. Step 14 states the gap in the row and lands.
    29. (coordinator, 2026-10-17, same delegation, from the same audit) `nconds` gets no
        numeric bound: the documentation gives none and a bound would be a language
        extension. The engine refuses a count it cannot allocate at `nconds=`, naming the
        text as written, and a count that fits once but not for a second engine (a
        cable, a report copy, a Line) is a stated limit recorded in the row until the
        engine is built at calculation only.

17. **One procedure for every MakePosSequence matrix question** (user, 2026-10-04, from
    two research runs: literature, derivation, DSS representation, a skeptic each. Refined
    through 2026-10-17 by three opus rounds of plan review and two fable rounds of
    literature review with two sources per statement. Items 16 to 18 were the
    coordinator's rulings of 2026-10-10 and 2026-10-16 under the delegation "вопросы ко
    мне решай сам".) The complete text is stated in `MAKEPOSSEQ_PLAN.md` and is binding
    there: the procedure in section 1 (the principle, the node map, the elimination, the
    chains, the merge, the two projection forms, the sources, the element written, the
    buses, the refusals and the unchanged circuit, the literature check), the decisions in
    section 2, the stated limits in section 6, the four questions decided in section 8.
    Five sentences of 2026-10-04 were replaced by the books' values and the plan states
    the replacements: the mirrored phase order is not an exact case of the inter-circuit
    term (section 6, L1), a unit of an open-delta pair divides its rated power by 2√3
    (section 1.13), a bank of one-phase units between one pair of ports is projected
    jointly (section 1.13), the two-phase self term is the balanced-current excitation
    (section 1.14), and `Cb = Aᵀ C A` is Dommel's, not the project's own derivation
    (section 1.7). Two more are refined by the plan's own derivation, which it states
    and pins: the rated power over 3 holds where the legs sit at one per-unit voltage,
    the other legs follow the leg rule of decision D17 (stated limit L11), and the lost
    power of a non-reciprocal inter-circuit term is written with 6 Re(A12) and
    6 Im(A12) (stated limit L1). The Clarke page 420 of the mirrored order is Vol. I,
    and a source above three phases is refused under case 2 of section 1.12.

## Queue

| # | step | rulings | state |
|---|---|---|---|
| 0 | wave 9: props evidence lock, schema property ranks, strict ledger load | - | landed |
| 1 | `NVariables` default 20 | 2 | landed |
| 2 | rating "not set" | 3, 14.9 | landed 2026-10-10 as 66839773 on update (squash of lane-c 2389d1f4 rebased onto cc76b42e, full gate green 3433 per lane), Q12 decided by 14.9, the test audit's minor T-1 and five notes recorded in the record as left for step 10 |
| 3 | MakePosSequence matrices | 1, 16.21, 16.22 | landed 2026-10-10 as b6517a73 on update (squash of lane-m d9eaeb29, gate reused, record bullet decided by 16.21), fresh audit of settle 4: no major, A-1 wording and three notes handled by 16.22 |
| 4 | LineGeometry rules | 4, 16.13, 16.19, 16.20, 16.23 to 16.27 | landed 2026-10-10 as 4212d57f on update (squash of lane-b c9bd413d, gate reused after the full gate of settle 3, 3501 per lane), the audit minors left for steps 17 and 21 recorded in the record |
| 5 | elements without dynamic state: disabled, joining, NaN never converges, VCCS without curves refuses | 5, 6 | running since 2026-10-16 on lane-m from df413f4a (state folder dynamics_no_state), the doubtful state-or-setting boundaries of ruling 6 decided by the plan under the delegation of 2026-10-10 |
| 6 | monitors and meters: mode 3 accepts WindGen, VSConverter and GICLine, no added node, failed binding, a mode-3 monitor on a WindGen and a `like=` Storage make the dynamics Y singular | 8, 9 | queued |
| 7 | Storage dynamics model | 7 | queued |
| 8 | `Open` on a machine in dynamics | 10 | queued, after step 5 |
| 9 | `DynamicEq` re-link: calculated values keep their rows by name, the equation memory survives, Save writes `DynamicEq` before `DynOut`, a machine refuses a `DynOut` that names one variable twice, a host with `NVariables` omitted lists only the named variables, upstream reports: a new one for the `DynOut` that names one variable twice (measured on r4133, with the documentation quote), the documentation quote added to 85 and 86, the r4133 measurements still missing in 84, 87, 89 | 12, 13 | running since 2026-10-16 on lane-a from df413f4a (state folder dynamiceq_relink), disjoint from step 5 by file |
| 10 | rating details: three states, typed `0` and negatives, Save writes `-1` or the typed number, no overwrite of a typed value, partly rated Line is an error, derived zero, derived negatives, `?` and Save after `Set %Normal`, refusal of inputs that make a stamp or rating non-finite | 14 | running since 2026-10-10 on lane-c from 73a364f9 (state folder rating_details), three plan checks each left one major (F1 under 14.10 to 14.12, A under 14.13, M1 under 14.14 which supersedes 14.13), the plan is revised a fourth time and checked again, then the executors, stopped 2026-10-10 on the user's order inside the first executor part, resumed 2026-10-16 from a clean tree |
| 11 | transformer kVA ratings not set | 14 | queued, after step 10 |
| 12 | protective-device ratings not set | 14 | queued, after step 10 |
| 13 | MakePosSequence by physics: the node map, elimination of internal nodes and chains, merge of parallel cores, the projection in its two forms, coupled `phases=M` elements, sources, refusals that stop the command, matrix order, repeated command, ratings | `MAKEPOSSEQ_PLAN.md` | replaced by `MAKEPOSSEQ_PLAN.md` steps MPS-01 to MPS-06, MPS-09 and MPS-10 (plan accepted 2026-10-10, its literature review closed 2026-10-17 by the second fresh check with no major, the plan holds every MakePosSequence rule itself since 2026-10-17, MPS-01 next) |
| 14 | LineGeometry additions: empty readback, equivalent distances with their own unit, `nconds=` order, conductor without position, `wires=` count rule, help text, capacitance pin, `mm` (the `like=` unit of 16.13 and the `nconds=` reset of 16.14 are done in step 4) | 16.11-16.18, 16.28, 16.29 | landed 2026-10-17 as 6357214f on update (squash of lane-b 6f95633a, seven commits on 73a364f9, the lane gate of 6f95633a reused since update moved by register and plan text only), fresh audits of settle 2 found no major: the two oracle statements they faulted were corrected at the landing and their other minors and notes are recorded for steps 17 and 21, the upstream reports 83, 98 and 100 verified on r4133 with report 83 gaining case 24 |
| 15 | delta Capacitor and Reactor: conductor count in either order of `conn=` and `phases=`, the delta Reactor matrix stamp | `MAKEPOSSEQ_PLAN.md` | replaced by `MAKEPOSSEQ_PLAN.md` step MPS-07 (wave 2) |
| 16 | MakePosSequence of PVSystem, Transformer, Storage and the other power conversion classes by the same rule | `MAKEPOSSEQ_PLAN.md` | replaced by `MAKEPOSSEQ_PLAN.md` step MPS-08 (wave 3) |
| 17 | exact unit words in every class with a unit property, the `nconds=` rule in LineSpacing, a short `x=` or `h=` list of a LineSpacing kept as not given | 16.12, 16.14, 16.28 | queued, after step 14 |
| 18 | overhead wire height above its radius, the finite full-Carson limit at zero height | - | queued, after step 14 |
| 19 | TSData tape-lap correction by physics: derive the resistance factor of a lapped tape from its geometry, check against Kersting 3rd ed. section 4.2 (tape-shield example) and manufacturer data, measure both oracles, pin, upstream report if the engine's factor is wrong | 16.20 | queued, after step 14 |
| 20 | Capacitor delta and cmatrix filter stamp by the exact form Y_d (I + zl Y_d)^-1 in place of the 1.000001 diagonal scaling, pinned against the wye equivalent, both oracles diverge by 6.7e-7 | 16.22 | queued, after step 14, disjoint from the MakePosSequence plan steps unless one touches the Capacitor stamp |
| 21 | LineGeometry `nconds` defaults to the documented 3 (tree and r4133 hold 0), with the Save, `like=` and readback consequences measured on both oracles and pinned, and the before-`nconds=` refusal restated as an explicit flag, since it keys today on a geometry without conductors, a fractional, non-finite or JSON-rendered `nconds=` and an `nphases=` past the integer range refused naming the text as written | 16.26, 16.29 | queued, after step 17 |
| 22 | zero bases in classes without a rating refused at the recalc by the rule of 14.9: `Vsource basekv=0`, `AutoTrans kvs=[0 6.9]`, `GICTransformer r1=0 r2=0`, and the other routes a census finds | 14.9, 14.12 | queued, after step 10 |
| 23 | a Line edited after its first build (`length=`, `rho=`, `earthmodel=`) recomputes its impedance at the next solve, where the tree and r4133 keep the old one until the frequency changes, measured on both oracles, pinned, with an upstream report | - | queued, after step 17 (found by the step 14 settlements) |

## Open items that need no ruling now

- Upstream reports for OpenDSS: disabled elements integrated, a joining element crashes
  the solve, the Storage idling branch (units and sign), VCCS without curves, floating
  nodes from meters, the LineGeometry cursor and crash, `Open` on a generator. Each is
  written inside its step.
- `Save circuit` writes the controls of one transformer in another order than r4133
  after one of them is disabled. To be put to the user with measurements.
