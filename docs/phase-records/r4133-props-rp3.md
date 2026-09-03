# R4133_PROPS - WP-RP3 records

> Moved verbatim from `STATUS.md` on 2026-09-03 (STATUS.md archiving round 2);
> order preserved, nothing rewritten. It holds STATUS §1's `### R4133_PROPS WP-RP3`
> condensed records (RP3.1-RP3.5) followed by the full sub-step records of
> RP3.6-RP3.13 with their audit settlements — and, appended on 2026-09-04 after
> them, §RP3.10's, the last WP-RP3 sub-step to run. The **three** pin-citation
> guards in `crates/dss-core/tests/props_r4133_replay.rs` read this file (RP3.11,
> RP3.13 and — since 2026-09-04 — RP3.10); STATUS.md §7 forwards `§WP-RP3` and
> `§RP3.1`...`§RP3.13` here.

### R4133_PROPS WP-RP3 — condensed records

> Plan: `R4133_PROPS_PLAN.md` §WP-RP3. Same branch (`r4133-props`), same
> per-sub-step ritual. Four bin-7 root-cause pairs (RP3.1–RP3.4) plus the five
> sub-steps the WP-RP2 triage opened (RP3.5–RP3.7 from RP2.2, RP3.8 from RP2.3's
> kill ruling, RP3.9 from the RP2.4 audit settlement) plus §RP3.12, which RP3.9's
> own P0 open item opened. RP4.1 waits on the first nine of
> them; **RP3.5 landed 2026-08-28 (audit settled 2026-08-29), RP3.6 both parts
> 2026-08-29 (audit settled the same day) and RP3.7 2026-09-02 (audit settled the
> same day: 11 findings, 9 fixed, 2 fixed with a sub-claim refuted, none
> dropped)**, and **RP3.8 landed 2026-09-02** as well (audit settled the same
> day: 8 findings after dedup, one major — `Save` was a fifth, un-refreshed
> `get_value` reader — 7 fixed, 1 recorded, none dropped), and **RP3.9 landed
> 2026-09-02** as well (27 pairs, all `PRECISION_ROUNDTRIP`, no product-crate
> line; audit settled 2026-09-03 — 15 raw findings, 11 distinct: 9 fixed,
> 2 recorded, none touching a verdict), so every WP-RP3 sub-step **RP4.1 waits
> on** has landed and the unmask is no longer blocked here. **§RP3.12 landed
> 2026-09-03** as well (audit settled the same day: 12 findings, 11 distinct —
> 9 fixed, 2 recorded, 0 refuted) — RP3.9's P0 open item, settled
> `UPSTREAM_BUG`/never-reproduced with zero product-crate lines, and no
> precondition to the flip either, its four decks being capi-only. **RP4.1 itself
> landed 2026-09-03** (audit settled the same day: nine dispositions, eight fixed,
> one recorded), taking the eight `property` entries RP3.1/RP3.2/RP3.4 staged into
> `ledger.json` and retiring their `RP3_ROUTING` rows — so nothing below is
> "staged" any more; the §RP4.1 record in §1 carries the landing. Of the WP's
> two sub-steps that run after RP4.1 and block §RP5.2 instead (plan §0),
> **§RP3.11 landed 2026-09-03** (`KEEP_LIVE_PINNED` on both `Save` and `Dump`;
> the kill criterion fired; audit settled the same day — 14 raw findings, 12
> distinct: 10 fixed, 2 recorded, 0 refuted, three more `SaveWrite` guards
> ported), and only §RP3.10 is left — **§RP3.13**, which RP3.11's P0 findings
> opened, was accepted and landed 2026-09-03 (two NCIM `PORT_BUG`s fixed in both
> lanes — four defects in all, with a fifth fixed by its audit settlement the
> same day: 12 findings, 6 fixed, 6 recorded, 0 refuted — zero ledger entries,
> zero golden bytes). The RP3.6, RP3.7, RP3.8,
> RP3.9, RP3.11, RP3.12 and RP3.13 records live in §1 above, beside RP3.5's
> narrative one.

- **RP3.1** (2026-08-24) — `swtcontrol.delay`: **a wired property that r4133
  silently ignores.** **Zero product-crate bytes** (the port already behaves
  correctly), **zero `ledger.json` bytes**, zero golden / manifest /
  `population.lock` / frozen-extract bytes, and no r4133 mask moved — the whole
  sub-step is two test files plus docs.
  - **Root cause, verified against the vendored r4133 source, not the plan's
    transcription.** `SwtControl` declares nine properties, the fifth being
    `Delay`. `Edit` stores every token in `PropertyValue[]` first
    (`Version8/Source/Controls/SwtControl.pas:192-193`) and then dispatches on
    the property number — and the `CASE` has arms for 1, 2, 3, 4, 6, 7, 8 and 9
    but **none for 5** (`:195-218`; the plan said `:194-217`, off by one, the
    only drift found). `delay=` therefore falls through to `ClassEdit` as an
    inherited parameter, `TimeDelay` keeps the 120.0 `Create` gave it (`:310`),
    and the getter — which is **LIVE**, `Format('%-.7g',[TimeDelay])` at `:588` —
    answers `120` to a deck that asked for 0.25. dss_capi 0.14.5 wires the
    property through its typed table (`src/Controls/SwtControl.pas:185`) and this
    port follows it (`elements/control/swt_control/accessors.rs:114`, ordinal at
    `mod.rs:51`, `PropDef::double("Delay")` at `mod.rs:78`), so **nothing in the
    engine was owed a change**.
  - **Render-only, and provably so.** The report had to state it is not a
    queue-delay bug, and the evidence is stronger than the plan assumed:
    `Sample`'s two `ControlQueue.Push` calls are commented out wholesale
    (`:484-507`, "Removing because action … and lock are instantaenous"),
    `DoPendingAction` likewise (`:396-408`), `set_States` writes
    `ControlledElement.Closed[]` immediately (`:532-549`) — **and** `LockCommand`
    is commented out of the class declaration too (`:39`), while
    `ActionCommand`/`PresentState`/`Armed` exist in neither `TSwtControlObj` nor
    `TControlElem`. The dead block would not compile if uncommented, so nothing
    upstream can consume `TimeDelay`. Two further facts worth the record: r4133's
    own `InitPropertyValues` (`:654-655`) writes `'120.0'` into slot 5 and then
    immediately `''`, so `Dump` prints the deck's token while `?` prints 120 —
    the engine contradicts itself; and the **sibling copy in the same trunk**
    (`Version8/Source/CMD_Lazz/Controls/SwtControl.pas:179`) still has
    `5: TimeDelay := Parser.DblValue;`, so the arm was lost in a rework rather
    than never written.
  - **Census, confirmed twice** (frozen extracts + a bounded live re-run,
    `DSS_PROPS_CENSUS=claims DSS_GATE_ONLY=swtcontrol`, artifacts deleted):
    42 cells / **24 in scope**, and the in-scope half decomposes exactly as the
    plan claims — **12 + 12** over `controls:swtcontrol/swtcontrol_time.dss` and
    `controls:swtcontrol/midi_swtcontrol.dss` (both `engines: r4133`, one cell
    per step 0..11, ours `0.25` vs r4133 `120`, `max_rel` 0.9979166666666667).
    The other **18** are all `capi_v0145`, hence owed nothing: **12** on
    `controls:swtcontrol/swtcontrol_lock.dss` (the same `~ delay=0.25`, 12 steps
    — it is the third deck of the `'0.25'` spelling, whose frozen row is
    therefore 36 cells, not 24) and **6** on three copies of the vendored
    `IEEE_519.DSS` (`Delay=0.0`, two SwtControls each, 1 step — the `'0'`
    spelling). `civanlar.dss` — 16 SwtControls, `engines: r4133`, in scope —
    types no `delay=` and produces **no** cell, i.e. our unset render already
    equals r4133's 120, measured by its absence. Since the audit settlement the
    whole decomposition is **derived, not transcribed**:
    `props_r4133_replay::the_rp31_census_decomposition_is_read_off_the_corpus`
    sweeps every corpus `.dss` for SwtControl declarations, multiplies each
    case's controls by its `population.lock.json` `steps=`, splits on the lock's
    `engines=`, reconciles the products against `bins.tsv`'s 42/24 and both
    frozen example rows (36 / 6), and requires the in-scope cases to be exactly
    the cases the drafted entries cite — "exactly two entries" is now a measured
    conclusion.
  - **The exclusion shape is a ledger entry, never an echo row** (the plan is
    explicit and the reason is the mechanism: `:588` is live, so calling this an
    echo would be a false statement). Per §1.1(e) the two entries are **drafted
    here and land in RP4.1's unmask commit** — earlier they would fail
    `assert_all_hit` as NEVER APPLIED, the r4133 property compare being masked
    until then. Verbatim, to be copied into `tests/corpus/ledger.json` at RP4.1
    (*copied verbatim and **landed 2026-09-03** in RP4.1's commit `59e521e5`,
    with the cause below; both entries hit — 24 in-scope cells — on the first
    unmasked run*):

    ```json
    "swtcontrol-delay-not-wired": "EPRI r4133 never wires SwtControl property 5 `Delay`: the Edit CASE (Version8/Source/Controls/SwtControl.pas:195-218) has arms 1,2,3,4,6,7,8,9 and NO arm 5, so `delay=` lands only in the echo store (:192-193) while `TimeDelay` keeps its Create value 120.0 (:310) and the LIVE getter renders it (:588, Format('%-.7g',[TimeDelay])). dss_capi 0.14.5 wires the property through its typed table (src/Controls/SwtControl.pas:185, PropertyOffset[ord(TProp.Delay)] := ptruint(@obj.TimeDelay)) and the port follows it, so a deck that sets `delay=` reads back its own value here and 120 there. The divergence is RENDER-ONLY on r4133: nothing consumes TimeDelay there — Sample's queue-pushing body is commented out wholesale (:484-507, pushes at :492/:498, and LockCommand is commented out of the class declaration at :39 so the block no longer even compiles), DoPendingAction likewise (:396-408), and set_States acts immediately (:532-549); the two decks' event log and control queue are empty under r4133 and are live-compared. Per the 2026-08-02 policy the port keeps the correct behavior (the property is wired) and the upstream defect is reported (investigations/to_opendss/43-swtcontrol-delay-not-wired.md, local) + excluded here + pinned by the expected-value pins named in `source`. Exact-pair (a discrete value jump, not display precision)."
    ```

    ```json
    {
      "id": "r4133-swtcontrol-delay-ignored-time",
      "case": "controls:swtcontrol/swtcontrol_time.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^swtcontrol\\.sw\\.delay$",
          "rust": "0.25",
          "oracle": "120"
        }
      ],
      "cause_ref": "swtcontrol-delay-not-wired",
      "source": "R4133_PROPS_PLAN RP3.1 (2026-08-24), drafted in the sub-step and landed at RP4.1 per §1.1(e) — earlier it would fail assert_all_hit as NEVER APPLIED, the r4133 property compare being masked until the unmask. Measured by the bounded claims census (DSS_PROPS_CENSUS=claims DSS_GATE_ONLY=swtcontrol): 12 in-scope cells, one per step 0..11, element SwtControl.sw, ours 0.25 (the deck's `~ delay=0.25`) vs r4133's frozen 120, max_rel 0.9979166666666667. Replacement pin: props_r4133_pins.rs::swtcontrol_delay_wires_the_property.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

    ```json
    {
      "id": "r4133-swtcontrol-delay-ignored-midi",
      "case": "controls:swtcontrol/midi_swtcontrol.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^swtcontrol\\.sw\\.delay$",
          "rust": "0.25",
          "oracle": "120"
        }
      ],
      "cause_ref": "swtcontrol-delay-not-wired",
      "source": "R4133_PROPS_PLAN RP3.1 (2026-08-24), drafted in the sub-step and landed at RP4.1 per §1.1(e). Same pair on the second r4133-gating SwtControl deck (the IEEE123-class midi loop tie): 12 in-scope cells, steps 0..11, element SwtControl.sw, ours 0.25 vs r4133 120. The third corpus deck that sets delay= (controls:swtcontrol/swtcontrol_lock.dss) is engines: capi_v0145, where the port and the pinned 0.14.5 oracle agree, so it needs no entry. Replacement pin: props_r4133_pins.rs::swtcontrol_delay_wires_the_property_on_the_midi_tie.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

    The shape was checked against the loader
    (`corpus_gate/ledger.rs`): key `format!("{element_lower}.{prop_lower}")` →
    `swtcontrol.sw.delay`, the numeric exact-pair arm requires the `rust` pin
    (`:1093-1107`), `cause_ref` must resolve to a `causes` key (`:1453-1460`),
    and the case's `engines` must contain the channel (`:1426-1435`) — both cases
    are `engines: "r4133"`.
  - **The pins** (`crates/dss-core/tests/props_r4133_pins.rs`, both lanes, no
    oracle): `swtcontrol_delay_wires_the_property` — `SwtControl.sw.Delay` is
    `'0.25'` on `swtcontrol_time.dss`, `'7.5'` after an `edit`, and `'120'` on
    `civanlar.dss` where no deck types `delay=` (three readings, so neither a
    hardwired getter nor a parse-time echo would pass) — and
    `swtcontrol_delay_wires_the_property_on_the_midi_tie` (`'0.25'` → `'3.5'` on
    the loop tie). Because these witness a **drafted ledger entry** and not an
    `EchoRow`, the file's "no un-cited `#[test]`" guard needed a second citation
    list: `props_r4133_replay::LEDGER_ENTRY_PINS`, pinned literally like
    `NOT_A_PIN` and cross-checked against the routing row, with
    `every_echo_row_pin_is_a_test_that_exists` now matching the **union** of the
    two lists both ways.
  - **The accounting move: nothing is claimed, and that is the honest answer.**
    A settled sub-step whose artifact is staged into RP4.1 leaves its rows on the
    work list — so `DECLARED_RP3` stays **`(7, 4, 7)`** and the new
    `RP3_ROUTING` (`props_r4133_replay.rs`, the `RP38_ROUTING`/`RP39_ROUTING`
    shape) carries the per-pair split and each sub-step's verdict:
    `generator.model` 1/1 (RP3.3, open), `gictransformer.r2` 1/1 (RP3.4, open),
    `swtcontrol.delay` **2/2 (RP3.1, settled — cited, drafted, pinned)**,
    `windgen.kvar` 3/3 (RP3.2, open). Its guard re-measures the three columns
    from the walk rather than transcribing them (a sum-preserving swap of two
    pairs' row counts reds it), forbids an open sub-step from owning a pin, and
    (audit settlement, below) checks each settled verdict against the
    obligations of its own **outcome tag**.
  - **Audit settlement (2026-08-24, same day).** Ten minor findings from the
    `audit-code`/`audit-tests` pair — seven distinct issues once the two
    duplicated pairs are merged — all settled in one commit as **4 new guards +
    3 strengthened checks inside the routing guard**, plus the record
    corrections they imply. Every one is *fixed*; none was waved off or
    deferred, and no finding touched the sub-step's premise. Still no
    `ledger.json` byte and no product-crate byte:
    - **The shrink is a hand edit at RP4.1, and the docs said otherwise.** No
      link of the chain reads `tests/corpus/ledger.json` (`Link::ORDER` is four
      links; `declare` routes bin-7 root-cause rows to `Owner::Rp3`
      unconditionally), so `DECLARED_RP3` will **not** move when the entries
      land. `DECLARED_RP3`/`RP3_ROUTING` now say so, plan §RP4.1 carries the
      obligation as a numbered precondition, and the new tripwire
      `the_staged_r4133_property_entries_have_not_landed_yet` reds the moment any
      `property`-scoped `r4133` entry appears in the ledger, with the
      instruction in its message (proved live: flipping its channel filter to
      `capi_v0145` lists the five existing capi property entries).
    - **The settled-verdict contract assumed RP3.1's shape was every shape.**
      Plan §WP-RP3 sanctions three outcomes; the guard demanded a staged ledger
      entry from all of them, so a future RP3.x closing as a port fix or an echo
      row would have had to *loosen* it. Replaced by the typed taxonomy
      `RP3_SETTLED_SHAPES` (`LEDGER` / `ECHO` / `FIX`, pinned literally by
      `the_settled_outcome_taxonomy_is_the_plans_three`): shared obligations
      first, then each tag's own; an unknown tag is a hard failure naming the
      table to extend.
    - **`.pas:` was satisfiable by the capi citation.** A settled verdict must
      now cite the r4133 unit itself (`Version8/Source/` **and** `.pas:`) — the
      audit's own mutation (r4133 cite → prose, capi cite left in place) is red.
    - **"names every witness" was prefix-shadowable.** `verdict.contains(pin)`
      let `…_wires_the_property_on_the_midi_tie` stand in for
      `…_wires_the_property`; the new `names_identifier` requires a whole-token
      match and has its own self-test.
    - **The 24 = 12 + 12 decomposition lived only in prose** (mutating `24` to
      `25` stayed green). `the_rp31_census_decomposition_is_read_off_the_corpus`
      derives it — corpus-wide SwtControl sweep × `population.lock.json`
      `steps=`/`engines=`, reconciled with `bins.tsv` 42/24 and both frozen
      example rows — ties the in-scope cases to the drafted entries one-for-one,
      and requires the verdict to carry the derived figures (the `24 → 25`
      mutation is now red).
    - **Records: the census arithmetic did not close over 42.** "The remaining 6"
      was 18 (the 12 `capi_v0145` cells of `swtcontrol_lock.dss` had been elided
      and the `'0.25'` spelling is 36 cells, not 24) — corrected in the bullet
      above and in the routing verdict; and §1's RP2.4 frontier sentence, which
      still said every one of the 889 in-scope UNCLAIMED cells belongs to an
      *open* RP3.x sub-step, now records RP3.1's 24 as settled-but-staged.
    - **The upstream report understated the bug** (local file, uncommitted): the
      DDLL `Delay` **write** is dropped too — `DSwtControls.pas:135-136` routes
      it through `Set_Parameter` → `DSSExecutive.Command` (`:20-28`), i.e. back
      into the armless `Edit` `CASE`, so only the COM setter
      (`ImplSwtControls.pas:183`, a direct field write) can set what the script
      cannot. Report corrected; the drafted ledger entries never made the claim.
  - **Test count 8 368 → 8 384** (4 184 → **4 192** per lane, **+8** — summed on
    the parity lane's full run, 4 192 passed / 0 failed; the default lane's full
    run is green and its two binaries carry the same +4: `props_r4133_pins`
    31 → 33, `props_r4133_replay` 117 → 123): 2 pins + 2 replay guards from the
    sub-step, 4 more replay guards from its audit settlement, single-binary
    each, no test deleted, `#[ignore]`d or loosened.
    Every new guard was mutation-probed and red as intended (a sum-preserving
    swap of two routing rows' counts; a pin renamed out from under its citation;
    `24 → 25` in the verdict's census; the r4133 citation replaced by prose; the
    short pin dropped in favour of its longer prefix-mate; a control count moved
    against the deck; an unknown outcome tag; the ledger tripwire's channel).
    Report: `investigations/to_opendss/43-swtcontrol-delay-not-wired.md`
    (gitignored, local-only — verified absent from both commits).
    **Open follow-up, out of RP3.1's render-only scope:** the port's `Sample`
    still queues at `time_delay` where r4133's body is dead
    (`swt_control/mod.rs:189-229`, which already carries the NOTE) — a behavioral
    lock-path question owned by whoever retires that body, not by this sub-step.
    Second: `swtcontrol_lock.dss` carries the same 12 divergent cells but is
    `engines: capi_v0145`, so if a later WP gates it on r4133 a third entry is
    due (recorded in entry 2's `source` so the fact cannot be lost).
    Third, for **RP4.1**: landing the staged entries is also an accounting
    commit — retire the settled `RP3_ROUTING` rows, shrink `DECLARED_RP3` by
    exactly those rows and re-state the tripwire (plan §RP4.1 precondition 2).

- **RP3.2** (2026-08-24) — `windgen.kvar`: **a wired property whose r4133 getter
  reads the wrong live field.** **Zero product-crate bytes** (the port already
  behaves correctly), **zero `ledger.json` bytes**, zero golden / manifest /
  `population.lock` / frozen-extract bytes, and no r4133 mask moved — two test
  files plus docs. The plan's three outcomes resolved to **LEDGER**; the kill
  criterion did **not** fire (the probe separated echo from live cleanly).
  - **Root cause, verified against the vendored r4133 source.** `WindGen`
    declares `kvar` as property 11 and documents it as "Specify the **base
    kvar**" (`Version8/Source/PCElements/WindGen.pas:364-365`, verbatim the
    `Generator` help at `Generator.pas:396`). The write path works — `Edit` arm
    11 exists (`:629`, `11: Presentkvar := Parser.DblValue`) and
    `Set_Presentkvar` stores the value in `kvarBase` (`:2996-3009`). The **read**
    path does not: `GetPropertyValue` arm 11 is `Format('%.6g',[presentkvar])`
    (`:2896`) and `Get_Presentkvar` returns
    `WindGenvars.Qnominalperphase*0.001*Fnphases` (`:2297-2300`) — the
    *dispatched* Q. So this is **not** RP3.1's shape (an unwired property) and
    not an echo either: the echo store's own default for the slot is `'60'`
    (`:2446`) and is unreachable, `TDSSObject.Get_PropertyValue` being virtual
    (`DSSObject.pas:117-120`). The axis is **live-field-A vs live-field-B**.
    r4133 contradicts itself inside its own trunk: `Generator` has the
    **identical** getter (`Generator.pas:2402-2405`) and the identical help, yet
    renders the base (`Generator.pas:3018`, `Format('%.6g',[kvarBase])`) — which
    is what the port does (`elements/pc/windgen/accessors.rs:431`, the twin of
    `pc/generator/accessors.rs:495`).
  - **The live probe (summarized; driver = `epri-worker` over the git-tracked
    `tools/opendss/bin/r4133/OpenDSSDirect.dll`, banner `Version 11.0.0.1
    (64-bit build) - Charlottesville`; the temporary port-side example was
    deleted, nothing under `tests/corpus` was written).**
    - **Decisive — not an echo.** A scratch deck typing `kW=3000 kvar=500`:
      r4133 `? WindGen.w1.kvar` → **`0`**, port → `500`. An echo store would have
      printed `500`.
    - **The value IS parsed; only the render ignores it.** `Edit WindGen.w1
      kvar=777` → r4133 kvar `0` **but PF `0.968058`** (= 3000/√(3000²+777²)),
      port `777` / `0.968057839822749` — `Set_Presentkvar`'s side effect fired
      identically on both engines.
    - **Not a port bug — the physics already agree.** Solved terminal powers on
      `WindGen.w1`, the observable independent of the property string, over all
      five corpus decks: Q **−2.13e-05 / −4.22e-05 kvar** in power flow on both
      engines; in dynamics **−37 087.81 (port) vs −37 087.76 (r4133)** and
      **−29 216.72 vs −29 216.67**. The port ports `SetNominalGeneration`
      loop-for-loop, `Else kvarCalc := 0` (`:1320-1321`) included
      (`windgen/nominal.rs:223-225`), so **no engine change was owed** — and
      adopting `presentkvar` as our render would be *reproducing* an upstream
      defect, which the 2026-08-02 policy bars.
    - **Wrong even when the dispatch works.** With `QMode=1` (the PF arm,
      `:1277`) the same deck renders **`363.54`** — the operating-point Q — for a
      typed `kvar=500`; terminal Q −363.540715 (r4133) vs −363.540715423 (port).
    - **In dynamics the render is a stale intermediate.** `:1254` skips the Q
      block, so after `Edit kvar=777` on `windgen_dyn.dss` r4133 renders **777**
      while its own `kvarBase` is **792.718441186736** (the port's value, reached
      through the identical `PFNominal 0.897802`) and the measured terminal Q is
      **−37 087.76 kvar** — three numbers, the render tracking none.
    - **Round-trip model corruption, produced not predicted.** `Save Circuit` on
      r4133 wrote `New "WindGen.w1" … kW=3000 kvar=0 …` for the machine built
      with `kvar=500` (the port wrote `kvar=500`), via `TDSSObject.SaveWrite`
      reading `PropertyValue[]` (`DSSObject.pas:156`) through the virtual getter;
      reloading then also resets `PFNominal`→1.0 and `kvarMax`/`kvarMin`→0
      (`:3001-3008`).
  - **Census, derived and closed over every cell.** 4 cells / **4 in scope**,
    `1 + 1 + 1 + 1` over the four diverging decks (`bins.tsv:303`
    `windgen.kvar numeric 7 - 4 4 9.86e+02 9.86e+02`; `examples_full.txt:3375-3377`
    `'854.95263026673'|'0'|2`, `'726.483157256779'|'0'|1`,
    `'986.05231553659'|'0'|1`). All five `modes:windgen/*` decks are
    `steps=1 … engines=r4133` in `population.lock.json`, one `WindGen.w1` each.
    **No deck types `kvar=` at all** — every value is a side effect: `kVA` unset
    ⇒ `kW·√(1/pf²−1)` (3000/0.95 → 986.05231553659; 1500/0.9 →
    726.483157256779), `kVA` set ⇒ `√(kVA²−(kVA·|pf|)²)` at `Create`'s pf 0.88
    (`:917`) ⇒ kWBase 1584, 854.95263026673 on both dyn decks. The fifth deck,
    `modes:windgen/windgen_snap.dss`, types `pf=1.0`, so **our** base is 0 and
    both engines print `0` — a value coincidence, **not** a mode story: typing
    `kvar=777` there diverges 777 vs 0 (probed, and pinned). Beyond the
    population, exactly **two** further corpus decks declare a WindGen
    (`…/WindGenerator/WindGen_GFL_Dynamics/` and `…/WindGen_QSTS/`
    `Run_IEEE123Bus_GFLDaily.DSS`); both sit in
    `tests/corpus/manifests/skipped_oracle_issue.json` under
    `capi015_multistep_limitation`, own no cell and owe no entry — and each would
    derive a **nonzero** base (~569.97) if ever promoted, i.e. `WindGens × steps`
    new in-scope cells and a re-derived entry count. What r4133 renders on those
    two is **not** claimed: no channel has ever run them, and both type
    `QMode=2` with a real `VV_Curve=`, so they take the volt-var arm
    (`WindGen.pas:1289-1319`) rather than the `Else kvarCalc := 0`
    (`:1320-1321`) the five census decks take. Since this sub-step the
    whole decomposition is **derived, not transcribed**:
    `props_r4133_replay::the_rp32_census_decomposition_is_read_off_the_corpus`
    sweeps every corpus `.dss` for `WindGen` declarations — quoted spelling
    included, and `Edit`/`BatchEdit` lines count as the same element scope since
    the audit settlement — reads each one's `kW=`/`pf=`/`kVA=`/`kvar=` and
    `QMode=`/`VV_Curve=` tokens there, re-derives the base through the two
    `WindGen.pas` branches, splits on the
    lock's `steps=`/`engines=`, reconciles per spelling and in total against the
    frozen extracts (each of whose r4133 columns must be `0`), and requires the
    in-scope cases to be exactly the cases the drafted entries cite. One measured
    correction to the hand-written table it replaced: the QSTS deck writes
    `PF=0.88` explicitly on a `~` continuation rather than inheriting it.
  - **The exclusion shape is a ledger entry, never an echo row** — the getter is
    live, so an echo row would misname the mechanism, and `props_norm.rs` carries
    none for this pair (only `windgen.dynout`). Per §1.1(e) the **four** entries
    (one per diverging case; the fifth deck owes none) are **drafted here and
    land in RP4.1's unmask commit**. Verbatim, to be copied into
    `tests/corpus/ledger.json` at RP4.1 (*copied verbatim and **landed
    2026-09-03** in RP4.1's commit `59e521e5`, with the cause below; all four
    hit — 4 in-scope cells — on the first unmasked run*):

    ```json
    "windgen-kvar-renders-dispatched-q": "EPRI r4133 renders WindGen property 11 `kvar` from the DISPATCHED reactive power instead of the base kvar the property documents: GetPropertyValue arm 11 (Version8/Source/PCElements/WindGen.pas:2896) is Format('%.6g',[presentkvar]) and Get_Presentkvar (:2297-2300) returns WindGenvars.Qnominalperphase*0.001*Fnphases. The Edit arm EXISTS (:629 `11: Presentkvar := Parser.DblValue`) and Set_Presentkvar (:2996-3009) stores the value in kvarBase, so this is NOT an echo: probed live on r4133 via epri-worker, a deck typing `kvar=500` renders `0`, and after `Edit kvar=777` the render is still `0` while `? PF` renders 0.968058 (= 3000/sqrt(3000^2+777^2)) — the value IS parsed and the getter reports a different live field. The rendered zero on these decks comes from a second, independent upstream defect: the steady-state `case WindModelDyn.QMode` (:1276-1322) implements arms 1 (PF) and 2 (Volt-Var) but has NO arm 0, while QMode defaults to 0 (:1020) and both the property help (:429-430 'Q control mode (0:Q, 1:PF, 2:VV).') and WTG3_Model.pas:252 document 0 as constant-Q — so `Else kvarCalc := 0` (:1320-1321) zeroes the dispatch and the getter reports that zero. The render is wrong independently of it: with QMode=1 the same deck renders 363.54 for a typed kvar=500 (the operating-point Q), and in dynamics — where :1254 skips the Q block — it renders Set_Presentkvar's intermediate 777 while kvarBase is 792.718441186736 and the measured terminal Q is -37087.76 kvar. The sibling class settles it inside the same trunk: Generator has the identical Get_Presentkvar (Generator.pas:2402-2405) and the identical property help (:396) yet renders the base (Generator.pas:3018, Format('%.6g',[kvarBase])). Consequence beyond the API: TDSSObject.SaveWrite reads PropertyValue[] (DSSObject.pas:156), which is virtual-dispatched to this getter (DSSObject.pas:117-120), so `Save Circuit` writes `kvar=0` for a machine built with kvar=500 (produced on r4133) and the reload resets PFNominal to 1.0 and kvarMax/kvarMin to 0 (:3001-3008) — silent model corruption. dss_capi 0.14.5 has no WindGen class at all, so there is no second oracle witness; the port renders kvar_base (crates/dss-core/src/elements/pc/windgen/accessors.rs:431), exactly as its Generator does and as r4133's own Generator does. Per the 2026-08-02 policy the port keeps the correct behavior and the upstream defect is reported (investigations/to_opendss/44-windgen-kvar-renders-dispatched-q.md, local) + excluded here + pinned by the expected-value pins named in `source`. The two engines' SOLVED state is unaffected and stays fully compared: probed terminal powers agree on every windgen deck (power-flow Q -2.1e-05/-4.2e-05 kvar on both; dynamics -37087.81 vs -37087.76 and -29216.72 vs -29216.67 kvar). Exact-pair (a discrete value jump, not display precision)."
    ```

    *(**Correction, 2026-09-04, §RP3.10.** The cause text quoted above closes with
    "the two engines' SOLVED state is unaffected and stays fully compared" — true
    only while the port still reproduced r4133's missing `QMode=0` arm. §RP3.10
    stopped reproducing it, so on the four decks that declare a WindGen with a
    non-zero base the solved model now diverges and is excluded by the
    `windgen-qmode0-constant-q-{snapdelta,daily,dyn,dynfault}-r4133` entries; the
    live `tests/corpus/ledger.json` text lost the claim in the same commit. The
    `property` pair below is unaffected — our `kvar` renders `kvar_base`, which the
    dispatch never writes. Full record: §RP3.10 below.)*

    ```json
    {
      "id": "r4133-windgen-kvar-dispatched-daily",
      "case": "modes:windgen/windgen_daily.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^windgen\\.w1\\.kvar$",
          "rust": "986.05231553659",
          "oracle": "0"
        }
      ],
      "cause_ref": "windgen-kvar-renders-dispatched-q",
      "source": "R4133_PROPS_PLAN RP3.2 (2026-08-24), drafted in the sub-step and landed at RP4.1 per §1.1(e) — earlier it would fail assert_all_hit as NEVER APPLIED, the r4133 property compare being masked until the unmask. 1 in-scope cell (steps=1, one WindGen.w1): ours 986.05231553659 = kW*sqrt(1/pf^2-1) for the deck's kW=3000 pf=0.95 (SyncUpPowerQuantities, kVA not set) vs r4133's dispatched 0, max_rel 9.86e+02 — the pair's worst cell, bins.tsv:303. Replacement pin: props_r4133_pins.rs::windgen_kvar_renders_the_base_on_the_daily_deck.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

    ```json
    {
      "id": "r4133-windgen-kvar-dispatched-delta",
      "case": "modes:windgen/windgen_snap_delta.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^windgen\\.w1\\.kvar$",
          "rust": "726.483157256779",
          "oracle": "0"
        }
      ],
      "cause_ref": "windgen-kvar-renders-dispatched-q",
      "source": "R4133_PROPS_PLAN RP3.2 (2026-08-24), drafted in the sub-step and landed at RP4.1 per §1.1(e). Same getter on the delta snapshot deck: 1 in-scope cell, ours 726.483157256779 = 1500*sqrt(1/0.9^2-1) vs r4133's 0. The fifth windgen deck, modes:windgen/windgen_snap.dss, needs NO entry: it types pf=1.0, so our kvar_base is 0 and both engines render `0` — a value coincidence, not agreement (typing `kvar=777` there diverges 777 vs 0, probed). Replacement pin: props_r4133_pins.rs::windgen_kvar_renders_the_base_on_the_delta_snapshot.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

    ```json
    {
      "id": "r4133-windgen-kvar-dispatched-dyn",
      "case": "modes:windgen/windgen_dyn.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^windgen\\.w1\\.kvar$",
          "rust": "854.95263026673",
          "oracle": "0"
        }
      ],
      "cause_ref": "windgen-kvar-renders-dispatched-q",
      "source": "R4133_PROPS_PLAN RP3.2 (2026-08-24), drafted in the sub-step and landed at RP4.1 per §1.1(e). The kVA-set derivation: the deck types kW=1500 kva=1800 and no pf, so Create's PFNominal 0.88 (WindGen.pas:917) drives RecalcElementData's kVANotSet=false branch (:1377-1378) to kWBase 1584 and kvarBase sqrt(1800^2-1584^2) = 854.95263026673, vs r4133's 0. This deck is also where the upstream render is provably stale rather than merely wrong: :1254 skips the Q block in dynamics, so after `Edit kvar=777` r4133 renders Set_Presentkvar's intermediate 777 while its own kvarBase is 792.718441186736 (the port's value, reached through the identical PFNominal 0.897802) and the measured terminal Q is -37087.76 kvar. Replacement pin: props_r4133_pins.rs::windgen_kvar_renders_the_base_on_the_dynamics_deck.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

    ```json
    {
      "id": "r4133-windgen-kvar-dispatched-dynfault",
      "case": "modes:windgen/windgen_dyn_fault.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^windgen\\.w1\\.kvar$",
          "rust": "854.95263026673",
          "oracle": "0"
        }
      ],
      "cause_ref": "windgen-kvar-renders-dispatched-q",
      "source": "R4133_PROPS_PLAN RP3.2 (2026-08-24), drafted in the sub-step and landed at RP4.1 per §1.1(e). The fault-ride-through twin of r4133-windgen-kvar-dispatched-dyn — same kW=1500 kva=1800 pf-default derivation, same 854.95263026673 vs 0, on the deck whose sustained 3ph fault drives the LVPL/LVQL path (terminal Q -29216.72 kvar on the port vs -29216.67 on r4133, i.e. the solved state agrees and only the render diverges). Entries are per case, so this one is its own. Replacement pin: props_r4133_pins.rs::windgen_kvar_renders_the_base_on_the_fault_ride_through_deck.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

    Shape checked against the loader (`corpus_gate/ledger.rs`): key
    `format!("{element_lower}.{prop_lower}")` → `windgen.w1.kvar` (`:1051`); no
    `num_rel`, so the **exact-pair-numeric** arm, which requires `rust`
    (`:1088-1107`); `oracle` is asserted against the live upstream render
    (`:1057-1064`), so `"0"` must stay exact; `cause_ref` must resolve to a
    `causes` key (`:1453-1460`); and each case's `engines` must contain the
    channel (`:1426-1435`) — all four are `engines: "r4133"`.
  - **The pins** (`crates/dss-core/tests/props_r4133_pins.rs`, both lanes, no
    oracle), four, one per drafted entry, each running the gate's own sequence
    (compile + the one `solve` the case's `steps=1` rigor prescribes — the gate
    issues that solve itself for every case, `corpus_gate/runner.rs:348-349`, and
    none of the four decks ends on the solve of its own mode: the daily and the
    two dynamics decks end at `Set mode=…` and `windgen_snap_delta.dss` at
    `Calcvoltagebases`, while the two dynamics decks do carry a *snapshot*
    `solve` before their `Set mode=dynamic`) and then the same `?` getter the
    property walk reads:
    `windgen_kvar_renders_the_base_on_the_daily_deck` (`986.05231553659`, then
    `edit kvar=777` → `777` **and** `PF` → `0.968057839822749`, the reading that
    makes the assertion about the value rather than about a constant),
    `windgen_kvar_renders_the_base_on_the_delta_snapshot` (`726.483157256779` →
    `777`, plus the load-bearing third reading on `windgen_snap.dss`: `0` unset,
    `777` once typed — the `civanlar.dss` analogue),
    `windgen_kvar_renders_the_base_on_the_dynamics_deck` (`854.95263026673`, then
    `edit kvar=777` → **`792.718441186736`** at `PF 0.897802095552545`, the
    `kVANotSet=false` re-derivation r4133 also performs and then fails to render)
    and `windgen_kvar_renders_the_base_on_the_fault_ride_through_deck`. All four
    are cited from `props_r4133_replay::LEDGER_ENTRY_PINS` (2 → **6** rows, the
    literal pin extended), so the union guard
    `every_echo_row_pin_is_a_test_that_exists` still matches both ways.
  - **The accounting move: nothing is claimed.** A LEDGER outcome does not empty
    its rows — the exclusion lands at RP4.1 — so `DECLARED_RP3` stays
    **`(7, 4, 7)`** and the `RP3_ROUTING` row keeps its `3, 3` columns, its
    verdict flipped `OPEN` → **`LEDGER — RP3.2 (…)`**. The settled set pinned in
    `the_bin7_root_cause_pairs_are_routed_to_their_sub_steps` was then
    `[("swtcontrol.delay", "RP3.1"), ("windgen.kvar", "RP3.2")]` (RP3.3 put
    `("generator.model", "RP3.3")` at its head the same day); the table's
    state is `generator.model` 1/1 (RP3.3, open), `gictransformer.r2` 1/1 (RP3.4,
    open), `swtcontrol.delay` 2/2 (RP3.1, settled — LEDGER),
    `windgen.kvar` **3/3 (RP3.2, settled — LEDGER)**. RP3.2 is the first
    sub-step to exercise `RP3_SETTLED_SHAPES` as a *taxonomy* rather than as
    RP3.1's shape: the guard's `LEDGER` branch checked the r4133 citation, the
    `RP4.1`/`§1.1(e)` staging clause, the four witnesses as whole identifiers and
    the absence of an echo row.
  - **Test count 8 384 → 8 394** (4 192 → **4 197** per lane, **+5**):
    `props_r4133_pins` 33 → **37** (the four pins), `props_r4133_replay`
    123 → **124** (the census derivation), single-binary each, no test deleted,
    `#[ignore]`d or loosened. Every new assertion was mutation-probed and red as
    intended: the daily pin's expected value flipped one digit; the verdict's
    derived `4 cells, all 4 in scope` mutated to `5`; a witness name dropped from
    the verdict (`names_identifier` red). The completeness sweep proved itself
    *unprompted* — it rejected the inherited claim that neither vendored deck
    types `pf=`, which is how the QSTS `PF=0.88` correction above was found.
    Report: `investigations/to_opendss/44-windgen-kvar-renders-dispatched-q.md`
    (gitignored, local-only — verified absent from the commit).
  - **Flagged, deliberately NOT acted on — and, since the audit settlement,
    OWNED by plan §RP3.10.** **D2 — the missing steady-state `QMode=0`
    (constant-Q) arm, reproduced by the port.**
    `WindGen.pas:1276-1322` implements arms 1 and 2 only; `QMode` defaults to 0
    (`:1020`); the help (`:429-430`) and `WTG3_Model.pas:252` both document
    `0 -> Constant Q`, and the dynamics model implements it (`:1059-1061`,
    `Qord := Qref`); `Else kvarCalc := 0` (`:1320-1321`) therefore zeroes the
    dispatch for every default-configured WindGen, and the port ports it verbatim
    (`windgen/nominal.rs:223-225`) — which is why the corpus gate's power channel
    is green. New probe evidence that it is an omission, not a design choice:
    r4133 dispatches correctly the moment an arm exists (`QMode=1` → −363.54 kvar
    below the aero cap, −985.69 kvar on the daily deck).
    *(**Correction, 2026-09-04, §RP3.10's probe.** The daily deck's `QMode=1`
    figure is **−414.8954549079094 kvar** under the gate's own replay
    (`Set mode=daily stepsize=1h number=1`, `Pg = 1262.29 kW`); the ≈ −986 family
    is the snap-mode capped-`Pg` reading, where arm 1 sits on its saturation
    fallback `kvarCalc := kvarBase = 986.0523155365896` — which is also what the
    constant-Q arm dispatches. Never −985.69; see the §RP3.10 record below.)* Under the 2026-08-02
    policy a reproduced upstream bug may not stand, but fixing it **moves solved
    powers on the r4133-gated windgen decks** — directly on the two power-flow
    ones, and on the two dynamics decks only through the snapshot solve they run
    before `Set mode=dynamic` (`:1254` skips the Q block in dynamics), which
    §RP3.10 measures rather than predicts — and is far outside RP3.2's
    property-render scope. It does not interact with this landing: the port
    renders `kvar_base`,
    which the dispatch never touches, so the four drafted `rust` values survive a
    future D2 fix unchanged. **The audit round refused to leave it at "flagged"**
    — a LEDGER outcome bars product-crate bytes, so the site
    (`windgen/nominal.rs:223-225`) carries no note and the item would have lived
    only in prose. It is now plan **§RP3.10** ("the reproduced `QMode=0`
    dispatch"), `opus-xhigh`, with its own precondition (**the user's go-ahead**,
    since it is a both-lane behavior change owing per-case *power*-channel ledger
    entries) and its own place in the ordering: it blocks **§RP5.2**, the closing
    record, and **not RP4.1** — measured, because the unmask compares properties
    and our `kvar` render reads `kvar_base`
    (`elements/pc/windgen/accessors.rs:431`), which the dispatch never writes.
    Two smaller flags, corpus/manifest bytes being
    off-limits this sub-step: `modes/windgen/windgen_snap_delta.dss:3` and its
    manifest note claim "the QMode=0 (constant-Q via PF) reactive dispatch … PF
    at unity here because the aero cap makes kvarCalc saturate" — **both clauses
    are wrong** (there is no arm 0; the `Else` yields 0 outright, now measured);
    and `WindGen.pas:2954`'s `MakePosSequence` gates on `PrpSequence^[19]/[20]`
    and emits ` maxkvar=`/` minkvar=`, but WindGen's properties 19/20 are
    `UserData`/`DutyStart` (`:394`, `:397`) — Generator's indices, a copy/paste
    leftover (`windgen/mod.rs:388` already notes the port registers no such rows).
  - **Audit settlement (2026-08-24, same day).** Ten minor findings from the
    `audit-code`/`audit-tests` pair — eight distinct issues once the two
    duplicated pairs are merged — settled in one commit as **2 hardened readers +
    3 new assertions + one newly owned follow-up**, plus the record corrections
    they imply. None touched the sub-step's premise (LEDGER stands: the probe,
    the census and the four entries are unchanged), and the settlement is again
    **zero product-crate bytes, zero `ledger.json` bytes**, no golden / manifest
    / `population.lock` / frozen-extract byte, no mask move. Test count is
    unchanged at **4 197 per lane** — every new assertion lives inside the
    existing census derivation.
    - **The completeness sweep read only unquoted declarations.**
      `windgen_facts` recognized an element by `lower.starts_with("new
      windgen.")`, so the quoted form `New "WindGen.w1"` — which the vendored
      corpus does use for other classes (`Test/IndMachTest.DSS:108`, `New
      "IndMach012.windgen1"`) — would have walked past "exactly four entries"
      unnoticed. Both census readers now share `element_scope`, which accepts the
      quoted spelling; `swtcontrol_facts` (RP3.1's) inherits the fix.
    - **"No deck types `kvar=`" was swept over declarations only.** An `Edit
      WindGen.w1 kvar=500` was invisible to the reader, and the audit proved it:
      that mutation on `windgen_daily.dss` left the whole replay binary green
      (124 passed) while the port's render moved. `element_scope` now treats
      `Edit`/`BatchEdit` as the same element scope, so the claim is swept over
      the lines that can make it false — the mutation is red (`measured ==
      cited`), on top of the pin that already caught it.
    - **The held-out pair's argument leaned on an unmeasured r4133 value.**
      `RP32_WINDGEN_SKIPPED_DECKS`' doc (and STATUS) said each vendored deck
      "would derive ~569.97 **against r4133's `0`**", but nothing has ever run
      them, and their mechanism is not the census's: both type `QMode=2` with a
      real `VV_Curve=`, i.e. the volt-var arm (`WindGen.pas:1289-1319`), not the
      `Else kvarCalc := 0` (`:1320-1321`) the five census decks take. The claim
      is now the nonzero base alone (`WindGens × steps` cells and a re-derived
      entry count on promotion, not "a cell"), and the `QMode=` token is read off
      every deck by the new `windgen_dispatch`: the five must select **no** arm
      (that IS where r4133's `0` comes from — the mechanism is derived now, not
      asserted) and the two held-out ones must be the volt-var pair the doc
      argues from.
    - **The pins' own justification was false for three of the four decks.**
      "The four decks end at `Set mode=…` and carry no solve of their own" holds
      only for `windgen_daily.dss`: `windgen_snap_delta.dss` ends at
      `Calcvoltagebases`, and both dynamics decks carry a *snapshot* `solve`
      before their `Set mode=dynamic`. The pins are unaffected — the gate issues
      the mode solve itself for every case (`corpus_gate/runner.rs:348-349`), and
      the pins do exactly that — but both copies of the wording (the pins' block
      comment and STATUS) now say what the decks contain.
    - **Citation drift, corrected in every copy.** `Set_Presentkvar`'s "init to
      something reasonable" line is `WindGen.pas:3002`, not `:2998` (a `Var`
      declaration) — wrong in the dynamics pin's doc comment and in the local
      report 44, fixed in both. Re-checking the report's Pascal block turned up a
      second one, report-only: its volt-var arm is `:1289`, not `:1300`. The
      report also attributed the daily deck's `QMode=1`
      flip (terminal Q −985.69 kvar) to its own reproduction deck, which measures
      −363.54 — the two probes are now separated there, as they already were
      here and in the routing verdict. *(**Correction, 2026-09-04:** the daily
      deck's `QMode=1` terminal Q is −414.8954549079094 kvar, not −985.69; the
      ≈ −986 reading belongs to the snap-mode capped-`Pg` configuration. Measured
      by §RP3.10's probe, recorded in its record below.)*
    - **The commit message of `c46bca42` welded two probe steps.** It reads "a
      typed `kvar=500` renders 0 while PF moves to 0.968058"; at `kvar=500` r4133
      renders PF `0.986394` (= 3000/√(3000²+500²)), and `0.968058` is the result
      of the *later* `Edit kvar=777`. The tree's own records (this section, the
      `RP3_ROUTING` verdict, the drafted cause) already split them correctly, so
      the message stands as written and the correction is recorded here rather
      than by rewriting the sub-step's commit.
    - **D2 stopped being a flag and became a sub-step.** The port reproduces
      r4133's missing steady-state `QMode=0` arm, which the 2026-08-02 policy
      forbids in any lane; RP3.2 could not fix it (a LEDGER outcome bars
      product-crate bytes, so not even a note at the site), which left a real
      policy item living in prose. It is now plan **§RP3.10**, with an explicit
      precondition (the user's go-ahead) and an explicit place in the ordering
      (blocks §RP5.2, not RP4.1). See the flagged bullet above.

- **RP3.3** (2026-08-24) — `generator.model`: **a getter with no arm, echoing the
  deck's own token past a live conversion.** **Zero product-crate bytes** (both
  engines' live state is identical), **zero `ledger.json` bytes**, zero golden /
  manifest / `population.lock` / frozen-extract bytes, and no r4133 mask moved —
  four test files (one of them a single doc-comment count) plus docs. The plan's
  three outcomes resolved to **ECHO** —
  the first sub-step to take that tag, RP3.1 and RP3.2 having both been
  `LEDGER`; the kill criterion did **not** fire
  (a live-field DLL getter separates the model state from the rendered string,
  and the census's factual basis re-verified exactly: 2 cells, exactly
  `modes:ncim/ncim_pv_pq.dss` + `modes:ncim/ncim_midi.dss`).
  - **The plan's premise was wrong, and disproving it was the sub-step.** Plan
    §RP3.3 (and the `RP3_ROUTING` `OPEN` verdict, and the sub-step's own state
    file) said r4133 "later **takes it back to 3**"
    (`Version8/Source/Common/Solution.pas:1760`). The *line* exists —
    `ReversePQ2PV` (`:1743-1768`, declared `:372`) carries the comment
    `// Takes it back to model 3` — but the procedure **has no caller anywhere in
    the trunk**. Exhaustive grep over the vendored r4133 tree returns the
    declaration, the definition, and `VersionC/Common/Solution.cpp:1360` plus
    `VersionC/Common/Solution.cpp:827`, which is the call **commented out**
    (`// ReversePQ2PV(ActorID); - not needed for now (04/01/2024)`).
    `DoNCIMSolution` (`:1095-1161`) ends at its `Until` with nothing after it;
    the sibling `DistGenClusters` (`:1687-1723`) is dead the same way. So a
    Q-clamped generator's live `GenModel` stays **4** after a converged NCIM
    solve, and the only live reversion is the in-loop `:2229`, which needs
    `not myPQOK` (`VNode > myVMax`) and cannot fire on decks clamped *upward*.
  - **The render, and why it is `EchoParse`.** `TGeneratorObj.GetPropertyValue`
    (`Version8/Source/PCElements/generator.pas:3007-3038`) has arms
    3,4,5,7,8,9,13,19,20,26,27,34,36,37,38,40..46 and **no arm 6**, so index 6
    falls to `ELSE Result := Inherited` (`:3035-3036`) →
    `General/DSSObject.pas:112-115` `Result := FPropertyValue[Index]`. The only
    writers of that slot are `InitPropertyValues`' `'1'` (`:2559`) and the `Edit`
    loop's unconditional store write (`:625`) — which runs *before* the CASE
    assigns the live field at `:643`. Both decks type `model=3` explicitly, so
    the store holds the deck's own token, not the class default: `EchoParse`, the
    `relay.reset` shape, not `EchoDefault`. The census read path is exactly that
    getter (`crates/dss-epri/src/capture.rs` `? <element>.<prop>` →
    `Executive/ExecHelper.pas:1755` `ActiveDSSObject.GetPropertyValue`).
  - **The probe — live r4133 DLL through `epri-worker`** (banner
    `Version 11.0.0.1`, `oracle.rev = r4133`), reading a channel independent of
    the property string: `GeneratorsI(9, 0)` is
    `Result := TGeneratorObj(Active).GenModel` (`DDLL/DGenerators.pas:125-134`),
    the field itself. `errno = 0` on every call.
    - decks as shipped: live `GenModel` **4**, render **`'3'`**, `kvar` 0.0,
      converged in 4 iterations, on both;
    - the same decks minus the trailing `Solve`: live 3 → **4** across the solve
      while the render is frozen at `'3'` the whole time (pre-solve present kvar
      `431.79425771046976` / `323.84569328285227`);
    - a **second** `Solve`: still 4 — the behavioural confirmation that
      `ReversePQ2PV` is dead;
    - **the decisive pair, neither of which runs the solver.**
      `GeneratorsI(10, 4)` writes the live field and nothing else
      (`DGenerators.pas:135-147` never touches `PropertyValue`): live → 4, render
      **stays `'3'`** ⇒ the getter does not read `GenModel`. `Edit Generator.g1
      model=4` writes the store (`generator.pas:625`): render **becomes `'4'`**
      ⇒ the getter reads `FPropertyValue[6]`. `Dump` writes `~ model=3` and
      `Save Circuit` writes `model=3` after the converged solve — the same store,
      so r4133's own round trip re-creates a model-3 generator.
    - control: `ncim_pq.dss` has 0 generators live, as its deck says.
  - **FIX and LEDGER were refuted by measurement, not by argument.** The port
    leg (temporary integration test, public API only, removed afterwards)
    reproduces r4133 digit for digit: render 3 → 4 across the solve, present kvar
    `431.79425771046976` / `323.84569328285227` before and `0` after, 4
    iterations, on both decks. So there is no behavioural divergence to fix **on
    any compared channel** — the one surface no channel compares, `Save`/`Dump`
    re-serialization, is bounded and owned by §RP3.11 (the settlement bullet
    below); and
    the `GeneratorsI(10, 4)` probe rules out "live but wrong", which is the only
    shape a ledger entry would have fitted. The port renders the live field
    (`obj/props/class_props/value.rs` → `elements/pc/generator/accessors.rs`) and
    its NCIM (`solution/solution/ncim.rs`) ports r4133's *executed* code
    loop-for-loop, including not restoring the model — which is now known to be
    correct rather than a gap. *(**Correction 2026-09-03, §RP3.13:**
    "loop-for-loop" no longer holds everywhere in that file — two array overruns
    the faithful port had inherited, `InitPQGen`'s length-1 `deltaQNom` sizing
    and `DOForceFlatStart`'s unconditional `node_v[1..3]` write, are proven
    r4133 defects and are **not** reproduced; the model-restore reading above is
    untouched.)*
  - **Landed: one echo row, one pin, one derivation.** The row is
    `echo("generator", "model", EchoParse, 2, …, Pin(…))`, the **82nd** in
    `PROPS_ECHO_R4133` and the first contributed by a WP-RP3 sub-step rather than
    by RP2.3's declared bucket. Its witness can only be a pin: both cells sit on
    `engines: "r4133"` cases, so the capi channel never value-compares them (a
    `Capi(n)` witness would be the witness-that-cannot-exist the RP2.3 settlement
    made a test) — and `ECHO_ROWS_ON_R4133_ONLY_CASES` gains
    `("generator", "model", 2, 2)`, its most extreme entry, where *every* cell of
    the pair is capi-blind. The pin
    `generator_model_renders_the_live_pv2pq_conversion` covers **both** decks
    (`ncim_midi.dss` had no unit pin at all): it reads `maxkvar` first, which
    names *which* conversion this is (the `:2120` Q-band promote, not the
    `:1935` zero-limits one), asserts the render `4`, types the deck's own token
    back (`edit … model=3` → `3`) and then **re-solves** — `4` again, so the pin
    is on the engine's field and not on the parser's echo.
  - **The census derivation.** `the_rp33_census_decomposition_is_read_off_the_corpus`
    (RP3.1/RP3.2's precedent) derives the `2` instead of transcribing it: the new
    reader `sets_ncim` sweeps all 1 000+ corpus decks for a live `set
    algorithm=NCIM` line — a line mentioning `algorithm` in an unparsable
    spelling is a hard error, not a silent miss — and finds exactly **five**;
    `generator_model_facts` then reads each survivor's `Generator` count and
    `model=` token. Products: `cells = generators × steps` = 1 + 1 = **2**, both
    `engines=r4133` ⇒ 2 in scope, reconciled against `bins.tsv`'s 2/2 and the
    single frozen example row `'4'` vs `'3'`. Two things are *derived* rather
    than asserted: `ncim_pq.dss` runs NCIM and declares no generator, hence no
    cell (the control — the mechanism alone makes nothing); and per case,
    r4133's frozen side must equal the deck's own typed token, which **is** the
    `EchoParse` claim. **Held out, named:** the corpus's two other NCIM decks —
    `IEEETestCases/IEEE118Bus/master_file.dss` (53 `model=3` generators) and
    `Examples/NCIM/Xmission_System_Kundur2Area/Master.dss` (3) — are
    `kind: "large"` and therefore outside the census population ("every live
    case, **non-large**, non-pending/abort/defer",
    `tests/corpus/props_r4133/triage.md` §Method), asserted from
    `population.lock.json`. Their generators live in *redirected* files, so the
    test reads 0 on each master and the real count on the sibling — the reason
    the table carries a sibling column at all.
  - **The accounting move: RP3.3 is the first sub-step that shrinks the
    bucket, and the tag is why.** An ECHO outcome ships its exclusion in the
    sub-step's own commit, so `Link::Echo` claims `generator.model`'s example row
    immediately and `declare` never routes it to `Owner::Rp3` again:
    `DECLARED_RP3` **`(7, 4, 7)` → `(6, 3, 6)`** and the `RP3_ROUTING` row's
    counted columns go **`1, 1` → `0, 0`** while the entry itself stays (the
    table must cover all four `BIN7_ROOT_CAUSE` pairs). Everything else moves
    with it: `PROPS_ECHO_R4133` 81 → **82**, `ECHO_PARSE_ROWS` 7 → **8**,
    `R4133_ONLY_ROWS`/`R4133_ONLY_CELLS` 57 / 34 969 → **58 / 34 971**,
    `CLAIMED_ECHO` 169 → **170** (`CLAIMED_TOTAL` 2 974 → **2 975**, derived) and
    `CLAIMED_SPELLINGS_LIVE` 2 981 → **2 982** — **re-measured**, not bumped: the
    full claims census (`DSS_PROPS_CENSUS=claims`, 439 cases × 2 channels, 65 s)
    reports `echo-row` at **488 019 cells / 468 046 in scope, 170 spellings, 82
    pairs** and `UNCLAIMED` down to **1 722 / 887 / 58 pairs**, and the r4133
    channel's claimed spellings sum to exactly 2 982. `LEDGER_ENTRY_PINS` stays
    at **6 rows** — an ECHO sub-step owns none, which is what the guard's
    per-tag branch enforces.
  - **The guard needed two deliberate extensions, both hardening.** A `0, 0`
    entry breaks two assumptions the routing test made when every settled outcome
    was a LEDGER. (1) Its middle term compared `RP3_ROUTING.len()` against
    `Ledger::owner`'s *pair* count; the table keeps four entries while the ledger
    now knows three, so the term is the entries that still declare rows. (2) The
    live re-measurement (`seen`) no longer contains the pair at all — which on
    its own would make "0, 0" untestable — so a zero-row entry now carries a
    **positive** obligation: its rows must still exist in the corpus, and
    **every** one must be claimed by `Link::Echo` specifically, so a pair that
    vanished for any other reason reds here. A zero-row entry must also be tagged
    `ECHO`; an `OPEN` sub-step may never declare zero rows.
  - **Test count 8 394 → 8 398** (4 197 → **4 199** per lane, **+2**):
    `props_r4133_pins` 37 → **38** (the pin), `props_r4133_replay` 124 → **125**
    (the census derivation), no test deleted, `#[ignore]`d or loosened. Every new
    assertion was mutation-probed and red as intended: the pin's expected `4`
    flipped to `3`; the deck-fact table's generator count perturbed 1 → 2 and the
    held-out sibling's 53 → 52; the verdict's derived `2 cells, all 2 in scope`
    mutated to `3`; the echo row renamed away (the row-set lock, the witness and
    pin guards, the claim accounting and the routing guard's ECHO obligation all
    red at once); and the `RP3_ROUTING` counts restored to `1, 1` (the bucket
    lock red, `(7, 4, 7)` against `(6, 3, 6)`). No upstream report was filed and none was owed:
    the divergence is r4133 rendering its own parse store, i.e. the same class as
    the 81 rows RP2.3 landed without reports.
  - **Corrections this sub-step owes elsewhere.** The plan's §RP3.3 text, the
    `RP3_ROUTING` `OPEN` verdict and this record's own inherited framing all said
    `Solution.pas:1760` "takes it back to 3". It is dead code; the live reversion
    is `:2229`. The plan now carries the as-executed note that says so, and the
    settled verdict states it in the tree.
  - **Audit settlement (2026-08-24, one commit over `fb0e9e7f`).** Seven minor
    findings from the two audit agents, all settled — none waved off, and none
    touched the classification: both auditors re-derived the probe and ECHO
    stands. Still **zero product-crate bytes**, zero `ledger.json` / golden /
    manifest / `population.lock` / frozen-extract bytes, no mask moved.
    - **Two stale hand-offs to RP4.1, corrected in both copies.** The plan's
      §RP4.1 precondition 2 and the tripwire
      `the_staged_r4133_property_entries_have_not_landed_yet` both still listed
      RP3.3 among the sub-steps whose staged ledger entries RP4.1 must land and
      retire. An ECHO outcome stages nothing and retires itself in its own
      commit, so RP4.1's real inheritance is RP1.4's, RP3.2's four and whatever
      RP3.4+ stages — exactly the discipline RP3.3 applied to the `:1760`
      citation and had not applied to its own outcome.
    - **`ECHO_ROWS_ON_R4133_ONLY_CASES`' `(2, 2)` is now derived, not merely
      described.** Its doc said the entry was derived by the census test; that
      test never read the entry, the table's `cases` column had **no** value lock
      anywhere, and the audit's mutation `(2, 2)` → `(2, 5)` shipped green.
      Landed: the accessor `props_norm::r4133_only_exposure`, read by
      `the_rp33_census_decomposition_is_read_off_the_corpus` against **both**
      derived columns; the table-wide sum lock `R4133_ONLY_CASES = 833` (column 3
      had `R4133_ONLY_CELLS`, column 4 had nothing); and the structural invariant
      `cases <= cells` per row — an exposed case contributes at least one cell.
      The audit's mutation now reds in two places at once.
    - **The NCIM completeness sweep's three escape hatches, closed rather than
      argued shut.** (1) *Abbreviated option names*: `Set` resolves through
      `TCommandList.GetCommand` → `THashList.FindAbbrev`, a linear prefix match
      (`Shared/Command.pas:53`/`:65` arm `AbbrevAllowed`,
      `Shared/HashList.pas:335-357`), so `set algo=ncim` selects NCIM in the
      engine and was invisible to a reader that knew only the full spelling —
      and invisible to its hard error too, which keyed on the literal substring.
      The reader now reads every non-empty prefix of `algorithm`, longest first,
      deliberately wider than the engine. (2) *Abbreviated values*: the reader
      tested `value == "ncim"` while `InterpretSolveAlg`
      (`Common/Utilities.pas:575-591`) compares `copy(lowercase(s), 1, 2)`, so
      `Set algorithm=nc` parsed and was then dropped — the new `selects_ncim`
      ports r4133's own two-character rule. (3) *The `.dss`-only universe*:
      `collect_dss` filters on the extension while the corpus really does
      `Redirect` `.txt` scripts, so a `Set algorithm=NCIM` inside one would run
      unswept; `redirected_non_dss_scripts` now walks the transitive
      `Redirect`/`Compile` closure by basename (11 files at HEAD — `WireData.txt`,
      the two `AllocationFactors*.Txt`, the LVTestCase's seven parts,
      `HW_Inverters.txt` — none of which names the option). All three are dormant
      on today's corpus, which is why the settlement also lands the self-test
      `the_ncim_sweep_reads_every_spelling_the_engine_accepts`: a dormant reader
      proves nothing about the completeness claim resting on it. Non-vacuity:
      reverting `selects_ncim` to the equality reds it, and the closure's live
      count is asserted `>= 5`.
    - **The one surface the sub-step could not close now has an owner: §RP3.11.**
      "No behavioural divergence" was true of every compared channel and the
      audit bounded it: r4133's `Save`/`Dump` print the same parse store, so its
      round trip re-creates the model-3 generator, while the port's serializer
      renders the LIVE field (`report/save/save.rs::save_write_token` — the
      2026-08-24 reading cited `:34-53`, which RP3.11's guards moved — goes
      through `ClassProps::get_value` where Pascal `SaveWrite` reads
      `PropertyValue[iProp]`, `General/DSSObject.pas:145-165`). Measured here on
      `ncim_pv_pq.dss` after the converged solve: the port writes
      `New "Generator.g1" PF=0.88 Bus1=genbus Phases=3 kV=12.47 kW=800 Model=4
      Maxkvar=1500 Minkvar=-1500 Vpu=1.01` against r4133's `… kW=800 model=3 …`
      — a re-compiled deck is a PQ generator instead of a Q-limited PV one, and
      the port's line also carries a `PF=0.88` the deck never typed (a
      `PrpSequence` difference, a second class of divergence). No oracle channel
      compares `Save` on these r4133-gating decks, so this is neither a
      regression of RP3.3's commit nor part of the echo classification: the new
      plan §RP3.11 owns both questions, runs **after RP4.1** (the echo table is
      the list of pairs where the two serializers can disagree) and blocks
      §RP5.2, not the unmask. The `RP3_ROUTING` verdict, the plan's as-executed
      note and this record all carry the bound now. *(Settled 2026-09-03 by
      §RP3.11 — `KEEP_LIVE_PINNED` on both surfaces — which also corrected the
      premise stated here: `PropertyValue[iProp]` resolves to the **virtual**
      `GetPropertyValue` (`General/DSSObject.pas:45`, `:117-120`), so r4133
      prints **live** values through 49 classes' hand-picked index sets and
      stores `model` only because `TGeneratorObj.GetPropertyValue` has no arm
      for it. Record in §1 above.)*
    - **A pre-existing flake in a gated binary, removed.** The audit measured
      `harness::props_norm::tests::the_value_chain_resolves_in_order_and_agrees_with_the_seam`
      failing ~1–3 % of runs with "the chain query moved a counter": it
      snapshotted the **process-global** seam counters around its body while
      sibling tests in the same binary deliberately drive those very seams, so
      "the five-command gate was green" could be luck. The counters keep their
      process-global role (the gate's live accounting reads them); the offline
      question — *did MY query reach a counting seam* — is now asked of a new
      per-thread counter `props_norm::seam_touches_here`, which libtest's
      one-thread-per-test model makes exact. Four assertions moved to it (the two
      offline-query tests, the value chain, and the echo seam's exact deltas);
      the "the SHIPPED statics moved" half stays on the globals as `>=`, the
      shape the floor seam test already used. Strictly stronger than what it
      replaces — no sibling can mask a real touch — and the floor test gained
      exact deltas it could not state before. `counter_totals` lost its last
      reader and was deleted rather than kept warm. Non-vacuity: making
      `record_touch` a no-op reds both seam-counting tests; 40 consecutive runs
      of the binary afterwards, 0 failures.
    - **Test count 4 199 → 4 200 per lane (8 398 → 8 400)**: the sweep
      self-test, `props_r4133_replay` 125 → **126**. No test deleted (the removed
      `counter_totals` is a helper, not a `#[test]`), none `#[ignore]`d, none
      loosened; the two `==`→`>=` moves on the global counters are paired with
      strictly exact per-thread assertions.

- **RP3.4** (2026-08-24) — `gictransformer.r2`: **the r4133 twin of an
  already-fixed, already-pinned divergence.** **Zero product-crate bytes**
  (`GOLDEN_REBASE_PLAN.md` G2.5 fixed the engine in both lanes on 2026-08-06),
  **zero `ledger.json` bytes**, zero golden / manifest / `population.lock` /
  frozen-extract bytes, no r4133 mask moved — two test files plus docs. Outcome
  **LEDGER**, and with it **all four bin-7 root-cause pairs are settled**.
  - **The r4133 source shares the capi slip line for line.** `RecalcElementData`
    fills the two conductances from the percentages when `FpctRSpecified`, and
    the winding-2 line reads the **H** winding's percentage:
    `Version8/Source/PDElements/GICTransformer.pas:495`
    `G2 := 100.0 / (FZBase2 * FPctR1);`, the byte-twin of pinned dss_capi 0.14.5
    `src/PDElements/GICTransformer.pas:441`. The reverse branch is right
    (`:497-498` / capi `:445-446`), which is what makes it a slip rather than a
    convention — the two maps are inverses only when the forward one reads
    `FPctR2` — and the creation defaults are independent (`%R1 = %R2 = 0.2`,
    `:458-459`). Which branch runs is set by the `Edit` `CASE`'s "specials":
    arms 13/14 (`%R1`/`%R2`, `:308-309`) set `FpctRSpecified := TRUE` (`:349`),
    arms 7/8 (`R1`/`R2`) set it FALSE (`:343`). On `type=Auto` the `busX` side
    effect promotes the X winding onto terminal 2 (`:323`, and `:340`), which is
    why these decks' whole solved model moves too — already excluded by the G2.5
    entries.
  - **The render is NOT an echo, and that is what fixed the outcome shape.** The
    plan's own text called it "the same un-honoured `%R2` **echo**", which would
    have licensed a `PROPS_ECHO_R4133` row. It is not one: property 8 is `R2`
    (`PropertyName^[8]`, `:130`) and `TGICTransformerObj.GetPropertyValue` arm 8
    is `Format('%.8g', [1.0/G2])` (`:723`; `DumpProperties` prints the same at
    `:663`) — a **live computation** off the mis-derived conductance. Property
    14 is `%R2` (`:136`) and renders the stored `FpctR2` (`:729`), which agrees
    with the port digit for digit — which is exactly why the census carries a
    cell on `r2` and none on `%r2`. capi 0.14.5 reaches the same number by a
    different road (`PropertyOffset[ord(TProp.R2)] := ptruint(@obj.G2)` +
    `TPropertyFlag.InverseValue`, `:233-234`), as does the port
    (`elements/pd/gic_transformer/accessors.rs:60` `R2 => self.g2` behind
    `PropFlags::INVERSE_VALUE`). **So no echo row was added; the exclusion is
    two drafted ledger entries.**
  - **Census, derived per element and closing over every cell.** `bins.tsv:230`
    `gictransformer.r2 numeric 7 - 2 2 2.50e-01 2.50e-01`; the single frozen row
    is `examples_full.txt:1439` `gictransformer.r2 | '0.09522' | '0.12696' | 2`.
    The whole corpus declares **22** GICTransformers in **4** files (and no
    non-`.dss` script declares one — the class is merely *mentioned* by **three**
    syntax-highlight files under `Version8/Distrib/Examples/SyntaxFiles/`
    (`opendss.stx:144`, `OpenDSS_syntax_NotepadPlusPlus.xml:35`,
    `…_V2.xml:28`), by the manifests and by the census extracts; the count was
    "two" as first written and is corrected here, the four-file conclusion being
    unaffected — the derivation sweeps the whole file universe, not this list):

    | case | rigor | declared | `%R`-spec'd | cells | in scope |
    |---|---|---|---|---|---|
    | `asymmetric:gic/gictransformer_gic.dss` | `micro steps=1 engines=both` | 3 (`tg1` GSU `R1=0.12`; `tg2` YY `R1=0.2 R2=0.1`; **`tg3` Auto `%R1=0.2 %R2=0.15`**) | 1 | **1** | **1** |
    | `asymmetric:gic/gic_midi.dss` | `midi steps=1 engines=both` | 3 (`tg1` GSU `R1=0.12`; `tg3` YY `R1=0.2 R2=0.1`; **`tg5` Auto `%R1=0.2 %R2=0.15`**) | 1 | **1** | **1** |
    | `solvable_now:…/GICExample/GIC_Example.dss` | `feeder steps=1 engines=both` | 15 (`T1…T15`, all ohms `R1=`/`R2=`) | 0 | 0 | 0 |
    | `modes:makeposseq/makeposseq_shunt.dss` | `micro steps=1 engines=capi_v0145` | 1 (`gt` GSU `R1=0.1`) | 0 | 0 | 0 |
    | **total** | | **22** | **2** | **2** | **2** |

    `ZBase2 = 138²/300 = 63.48 Ω`, so ours is `63.48*0.15/100 = 0.09522` against
    both oracles' `63.48*0.20/100 = 0.12696`, `rel = 0.25` — the frozen
    `2.50e-01`. `R1 = 396.75*0.2/100 = 0.7935` on **both** engines, which is why
    the census has no `gictransformer.r1` row at all. Two independent
    cross-checks of the same population read, both landed as assertions: the
    class-wide pairs `gictransformer.enabled` (`bins.tsv:45`) and
    `gictransformer.pctperm` (`:229`) each record **22 cells / 21 in scope** =
    `Σ declared × steps` and its r4133-gating subtotal, which owes nothing to the
    `%R` arithmetic; and the **positive measurement** (this sub-step's
    `civanlar.dss`) — the **20** ohms-specified GICTransformers, **19** of them
    on r4133-gating cases and four of them `type=Auto` like `tg3`/`tg5`, produce
    **zero** cells, because the ohms spec sets `FpctRSpecified := FALSE` and
    leaves the untouched reverse branch to answer. A cell needs `%R1 ≠ %R2`, and
    no corpus deck has the `%R1=`-alone blast-radius shape the cause blob warns
    about (asserted, not assumed).
  - **The other four G2.5 property surfaces stay capi-only — with one correction
    to the plan's stated reason.**

    | ledger entry | `property` scopes | case | `engines=` | why r4133 never sees it |
    |---|---|---|---|---|
    | `makeposseq-cuf-applied-capi-props` | 3 (`capacitor.cap_cmat.{cuf,normamps,emergamps}`) | `modes:makeposseq/makeposseq_shunt.dss` | **`capi_v0145`** | the channel is not gated at all |
    | `capi-generator-makeposseq-rating` | 4 (`generator.g_kva.{kva,mva}`, `generator.g_mva.{kva,mva}`) | `modes:makeposseq/makeposseq_pc.dss` | **`capi_v0145`** | same |
    | `capi-linespacing-normamps` | 2 (`line.lsp.{normamps,emergamps}`, + 2 `probe` scopes) | `asymmetric:line/line_spacing_asym.dss` | **`both`** (!) | the ledger holds `r4133-linespacing-asym-303`, `kind: "skip"` (EPRI #303 AV while compiling the `tscables=`/`wires=` spacing), so `ledger.rs::channel_is_skipped` makes `scheduler.rs:355-356` `continue` past the channel — **and** r4133's own `General/LineGeometry.pas:1235-1239` implements the min-over-phase rule the port follows, so there would be nothing to twin |

    2 (gic) + 3 + 4 + 2 = the plan's **11 property scopes over 5 entries**. The
    census confirms it from the other side: `line.normamps`/`line.emergamps` and
    `generator.kva`/`generator.mva` are **absent from `bins.tsv` entirely**, and
    `capacitor.normamps`/`.emergamps` record 4 cells / **0 in scope**. The
    plan's shorthand ("their decks do not gate r4133") is true of the two
    `makeposseq` cases and **false as stated** for `line_spacing_asym.dss`; the
    mechanism above is what the record carries.
  - **The two drafted entries, VERBATIM — they land at RP4.1 per §1.1(e), NOT
    here** (*landed 2026-09-03 in RP4.1's commit `59e521e5`, verbatim, on the
    pre-existing `gic-pct-r2-ignored` cause; both hit — 2 in-scope cells*)**.** Both reuse the existing `gic-pct-r2-ignored` cause unchanged (it
    already names the r4133 lines), and the ids mirror the capi originals'
    `-props` suffix because plain `…-r4133` is taken by the G2.5 solved-model
    exclusions. Schema checked against `corpus_gate/ledger.rs:1420-1560`.

    ```json
    {
      "id": "gic-pct-r2-honoured-gictransformer-r4133-props",
      "case": "asymmetric:gic/gictransformer_gic.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^gictransformer\\.tg3\\.r2$",
          "rust": "0.09522",
          "oracle": "0.12696"
        }
      ],
      "cause_ref": "gic-pct-r2-ignored",
      "source": "R4133_PROPS_PLAN RP3.4 (2026-08-24), drafted in the sub-step and landed at RP4.1 per 1.1(e) - earlier it would fail assert_all_hit as NEVER APPLIED, the r4133 property compare being masked until the unmask. The r4133-channel twin of gic-pct-r2-honoured-gictransformer-capi-props: EPRI r4133 carries the identical forward arm, Version8/Source/PDElements/GICTransformer.pas:495 `G2 := 100.0 / (FZBase2 * FPctR1);`, the byte-twin of pinned dss_capi 0.14.5 src/PDElements/GICTransformer.pas:441, and renders property 8 (`R2`, :130) as Format('%.8g',[1.0/G2]) from GetPropertyValue arm 8 (:723; DumpProperties prints the same at :663) - a LIVE read of the mis-derived conductance, not a parse-store echo, which is why the exclusion here is a ledger entry and not a PROPS_ECHO_R4133 row. Property 14 (`%R2`, :136) renders FpctR2 (:729) and agrees with the port, so only the derived ohms diverge. 1 in-scope cell (steps=1, the deck's one %R-specified GICTransformer, tg3): ours 0.09522 = ZBase2*%R2/100 = 63.48*0.15/100 for the `%R1=0.2 %R2=0.15 kvll1=345 kvll2=138 mva=300` of gictransformer_gic.dss:18-19, vs r4133's 0.12696 = ZBase2*%R1/100, max_rel 2.50e-01 (tests/corpus/props_r4133/bins.tsv:230). The deck's other two GICTransformers use the ohms R1=/R2= spec and the untouched else arm (:497-498), so they produce no cell. Replacement pin: props_r4133_pins.rs::gictransformer_r2_honours_the_x_winding_percentage.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

    ```json
    {
      "id": "gic-pct-r2-honoured-midi-r4133-props",
      "case": "asymmetric:gic/gic_midi.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^gictransformer\\.tg5\\.r2$",
          "rust": "0.09522",
          "oracle": "0.12696"
        }
      ],
      "cause_ref": "gic-pct-r2-ignored",
      "source": "R4133_PROPS_PLAN RP3.4 (2026-08-24), drafted in the sub-step and landed at RP4.1 per 1.1(e). The r4133-channel twin of gic-pct-r2-honoured-midi-capi-props - same getter (Version8/Source/PDElements/GICTransformer.pas:723 over the :495 slip), same bases, on tg5 of the 6-substation 345 kV ring: gic_midi.dss:27-28 declares `%R1=0.2 %R2=0.15 kvll1=345 kvll2=138 mva=300 type=Auto`, so ZBase2 = 138^2/300 = 63.48 ohm gives ours 0.09522 against r4133's 0.12696. Entries are per case, so this one is its own; 1 in-scope cell (steps=1). The ring's other two GICTransformers (tg1 GSU R1=0.12, tg3 YY R1=0.2 R2=0.1) are ohms-specified and produce no cell - 20 of the corpus's 22 GICTransformers are, 19 of them on r4133-gating cases, and not one of them diverges, which is the measurement that the %R path is the whole divergence class. Replacement pin: props_r4133_pins.rs::gictransformer_r2_honours_the_x_winding_percentage_on_the_ring.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

  - **No new upstream report, and that is a finding, not a skip.**
    `investigations/to_opendss/07-gictransformer-g2-uses-pctr1.md` (local,
    gitignored) is already written **against r4133** — its "The line at fault"
    section reads "As of SVN trunk r4133, `Version8/Source/PDElements/
    GICTransformer.pas`, lines 486-501" and its reproduction is exactly this
    surface (`? GICTransformer.t1.R2` → `0.12696`), with the `else`-arm inverse
    argument and the `type=Auto` `busX` blast radius. The Russian deep-dive is
    `investigations/issue-07-gictransformer-g2-pct-r1.md`. **The next free report
    number stays 45** (taken by RP3.5 on 2026-08-28; 46 is next).
  - **Pins (2, both lanes, no oracle):**
    `gictransformer_r2_honours_the_x_winding_percentage` (`tg3`) and
    `gictransformer_r2_honours_the_x_winding_percentage_on_the_ring` (`tg5`),
    one per drafted entry. Each asserts the actual `0.09522`, not "not
    0.12696", and carries the **discriminating** half the module doc demands:
    an `edit %R2=0.3` moves `R2` to `0.19044` (the setter drives this getter),
    and an `edit %R1=0.4` then moves `R1` to `1.587` while leaving `R2`
    **unmoved** — upstream, whose `G2` is a function of `FPctR1`, would answer
    `0.25392` there. That reading separates the two engines' *mechanisms*, not
    two numbers. Each pin also reads the deck's ohms sibling (`0.1`, the reverse
    branch), the GSU's `Create`-derived `0.38088`, and `%R2` itself (`0.15`, the
    reading that agrees with r4133 and proves the divergence is confined to the
    derived ohms).
  - **Accounting.** `RP3_ROUTING`'s `gictransformer.r2` row flips
    `OPEN` → **`LEDGER`** with the full verdict (r4133 unit citation, `RP3.4`,
    `RP4.1` + `§1.1(e)`, both pins as whole identifiers, the derived census, the
    positive measurement, the class-wide cross-check, the report pointer); its
    counted columns stay **`1, 1`** and `DECLARED_RP3` stays **`(6, 3, 6)`** —
    a LEDGER outcome stages, so the row remains on RP4.1's hand-edit list.
    `LEDGER_ENTRY_PINS` **6 → 8** rows (literal lock moved with it);
    the settled set in
    `the_bin7_root_cause_pairs_are_routed_to_their_sub_steps` becomes all four
    pairs in `RP3_ROUTING` order — the guard needed no relaxation for the
    all-settled state, and its message now says so. New derivation
    `the_rp34_census_decomposition_is_read_off_the_corpus` reads the two cells
    off the corpus (whole **file** universe, not just `.dss`), the
    `population.lock` rigor and the frozen extracts, plus the new per-element
    reader `gictransformer_elements` (`element_tokens` cannot serve: it collapses
    a class to one scope per deck and both gic decks type two different `R1=`)
    and its self-test
    `the_gictransformer_reader_separates_the_percentage_and_ohms_specs` — the
    `%R1=`/`R1=` separator rule is what selects the branch, so a reader that got
    it wrong would answer "zero cells, no entry", green and wrong. Single-claim
    holds: `props_norm` carries rows on `gictransformer.enabled` and
    `.pctperm` but **none** on `.r2`, and the standing
    `!has_echo_row("gictransformer", "r2")` assertion is kept with its comment
    re-worded to the finding. The staged-entries tripwire
    `the_staged_r4133_property_entries_have_not_landed_yet` now names RP3.4's two
    ids in its doc; `tests/corpus/ledger.json` is byte-untouched.
  - **Non-vacuity, proven by mutation (15, all reverted).** Pins: each asserted
    value flipped one digit (`0.09522`→`0.09523` on both decks, the GSU control
    `0.38088`, the `%R2` agreement reading `0.15`) and the discriminator
    rewritten `%R1=0.4`→`%R2=0.4` — five reds. Accounting: the verdict's derived
    split (`2`→`3` `%R` decks), the verdict orphaning its derivation test, the
    census token (`%R2` `0.15`→`0.2`), an ohms sibling's token
    (`R1=0.12`→`0.13`), a `%R` element re-classified as ohms-specified, a dropped
    element row (`GIC_Example` `T15`),
    the class-wide lock (`21`→`20`), the derivation reading `%R1` where it must
    read `%R2`, the outcome tag (`LEDGER`→`ECHO`) and a deleted
    `LEDGER_ENTRY_PINS` citation — ten more reds, three of them in two or three
    tests at once.
  - **Test count 4 200 → 4 204 per lane (8 400 → 8 408)**: `props_r4133_pins`
    38 → **40** (the two witnesses), `props_r4133_replay` 126 → **128** (the
    derivation plus the reader self-test); `props_r4133_evidence_lock` unchanged
    at 11. Both gate lanes green at 4 204 with 0 failed / 0 ignored / 0 filtered.
    No test deleted, `#[ignore]`d or loosened; no tolerance
    exists here to move (the pins compare rendered strings).
  - **Audit settlement (2026-08-24, one commit over `cab2e667`).** Seven minor
    findings from the two audit agents, all settled — none waved off. The
    classification is untouched: both auditors re-derived the mechanism and
    `LEDGER` stands, the census still decomposes to 2 cells / 2 in scope, and the
    two drafted twins are unchanged. Still **zero product-crate bytes**, zero
    `ledger.json` / golden / manifest / `population.lock` / frozen-extract bytes,
    no mask moved, and the test count stays 4 204 per lane (the settlement adds
    assertions, not tests).
    - **The ring pin's discriminator was half a pin, and is now whole.**
      `gictransformer_r2_honours_the_x_winding_percentage_on_the_ring` ended at
      "`%R1=0.4` leaves `R2` unmoved" — a reading a correct engine and a *no-op
      edit* satisfy alike, since the test never read `R1` back. The audit
      measured it: rewriting the edit to `%R9=0.4`, a property that does not
      exist, left the whole binary green (40 passed), while the same mutation on
      the micro-deck twin — which always carried the control — went red. The
      missing `tg5.R1 == "1.587"` reading is added, and the `%R9` mutation now
      reds by name. (The STATUS bullet above always described *both* readings for
      *both* pins; as of this settlement that description is true.)
    - **The census's `%R1 == %R2` branch had no counter, so it could only red as
      something else.** An element whose two percentages coincide renders the same
      number on both engines and carries no cell; the derivation `continue`d past
      it without counting it, so it landed in neither `ohms` nor the diverging
      set and the *next* cross-check failed with "nothing may fall between the
      two" — the wrong cause, and an invitation to relax that check instead of
      extending the classification. Measured both ways with one coordinated
      mutation (deck token `%R2=0.15`→`0.2` on `gic_midi.dss` `tg5`, its
      `RP34_GIC_ELEMENTS` row, and the frozen `bins.tsv`/`examples_full.txt`
      counts 2→1, all reverted): at `cab2e667` it reds `21 != 22`, "nothing may
      fall between the two"; now it reds naming the coincidence and listing the
      element. The three classes (`ohms` / `coincident` / `diverging`) are each
      counted and each adjudicated — `coincident` empty, the classification
      exhaustive, and **every diverging element on an r4133-gating case** (the
      second hole the audit named: a `%R` element diverging on a capi-only case
      would also have "fallen between").
    - **"In scope" no longer means `engines=` alone — the shorthand this very
      sub-step disproved.** RP3.4 found that `asymmetric:line/line_spacing_asym.dss`
      is `engines: "both"` and yet never compared on r4133, and then encoded the
      shorthand in its own guard. New reader `r4133_skipped_cases` applies
      `corpus_gate/ledger.rs::channel_is_skipped`'s own condition (`channel ==
      "r4133"`, `kind == "skip"`) to `tests/corpus/ledger.json` (read-only), and
      all **four** RP3 census derivations now ask both halves of the question
      (RP3.3's r4133-only cases assert the negative directly). No number moved —
      none of the RP3.1/RP3.2/RP3.3/RP3.4 cases is in today's four-entry skip set
      — and the second half is kept non-vacuous by a witness assertion on
      `r4133-linespacing-asym-303`: mutating the reader's channel filter to
      `capi_v0145` reds it.
    - **Three stale citations, corrected in every copy.** The class is mentioned
      by **three** syntax-highlight files, not two (the four-file declaration
      count is unaffected and was re-derived independently); the `continue` past
      a skipped channel is `scheduler.rs:355-356`, not `:355` (both the STATUS
      table and the plan's as-executed note); the routing guard's own doc still
      said "the settled set is pinned literally (RP3.1 and RP3.2 today)" while its
      literal carries all four pairs, and the staged-entries tripwire enumerated
      RP1.4's, RP3.2's four and RP3.4's two while omitting RP3.1's two — it now
      lists all **eight** staged ids by name, matching plan §RP4.1 precondition 2.
    - **Flagged for the user, not ruled in-lane (the one process finding).** The
      sub-step's kill criterion included "a `makeposseq`/`linespacing` deck DOES
      gate r4133", and `line_spacing_asym.dss` *is* `engines: "both"`; part A
      classified that as not-the-kill-criterion itself rather than escalating.
      The substance was re-verified twice (the `kind: "skip"` entry, r4133's own
      agreeing `LineGeometry.pas:1235-1239`, and the census carrying no
      `line.normamps`/`line.emergamps` row at all) and by the new skip-aware
      predicate, so nothing rests on the ruling; it is recorded here and reported
      to the user as a deviation, since a kill-criterion reading is the user's to
      make.

- **RP3.5** (2026-08-28) — `line.units`: **a port fix in both lanes, and the
  first RP3 sub-step whose fix moves the port's own render.** Outcome `FIX` for
  three independent defects in one routine (`TLineObj.MergeWith`), plus one
  upstream report for a fourth that is r4133's alone. It is also the first RP3
  sub-step to land a **live** ledger entry: the fix reds the `capi_v0145`
  `all_properties` compare on `modes:reduce/midi_reduce.dss`, whose channel is
  not masked, so the entry, its cause and the `population.lock.json` rewrite ship
  in this commit rather than staging to RP4.1.
  - **The probe ran first, on three engines** (r4133 `OpenDSSDirect.dll`
    `Version 11.0.0.1 (64-bit build) - Charlottesville`; pinned dss-python 0.15.7
    / dss_capi 0.14.5; the port, built out-of-tree against `dss-core` by path):
    the two vendored `modes:reduce` decks under `CorpusGuard`, plus six
    purpose-built micro-decks for the arms and the CIM surface no corpus deck
    reaches. No repo byte was written by the probe. None of the plan's four
    decision-table kill criteria fired.
  - **(A) The matrix-series branch lost the length units.** r4133 saves them
    (`Version8/Source/PDElements/Line.pas:1627`) and re-applies them with a
    **separate** `Length=%-g  Units=%s` Edit at `:1794-1796` that runs *after*
    the `Rmatrix/Xmatrix` (`:1778-1779`) and `Cmatrix` (`:1791-1792`) Edits,
    because the `12..14` side effect calls `ResetLengthUnits` (`:691-693`); its
    own `MakePosSequence` uses the same construction at `:1595-1596` under the
    comment "Repeat the Length Units to compensate for unexpected reset". dss_capi
    0.14.5 inverted the two — `src/PDElements/Line.pas:1806-1807` writes the
    fields, `:1815-1817` then runs `PropertySideEffects(rmatrix/xmatrix/cmatrix)`
    — and the port had copied that. Measured: r4133 renders `kft` on all three
    merged lines of `midi_reduce.dss` where the port and capi render `none`, and
    the getter is deck-dependent (`mi`/`cm` on two micro-decks whose surviving
    line carries those units, `none` on a switch), i.e. live state and not an
    echo. Fixed in both lanes (`exec/reduce.rs`: `red_set_units` after the side
    effects). **No solved-state quantity moves** — `ConvertLineUnits` returns 1.0
    whenever either side is `UNITS_NONE` (`Shared/LineUnits.pas:110-115`), so
    `FUnitsConvert` stays 1.0 and YPrim, Y, node voltages, currents, powers,
    losses, the node count (88) and the iteration count (5) are identical to
    r4133 before and after. Two *length-derived* quantities do move, both as
    corrections and neither compared anywhere today — `miles_this_line` and the
    meter zone's `line_length_km`; they are pinned by the audit settlement
    below.
  - **(B) `reset_length_units` also cleared `user_length_units`, which neither
    oracle does.** r4133 `:2330` and dss_capi 0.14.5 `:2084` carry the identical
    statement pair under the identical comment, "but do not erase
    FUserLengthUnits, in case of CIM export" — so this was a port-authored
    divergence from **both** oracles and there was no authority question to
    weigh. The plan called it "not observable in the census", which is true and
    not the same as unobservable: an exhaustive grep of both trees finds one
    consumer, `ExportCIMXML` (r4133 `:3707`, `:3735`, `:3877`), and it
    discriminates — on a deck typing `units=kft` before its matrices,
    `Export CIM100` writes `<cim:Conductor.length>609.6</…>` (= `2 × 304.8`) on
    r4133 **and** on capi 0.14.5 against `2` on the port. The line is deleted in
    both lanes. **No CIM golden byte moves**: in every golden CIM deck
    (`cim_lines`, `cim_load`, `cim_shunt`, `cim_xfmr`, `cim_der`,
    `IEEE13Nodeckt`, `IEEE123Master`) `units=` is the last impedance-relevant
    token of every `Line` declaration or absent altogether, so
    `reset_length_units` never runs after a `units=` write — swept, not assumed.
  - **(C) The sym-components branch's two switch arms, a gap the plan called
    already-correct.** The port's `Length=`/`Units=` re-apply sat inside
    `if let Some(v) = rxc`, while both oracles run it unconditionally (r4133
    `:1724-1726`; capi `:1764-1768`, outside `if UseRXC`). The two arms that
    produce no impedance values therefore kept a pre-merge `Len`: measured
    `0.001` (self-is-switch) and `1.1` (partner-is-switch) against `1` on both
    oracles. The partner arm carried a **second**, unrelated defect: the port
    emitted the text `Switch=1`, a transliteration of capi's *typed*
    `SetInteger(ord(TProp.Switch), 1, [])` (`:1736`), where r4133 emits the text
    `' switch=yes'` (`:1709`). `InterpretYesNo` rejects `1` on both engines
    (probed: `edit line.a Switch=1` leaves `switch='False'`, `r1='0.301'` on the
    r4133 DLL), so the arm was a silent no-op and the merged branch kept the
    partner's real impedance where both oracles give it dummy z (`r1 = 1`) —
    live state, not a render. Both halves fixed in both lanes. No vendored corpus
    deck reaches either arm (0 census cells), so no gated case moves; found and
    fixed inside the sub-step per CLAUDE.md's "port gaps immediately". Making the
    partner arm reachable exposed a **third** defect on it, found by the audit
    settlement below and fixed there: the sym branch's deferred
    `RecalcElementData`.
  - **(D) A new r4133 defect in the same routine — reported, never reproduced.**
    `Line.pas:1715` reads `S := ' R0=' + …` where every neighbouring statement
    appends (`S := S + …`), so the parallel symmetrical-components edit string
    loses its `R1=`/`X1=` half. On `modes:reduce/reduce_mergeparallel.dss` r4133
    renders the *un-merged* `r1 = 0.301` / `x1 = 0.667` while `r0`, `x0`, `c1`,
    `c0`, `length` and `units` match the port and capi exactly — the precise
    asymmetry an assignment-instead-of-append predicts — and the four measured
    relative gaps (0.5368421052631595, 0.5368421052631575, 1.5030841450107981,
    1.2866220615751998) reproduce the frozen census cells to the last digit, so
    the census row is not stale. capi 0.14.5 does not carry it (`:1740-1761`
    fills `RXC[1..6]` and sets all six) and neither does the port; the only
    exposing deck is `engines: "capi_v0145"`, so 0 in-scope cells and no
    exclusion is owed. Report:
    `investigations/to_opendss/45-line-mergewith-parallel-drops-r1-x1.md`
    (local). Next free report number is now **46**.
  - **The plan text was wrong about the population and silent about the cost;
    both corrected in place (§RP3.5).** `reduce_mergeparallel` contributes
    **zero** `line.units` cells — all 3 are on `midi_reduce.dss`
    (`Line.l2a~l2b`, `Line.l3a~l3b`, `Line.bb14_15~l9a`), and `Line.b1||b2.units`
    renders `km` on all three engines because that deck's lines are 3-phase
    symmetrical-components and take the sym branch. And the plan's two stated
    outcomes ("a port fix with a pin" / "a cited exclusion") did not anticipate
    the capi channel: `MODES.compare_all_properties` is `true` and
    `force_properties` ORs it in for every `gates_capi()` family case, and
    `(Line, Units)` is in no `SKIP_PROPS` / `PROPS_015X` / `LANE_SKIP_PROPS` row,
    so the fix reds 3 live cells.
  - **Ledger + lock (this commit, not staged).** New `capi_v0145` `divergence`
    entry `reduce-merge-units-restored-midi-capi-props` on
    `modes:reduce/midi_reduce.dss` — three exact-pair `property` scopes
    (`line.l2a~l2b.units`, `line.l3a~l3b.units`, `line.bb14_15~l9a.units`, each
    `rust: "kft"` / `oracle: "none"`) — plus the new cause
    `line-merge-length-units-reset`. Measured 3 hits on the case's one step.
    §1.1(e)'s staging rule is written for **r4133** `property` entries, which are
    staged only because that channel's props compare is masked until RP4.1; the
    capi compare is live, so staging this one would leave the gate red.
    `population.lock.json` moves exactly one line — `modes` `reduce/midi_reduce.dss`
    gains `ledger=capi_v0145:reduce-merge-units-restored-midi-capi-props@01a907f79c55bac9`.
  - **Pins (4 new `#[test]`s here, 3 more in the audit settlement below; all
    oracle-free and green in both lanes).**
    `exec::tests::reduce::merged_matrix_line_keeps_the_surviving_lines_length_units`
    (two micro-decks whose surviving lines carry *different* saved units, so the
    assertion cannot pass against a hardwired getter, plus an un-merged control
    and a post-merge `rmatrix=` reset);
    `exec::tests::reduce::parallel_merge_with_a_switch_restores_length_and_dummy_z`
    (three decks: partner-is-switch, self-is-switch, non-switch control);
    `elements::pd::line::tests::reset_length_units_keeps_the_users_units` (all
    three `reset_length_units` callers, asserting the pair — `length_units`
    cleared **and** `user_length_units` kept);
    `golden_cim::cim_conductor_length_uses_the_users_length_units` (the
    observable half of (B): `units=` first → 609.6, `units=` last → 914.4, no
    `units=` at all → 5, so the pin is a reading and not a constant factor).
    They are **not** in `props_r4133_pins.rs`: that file's guard
    (`props_r4133_replay::every_echo_row_pin_is_a_test_that_exists`) admits only
    `#[test]`s cited by a `PROPS_ECHO_R4133` row or by `LEDGER_ENTRY_PINS`, and
    RP3.5 owns neither (its outcome is `FIX`, and its ledger entry is a *live*
    capi one, not a drafted r4133 one whose window that table exists to cover).
    The house precedent for a landed capi property entry is an in-engine pin
    cited from the fix site — `capacitor::tests::make_pos_sequence_cmatrix_
    applies_the_positive_sequence_cuf` for `makeposseq-cuf-applied-capi`,
    `exec::tests::compat_quirks::gic_transformer_pct_r2_drives_winding_two` for
    `gic-pct-r2-honoured-*` — and that is the shape used here.
  - **Not moved, deliberately.** No `PROPS_ECHO_R4133` row (r4133's index-20
    getter is live at `:1404`; an echo row would be a false statement about the
    mechanism, the same reading RP3.1/RP3.2/RP3.4 made). No r4133 ledger entry
    (0 in-scope cells, and the r4133 props compare is masked until RP4.1, so
    `assert_all_hit` would fail it as NEVER APPLIED). No `props_roundtrip`
    scenario for a merged line — that gate replays dss_capi 0.14.5 values and
    would pin the capi bug as the expectation. `DECLARED_RP35 = (8, 6, 5)` and
    `DECLARED_RP3 = (6, 3, 6)` unchanged: they are read off the **frozen**
    extracts through the shipped chain predicates, which never run the engine, and
    `line.units` still declares a row, so `RP22_ROUTING`'s liveness guard stays
    green. `lane_diff.ps1` was **not owed** — the change carries no
    `#[cfg(feature = …)]` and moves no solved-state quantity on any gated case —
    but was run anyway rather than argued: **VERDICT PASS, `max |Δ| = 0` exactly
    on all eight gated kinds** (conv/cur/errs/iter/loss/pow/v/y, 3 220 247
    records over 522 cases, 0 iteration counts drifted), so the two lanes stay
    bit-identical and the default lane keeps the parity lane's oracle standing.
  - **A convention this sub-step establishes.** RP3.5 is the first RP3 sub-step
    whose fix changes the port's own render, so the frozen
    `tests/corpus/props_r4133/` extracts now record a `rust` value (`'none'`) the
    engine no longer produces. They are a **data** lock recording the 2026-08-08
    measurement (`props_r4133_evidence_lock.rs` re-measures nothing), so they are
    **not** edited; the ledger entry's `source` says so, and later sub-steps that
    fix rather than exclude inherit the same rule.
  - **Audit settlement (2026-08-29, one commit over `9daff660`).** Nine claims
    across the two audit agents — eight headline findings, one of them raised by
    both, plus the two halves of the tests audit's Major. Both auditors
    re-derived the mechanism independently on live oracles and confirmed the
    classification: outcome stays `FIX` in both lanes, the census still
    decomposes to 3 cells / 0 in scope, the ledger entry and its cause are
    unchanged, no upstream bug is reproduced and no `TODO(compat)` was added.
    Seven claims are real and settled here, one is **refuted** by live
    re-measurement, and one is real but outside `MergeWith`, recorded below with
    its owner. Settling the refuted one surfaced **a further defect in
    `MergeWith` itself** — the deferred `RecalcElementData` — fixed in the same
    commit under "port gaps immediately". No test was deleted, `#[ignore]`d or
    loosened, no tolerance exists here to move (every pin compares rendered
    strings), and no golden, frozen extract, `ledger.json` or
    `population.lock.json` byte moves.
    - **`MergeWith` re-pointed only the partner's controls, and named them with
      the pre-rename string — REAL, fixed.** r4133 calls
      `UpdateControlElements` **twice** (`Version8/Source/PDElements/Line.pas:
      1682-1683`), once for the surviving line's own old name and once for the
      partner's, both with `NewName`, and only then assigns `Name := NewName`
      (`:1684`). The port had the partner half alone — and in every reduce
      strategy that is the *vacuous* half: `DoReduceDefault` and
      `DoReduceShortLines` refuse to merge a line out when it `HasControl` or
      `IsMonitored` (`Meters/ReduceAlgs.pas:179-180`, `:347-348`), so a control
      can only ever sit on the **survivor**. Measured live on a `shortlines` deck
      merging `s1` into `s2`: the r4133 DLL renders `? CapControl.cc.element` as
      `line.s1~s2` where the port rendered `Line.s2` — and the port then failed
      to solve that deck (`singular at column 9`) while r4133 converged, because
      the stale reference kept the eliminated bus alive. The port now renames
      first and re-points both old references with the merged name; matching by
      the stable `ElemId` makes the Pascal's name-comparison order irrelevant,
      and renaming first is what lets the `element=` re-edit resolve. Pinned by
      `exec::tests::reduce::merge_repoints_the_controls_of_the_surviving_line`,
      proven non-vacuous by two mutations (drop the self half → the stale name
      and the singular Y; re-point before the rename → `Line.s2`). A control
      class whose property 1 is not spelled `element` (Relay/Recloser/Fuse:
      `MonitoredObj`) takes an unknown-parameter diagnostic and keeps its old
      name on **both** engines — probed on the same deck with a `Relay`, so that
      half is faithful and stays. This also closes a Phase-8 deferral that was
      still open by the "verify the successor of a forward-handoff" rule:
      `docs/phase-records/phase-8.md` recorded "`UpdateControlElements` has no
      runtime deck coverage (synthesized control-on-merged-line deck = a WP8.8
      sweep candidate)" and no later WP built it. It exists now, and it found the
      routine wrong.
    - **`RecalcElementData` was deferred out of the sym branch, and the deferral
      is not equivalent — REAL, fixed.** `MergeWith` ends its
      symmetrical-components branch with `RecalcElementData` (r4133 `:1730`,
      dss_capi 0.14.5 `src/PDElements/Line.pas:1771`); the port deferred it to
      `CalcYPrim` on the `SymComponentsChanged` flag. But that call *clears* the
      flag, and the flag is exactly what `CalcYPrim` tests before running its
      "the user never specified C1/C0" fix-up (`Line.pas:1031-1038`:
      `C1 := C1 / ConvertLineUnits(UNITS_KFT, LengthUnits)`). Every arm whose
      edit string carries `C1=`/`C0=` sets `FCapSpecified` and is immune — which
      is why the gap stayed invisible — but the two switch arms carry no
      impedance at all, and RP3.5's own item (C) had just made the
      partner-is-switch arm reachable. Measured on the sub-step's own d5 deck:
      after the (C) fix the port rendered `c1 = 3.60892388451444`,
      `c0 = 3.28083989501312` (the dummy `1.1 nF`/`1.0 nF` divided by
      `ConvertLineUnits(kft, km) = 0.3048`) where the r4133 DLL **and** capi
      0.14.5 both render `1.1` and `1` — a divergence from both oracles, so no
      authority question. `exec/reduce.rs` now recalcs in place; pinned by
      `exec::tests::reduce::parallel_merge_with_a_switch_recalcs_before_the_cap_fixup`
      (the self-is-switch arm restores `Units=none`, where the factor is 1.0, and
      is the discriminator), red on mutation.
    - **`reduce_mergeparallel` renders `linecode = ''` on r4133 — REFUTED.** The
      tests audit read the sub-step's own probe table as saying the r4133 DLL
      answers `''` on the partner-is-switch merge although its `switch=yes` arm
      (`:694-700`) does not clear `FLineCodeSpecified` — and drew from it that
      §RP3.6's premise was contradicted before RP3.6 starts. Re-measured on the
      same deck with the same driver: **r4133 renders `lc`**, exactly as the
      source says (`FLineCodeSpecified` is written at `:413` and cleared only by the
      impedance arms `:685`/`:691` and by the geometry/spacing/wire fetchers —
      never by arm 15). §RP3.6's premise stands and is now measured, not read.
      What the re-probe *did* find is the mirror image and belongs to RP3.6: the
      port renders `''` there, because its `SWITCH` side effect calls
      `kill_line_code_specified`. RP3.5's own (C) fix made that arm reachable, so
      the divergence is live on a path no corpus deck walks (0 cells) until
      RP3.6 lands — recorded here so RP3.6 inherits a measurement instead of a
      premise.
    - **A CIM divergence the probe measured and the record dropped — REAL, out of
      RP3.5's routine, recorded with an owner.** On a deck whose line names a
      LineCode and then overrides `r1=` *after* `units=`, r4133 back-fills the
      LineCode's units from `FUserLengthUnits` and writes
      `PerLengthSequenceImpedance.r = 0.301/304.8 = 0.00098753281`, because it
      matches by the `CondCode` **string**, which survives the flag being cleared
      (`Common/ExportCIMXML.pas:3877`, `if pLine.CondCode = pLnCd.LocalName`).
      capi 0.14.5 matches by the live object (`LineCodeObj <> NIL`) and writes
      `0.301`; the port follows capi (`cim/export.rs::find_line_units_for_linecode`,
      `line.line_code_ref.is_some()`). Item (B) is what *created the
      precondition* for the back-fill (before RP3.5 the port had no surviving
      `user_length_units` at all), which is why it was measured here — but the
      fix itself is not in `MergeWith`: it needs the port's
      `kill_line_code_specified` to stop clearing `line_code_name` while the
      render still answers `''`, i.e. the `FLineCodeSpecified`-vs-`CondCode`
      split that **§RP3.6 must build anyway**. Owner: **RP3.6**, noted in the
      plan's §RP3.6 text. No golden CIM deck exercises it (all of them name a
      LineCode or an impedance, never both).
    - **r4133 truncates the merged length through `%-g` — REAL as a latent
      r4133-channel divergence, recorded, not reproduced.** r4133 sets the merged
      `Len` only by rendering `TotalLen` into `Format(' Length=%-g  Units=%s')`
      and parsing it back (`:1725`, `:1795`), so its length carries 7 significant
      digits: measured `3.378788` against the port's and capi 0.14.5's
      `3.37878787878788` (rel 2.6e-7) on the sub-step's own micro-deck. The port
      keeps the exact `f64` sum, as capi does by assigning the field (`:1806`);
      reproducing the truncation would move the *capi* channel, which is the one
      that gates every reduce deck. Every vendored `reduce` deck sums to an
      exactly representable length (`4`, `1.4`, `1`, …), so no gated case sees it
      today — but `reduce_breakloop`, `reduce_dangling` and `reduce_laterals` do
      gate r4133, so RP4.1 gets this in writing rather than re-investigating it.
      Written into the pin's doc-comment beside the sentence that used to claim
      `length` "already agreed digit for digit" — true of `midi_reduce`, false of
      the micro-decks the sentence sat next to.
    - **The routing table still described the defect in the present tense — REAL,
      fixed** (both auditors, independently). `props_r4133_replay.rs`'s
      `RP22_ROUTING` comment for `line.units` still read "the port's
      matrix-series branch does the two in the opposite order" and "the port's
      `reset_length_units` clears `user_length_units`", with line references that
      the fix had moved. It is the one place in the tree where the mechanism sits
      beside its routing, and RP3.6's row sits directly under it. Rewritten as a
      settled `FIX` verdict naming both Rust sites, both lanes, the ledger entry,
      the pins and the derivation test; the citation column now reads
      `RP3.5 FIXED (2026-08-28) — …`. `Owner::Rp35`'s doc gained the sentence
      `RP3_ROUTING` already carries: a settled sub-step does **not** leave the
      accounting bucket, because no `Link` reads the ledger.
    - **The census decomposition was prose — REAL, derived now.**
      `the_rp35_census_decomposition_is_read_off_the_corpus` joins the RP3.1–RP3.4
      family: it sweeps the corpus **file universe** for a bare `Reduce` (with
      `files.len() > 1000` as the vendoring tripwire and the one non-script
      carve-out — `SyntaxFiles/opendss.stx`, a keyword list — named and asserted
      unique), reads each deck's `Set ReduceOption=` and whether it declares a
      line that is not 3-phase symmetrical-components, and combines the two: a
      cell needs a strategy that calls `MergeWith` at all
      (`ReduceAlgs.pas:54`/`:214`/`:258`/`:319`/`:361`, never the branch-disabling
      `:61`/`:87`/`:101`/`:372`/`:453`) **and** a line that can take the matrix
      branch (the negation of `Line.pas:1695`). Exactly one deck satisfies both —
      `midi_reduce` — and both halves are load-bearing on today's corpus:
      `reduce_laterals` declares 1-phase laterals but only removes branches, and
      `reduce_mergeparallel` merges but only 3-phase sym lines, which is RP3.5's
      counter-claim to the plan text asserted rather than told. Scope is
      skip-aware (`engines=` **and** no ledger `skip`, `r4133_skipped_cases` +
      the `SKIP_WITNESS` assertion — the RP3.4 house rule), and the products
      reconcile against `bins.tsv` and `examples_full.txt`: 3 cells, 0 in scope.
      Three reduce decks *do* gate r4133, asserted so the zero is a statement
      about this pair and not about the channel.
    - **No pin read the corpus deck the sub-step exists for — REAL, added.**
      `exec::tests::reduce::the_corpus_reduce_decks_merged_lines_render_kft`
      compiles the vendored `modes/reduce/midi_reduce.dss` and asserts the three
      merged lines render `kft` (and `length = 4`), with the deck's own
      discriminators: the un-merged `Line.bb1_2` also reads `kft` and the
      `switch=yes` `Line.tie` reads `none`, all four measured on the r4133 DLL.
      The ledger entry pins the same three cells from the other side, but that
      half needs the pinned dss-python installed; this one needs only the
      vendored deck.
    - **"Nothing else moves" understated the blast radius — REAL, corrected and
      pinned.** The restored units also move two length-derived quantities that
      no property renders and no gated case compares: `miles_this_line` (the
      `4,20:` side effect, `Line.pas:667-670`, feeding the reliability registers)
      and `line_length_km` (feeding the EnergyMeter zone's line length). Both
      were wrong before — miles kept the *pre-merge* value the matrix branch
      never recomputed, and the zone counted a length in miles as kilometres — so
      both are corrections; `exec::tests::reduce::merged_matrix_line_converts_
      its_length_with_the_restored_units` asserts them instead of leaving the
      change silent. The solved-state claim is unchanged and re-verified:
      `units_convert` is 1.0 either way, so Y, V, I, S, losses, node count and
      iteration count do not move.
    - **Test count 4 208 → 4 213 per lane (8 416 → 8 426), +5.** Four
      `exec::tests::reduce` pins
      (`merge_repoints_the_controls_of_the_surviving_line`,
      `parallel_merge_with_a_switch_recalcs_before_the_cap_fixup`,
      `the_corpus_reduce_decks_merged_lines_render_kft`,
      `merged_matrix_line_converts_its_length_with_the_restored_units`) and one
      replay derivation
      (`the_rp35_census_decomposition_is_read_off_the_corpus`). RP3.5 itself had
      taken 4 204 → 4 208 with its four pins and recorded no count; the ledger is
      picked back up here.
    - **Gate.** All five commands green in both lanes, **4 213 tests per lane**
      (0 failed, 0 filtered; the four `ignored` are the pre-existing doctest
      markers), corpus gate 131/131 with the entry still at 3 hits. `lane_diff`
      was re-run because this commit *does* move solved state on the
      partner-is-switch arm (no gated case reaches it): **VERDICT PASS,
      `max |Δ| = 0` exactly** on all eight kinds (conv/cur/errs/iter/loss/pow/v/y,
      3 220 247 records over 522 cases, 0 iteration counts drifted), so the two
      lanes stay bit-identical.


## Full sub-step records RP3.6 – RP3.13 and RP3.10 (moved from STATUS §1)

**RP3.6 part (a) (`line.linecode`, the switch arm) landed 2026-08-29 — the
second `FIX`, and the one RP4.1 actually waits on.** r4133's `switch=` side
effect (`Version8/Source/PDElements/Line.pas:694-700`) assigns
`r1/x1/r0/x0/c1/c0/len` as fields, kills geometry and spacing and resets the
length units, and carries **no** `FLineCodeSpecified` statement — while the two
neighbouring impedance arms each open with one (`6..11, 26..27` at `:685`,
`12..14` at `:691`), so the omission is written arm by arm rather than forgotten
at the end of a block. The flag is read live by the property getter (`3: If
FLineCodeSpecified Then Result := CondCode else Result := ''`, `:1357`) and by
the `units=` arm, which picks `ConvertLineUnits(FLineCodeUnits, NewLengthUnits)`
when it is TRUE and `FUnitsConvert * ConvertLineUnits(LengthUnits,
NewLengthUnits)` when it is FALSE (`:626-627`). dss_capi 0.14.5 **added** the
kill there and flagged it in its own source (`src/PDElements/Line.pas:677`,
`KillLineCodeSpecified(); //TODO: check if this missing is relevant bug`); the
port had copied 0.14.5. r4133 is the behavioral authority (CLAUDE.md
2026-08-02), so the single call is gone from the `SWITCH` arm of
`elements/pd/line/accessors.rs` in **both lanes**; RP3.5's `user_length_units`
line in `reset_length_units` is untouched. (The call-site accounting in this
paragraph as first written — "the six other … each match an r4133 counterpart" —
was wrong twice over and is corrected by the audit settlement below: seven
remained after the deletion, r4133 has eight, and the missing eighth
(`FetchConductorList`, `:1853`) was ported on 2026-08-29.)

Probed live on all three engines (r4133 DLL 11.0.0.1, the pinned 0.14.5 oracle,
the port) before any edit, per the plan's "Do first". **The premise held and the
kill criterion did not fire.** On the corpus shape (`LineCode.99` in metres, the
line `linecode=99 … Switch=True units=m`) exactly one property cell differs —
`linecode`, `'99'` on r4133 against `''` on capi and the port — while `units`,
`length`, `r1`, `x1`, `r0`, `x0`, `c1`, `c0` and the three matrices agree
numerically on all three engines. On the discriminating shape the corpus never
has (a code in **kft**, the line in **m**) the flag proves it is not cosmetic:
r4133 renders `r1 = 0.00328084 = 1/304.8` where capi and the pre-fix port render
`1`, and a later `Edit … units=kft` flips r4133 back to `1` and the other two to
`304.8` — the branch is re-evaluated from the code's units on every `units=`,
never latched. Controls: a switch with no code, and a `switch=` typed *before*
`linecode=` (which `FetchLineCode` re-arms at `:413` — why the corpus's 160
`LVTestCaseNorthAmerican` declarations carry zero cells), agree on all three
engines. **No solved state moves**: read at f64 from the live `YNodeVarray` and
every element's terminal currents, `r4133 vs port` is `max rel |dV| = 3.6e-10`
and `max rel |dI| = 9.7e-08` — *smaller* than `capi vs port` (`4.8e-10` /
`1.9e-07`) on the same deck, whose flag is identical, so the residue is the
documented faer-vs-KLU near-cancellation floor on an ideal switch and not a flag
effect. Both branches of `ConvertLineUnits` evaluate to exactly 1.0 on every one
of the five cells (`m`→`m` and `none`→`m`), so **no impedance and no golden byte
moves** — measured deck by deck across the seven CIM goldens, all 322
`props/*.json` scenarios, `json/*`, `reports/dump3_*`, `reports/save_*` and
`save_roundtrip.rs`, and `golden.lock.json` is unmoved.

What does move is the **live capi comparison**, on exactly five cells over two
`engines: both`, `kind: feeder` cases whose property compare `force_properties`
turns on today — so the exclusion ships **now** rather than staging to RP4.1
(§1.1(e) is an r4133 rule, that channel's props compare being masked):
`line-switch-keeps-linecode-zone2-capi-props` (`line.261249` `99`,
`line.183046` `98`, `line.255376` `99`) and
`line-switch-keeps-linecode-zone3-capi-props` (`line.175078`, `line.249319`,
both `99`), oracle `""` on all five, one new cause `line-switch-kills-linecode`,
`population.lock.json` two lines. No r4133 entry is drafted or staged: after the
RP4.1 unmask these five cells compare and **match**, which is the point of the
sub-step — `line.linecode` is, by `DECLARED_RP35`'s own comment, the only RP3.5+
pair the unmask will actually compare. No `PROPS_ECHO_R4133` row (the getter is
live, not an echo), no `PROPS_NORM_R4133` row, and no `SKIP_PROPS` /
`SKIP_PROPS_CAPI_ONLY` / `LANE_SKIP_PROPS` row — those are `(class, prop)`-keyed
and would blind `line.linecode` on ~77 659 capi cells to cover five.
`DECLARED_RP35 = (8, 6, 5)` is unmoved (a `FIX` does not claim the frozen census
rows, which record the *old* port value), and the frozen extracts under
`tests/corpus/props_r4133/` stay frozen — their `rust=''` column is historical
from here on.

Held by one oracle-free pin in both lanes,
`exec::tests::line_fetch::switch_yes_keeps_the_linecode_and_its_units_conversion`,
which reads the `FUnitsConvert` branch through `r1` as well as the string under
test, so a port that merely stopped erasing the name would still red it.
**Non-vacuity proven by re-running the pre-fix engine** (the kill restored in a
scratch edit, reverted after): the pin fails at `left: "" / right: "lckft"`, and
the two ledger entries fail with
``ledger `line-switch-keeps-linecode-zone2-capi-props` property
line.261249.linecode: rust value "" != pinned rust "99"`` and the same for
`line.175078` on zone_3. Five of the pin's assertions discriminate that way; the
rest are invariance controls that hold on both engines and would catch an
over-broad fix. The "5 cells, all 5 in scope" split is derived from the corpus
by `the_rp36_census_decomposition_is_read_off_the_corpus`, which sweeps every
corpus file for `Line` declarations carrying both a `linecode=` and a `switch=`,
walks each case's `Redirect`/`Compile` closure to find the owners, applies
`force_properties`' rule and the skip-aware r4133 predicate, and reconciles the
products against `examples_full.txt` and `bins.tsv`.

Two plan-text corrections, both factual: the plan and this file named
`Examples/StoCtrl_Current_PeakShave/Line.DSS` as "the affected decks" — it has
the right shape on nine lines but its case is `kind: large`, which
`force_properties` never property-compares, so it contributes **zero** of the
five; the decks are
`ADiakoptics/EPRI_Ckt7-G/Torn_Circuit/zone_2/Branches.dss:93,:95,:479` and
`zone_3/Branches.dss:161,:165` (the census test asserts the counter-claim, not
just the total). And "the capi channel proven unmoved" was wrong as written: the
capi *oracle* is unmoved, the capi *comparison* is not.

`lane_diff.ps1` was **run rather than argued** — the deduction says it is not
owed (no `compat` kernel, no lane alias, and both `ConvertLineUnits` branches
evaluate to exactly 1.0 on every affected line), but these are near-ideal-switch
decks where one ULP on a 1e-6 switch admittance is 1.45 kW
(`crates/dss-core/src/compat.rs:95-115`), so the claim is a measurement:
**PASS, `max |Δ| = 0.000e0` exactly on all eight kinds** (conv 2150, cur
1 169 500, errs 518, iter 2150, loss 366 320, pow 1 169 500, v 375 744,
y 1 738 048 records over 522 cases), 0 iteration counts drifted.
Recorded and handed on, not chased: with `clear_seq(prop::LINECODE)` no longer
running in that arm, `Dump` and `Save Circuit` now print `LineCode=` on a
switched line — toward r4133, which prints `CondCode` unconditionally
(`Line.pas:1273`) and flag-gates its `Save`; but the port's `Save` still emits
`R1..C0` there where r4133 emits none (its `set_as_next_seq(R1..C0)` block is
0.14.5's `PrpSequence` bookkeeping), so a save→reload loses the name to arm 6
while the numbers round-trip identically. (On the *discriminating* shape — a code
in kft, the line in m — the emitted scalars are `R1=1/304.8 …`, not the `R1=1`
this paragraph first said: `Save` writes the getter's value, which is
`R1/FUnitsConvert`. Corrected by the audit settlement below; measured on the
port's own `Save Circuit` output.) No golden or gate reads either surface
on a switched, linecode-bearing line; the general store-vs-live serialization
question, and whether `Dump` should print the raw `CondCode`, belong to
**§RP3.11**. Part **(b)** of RP3.6 — splitting `FLineCodeSpecified` from
`CondCode` so the port keeps the name across a flag kill, and repointing the CIM
LineCode-units back-fill onto the `CondCode` string match r4133 uses
(`ExportCIMXML.pas:3877`, where r4133 writes
`PerLengthSequenceImpedance.r = 0.301/304.8 = 0.00098753281` and capi and the
port write `0.301`) — is a separate change and has **not** landed here. One
incidental, unrelated finding: both oracles emit `<cim:ACLineSegment.b0ch>`
twice on an `ACLineSegment` where the second should be `g0ch`; the port already
writes `g0ch` correctly.

**RP3.6 part (b) (the `FLineCodeSpecified`/`CondCode` split and the CIM units
back-fill) landed 2026-08-29.** r4133 keeps **two** independent pieces of
linecode state where the port kept one. `FLineCodeSpecified`
(`Version8/Source/PDElements/Line.pas:57`) is raised by `FetchLineCode` (`:413`)
and cleared at eight sites — the impedance arm (`:685`), the matrix arm
(`:691`), `FetchLineSpacing` (`:1832`), `FetchConductorList` (`:1853`),
`FetchWireList` (`:1952`), `FetchCNCableList` (`:2016`), `FetchTSCableList`
(`:2075`), `FetchGeometryCode` (`:2131`) — and by the constructor (`:853`).
`CondCode` (`:103`) is written by that same `FetchLineCode` (`CondCode :=
LowerCase(Code)`, `:387`, inside its `IF LineCodeClass.SetActive(Code)` success
branch) and cleared by **nothing but the constructor** (`:825` — the routine
around that line is `TLineObj.Create` itself, whose port counterpart is
`Line::default`, which already starts the name empty). Two surfaces read the raw
name past a kill — `DumpProperties`
(`Writeln(F,'~ ',PropertyName^[3],'=',CondCode)`, `:1273`) and the CIM
LineCode-units back-fill (`if pLine.CondCode = pLnCd.LocalName`,
`Common/ExportCIMXML.pas:3876`) — while the property render (`3: If
FLineCodeSpecified Then Result := CondCode else Result := ''`, `:1357`), the
`units=` conversion branch (`:626-627`) and the CIM
`Conductor.length`/`LineCodeRefNode` branch (`ExportCIMXML.pas:3734-3738`) read
the flag. dss_capi 0.14.5 has **no `CondCode` field at all**: its
`KillLineCodeSpecified` NILs `LineCodeObj` (`src/PDElements/Line.pas:1994-1999`)
and every render goes through the object, so it cannot tell the two apart — and
that is the data model the port had copied.

The port now models both. `line_code_specified` is the flag; `line_code_name` is
`CondCode` and survives every kill; `line_code_ref` — a port convenience with no
r4133 counterpart (r4133's `LineCodeObj` is a *local* of `FetchLineCode`,
`:376`) — carries the flag's lifetime, so a superseded code cannot be resolved
through it. Five product sites, both lanes: `kill_line_code_specified`
(`elements/pd/line/code.rs`) drops the flag, the handle and the set-order mark
and no longer erases the name; `fetch_line_code` raises the flag where r4133
does (`:413`, right after `FLineCodeUnits`); the property-3 render and the
`units=` branch (`elements/pd/line/accessors.rs`) read the flag; `Dump` prints
the raw name unconditionally (`elements/pd/line/dump.rs`, r4133 `:1273`); and
`cim/export.rs::find_line_units_for_linecode` matches the **name**
(`ExportCIMXML.pas:3872-3884`) instead of the live handle, keeping the `enabled`
test, the `Units = UNITS_NONE` precondition, the first-match break and the
case-insensitive compare. `has_line_code` at the ACLineSegment branch is the
flag, matching `:3734`. All eight `kill_line_code_specified` call sites were
re-read against r4133 under the new semantics: seven match a flag-clearing
counterpart one-for-one and stand; the eighth (the `switch=` arm) was already
deleted by part (a).

**Two measured behaviour changes, both toward r4133, both new here.**
(1) `Dump line.<x>` on a line whose code was superseded now prints
`~ LineCode=<code>` where the port (and 0.14.5) printed nothing — RP3.6's probe
deck C measured r4133 answering `''` to `? Line.q.linecode` *and* `lcnone` to
`Dump Line.q` on the same object, which is the split's decisive observation.
Part (a) had handed the `Dump` question to §RP3.11 on the dossier's routing;
under the 2026-08-02 policy it is a plain r4133-vs-0.14.5 render difference with
a one-line fix and no golden byte behind it, so it is settled here instead, and
§RP3.11 keeps only the `Save`-side `set_as_next_seq(R1..C0)` residue —
*settled 2026-09-03 by that sub-step: the 0.14.5 property-tracking stamps stay
(the AltDSS JSON export is captured with them), the re-emitted `R1..C0` are the
live switch values, and the divergence from r4133 is pinned rather than
removed*.
(2) The CIM units back-fill now adopts the units of a line whose flag was
cleared: on `linecode=lcnone units=kft length=2 r1=0.301` (the LineCode declared
without `units=`), r4133 writes `PerLengthSequenceImpedance.r = 0.301/304.8 =
0.00098753281` where 0.14.5 and the pre-split port wrote `0.301` — RP3.5's
number, reproduced by the RP3.6 probe on decks D and E and now produced by the
port. The class this widens is every line that names a code and then overrides
it (impedance, matrix, geometry, spacing, wires, cables), not only the switched
ones part (a) restored.

**Golden blast radius: no committed byte moves — measured, not reasoned.** The
render is the only property surface, and flag-gating it reproduces exactly the
pre-split answer (the old `line_code_ref` fell wherever the flag now falls), so
`props/*.json` — including `line.json::line_code_then_r1`, which pins
`"LineCode": ""` after an arm-6 override — and `json/**` are unmoved; the `Save`
writer is set-order-gated *and* render-gated, so `feeders_controlsoff/*` and
`save_roundtrip` are unmoved. For `Dump`, all six line-bearing report goldens
were read deck by deck: `reports/dump_line_switch` (a switch with no code),
`dump_line_lc`/`dump_line_sym`/`dump_line_geo`, and `reports/dump3_bare`/
`dump3_debug` (`~ LineCode=lc` on a line with **no** override) — none contains a
line that names a code and then supersedes it, so none reaches the changed
branch. For CIM, the six synthetic decks in `tools/golden/cim_decks/` declare
`units=` on every `LineCode` (`lc_sym` kft, `mtx606`/`mtx607` mi, `lc1` kft), so
the `Units = UNITS_NONE` loop never runs there at all; `IEEE13Nodeckt` and
`IEEE123Master` do have codes without units, but neither deck contains a single
line combining a `linecode=` with an override (their switches carry `r1=1e-3`
and no code, and neither issues an `Edit Line.`), so the match set is unchanged.
The two corpus decks that export CIM (`Examples/CIM/IEEE13_{Assets,CDPSM}.dss`)
have the same shape and put `export cim100` last, after the final `Solve`, so
the writer's `pLnCd.Units` mutation — which r4133 performs identically, on a
wider match set than 0.14.5 — cannot reach a solve. Empirically: `golden_cim`
(13), `golden_reports` (305), `golden_json` (305), `props_roundtrip`,
`save_roundtrip` (9) and `golden_lock` (4) are green in both lanes and
`tests/golden/golden.lock.json` did not move. No `tests/corpus/ledger.json`
entry and no `population.lock.json` line moves either: the corpus gate compares
properties through `? name.prop` (the flag-gated getter — `dss-epri`'s
`capture_all_properties` and the capi channel alike) and compares neither `Dump`
text nor CIM XML, so part (b) is invisible to it. Measured rather than deduced:
the **full 521-case gate** was run in both lanes and
`corpus_gate_all_cases_match_engines` is green with no new pin.

Held by two pins, both oracle-free and in both lanes.
`exec::tests::line_fetch::linecode_name_survives_the_flag_that_gates_its_render`
builds probe deck C's shape plus a `geometry=` sibling and reads both surfaces on
the same objects: `? …linecode` is `''` for the two superseded lines and the name
for the control, while `Dump` prints `~ LineCode=lcnone` for all three; it also
pins the arm-6 side effects that came with the kill (`units = none`, `r1 =
0.301`). `golden_cim::cim_linecode_units_backfill_matches_the_condcode_string`
exports CIM100 over five codes — one superseded by `r1=`, one by `geometry=`,
one reached only through a switched line, one declaring its own `units=kft`, one
referenced by nobody — and pins r4133's `0.00098753281` on the first three
against `0.301` for the unreferenced control. **Non-vacuity measured for each,
by reverting the product change and reading the failure text**: restoring the
name-clear in `kill_line_code_specified` fails the engine pin with `Dump Line.q
must print the raw CondCode 'lcnone'` (an empty dump line) and the CIM pin with
`lckill: … left: Some("0.301") right: Some("0.00098753281")` — in that same run
`lcsw` still reads `0.00098753281`, which isolates part (a)'s switch arm from
part (b)'s name survival; restoring `line.line_code_ref.is_some()` in
`find_line_units_for_linecode` fails the CIM pin the same way on `lckill` and
`lcgeo` only; and un-gating the render fails the engine pin at `left: "lcnone" /
right: ""` **and** reds a committed golden (`props_roundtrip: scenario
line_code_then_r1 property LineCode: structure differs (actual "mtx601" vs
expected "")`) — the tripwire part (a) predicted.

`lane_diff.ps1` is not *owed* — part (b) touches two render surfaces and the CIM
writer only, with no impedance, no `Y`, no `compat` kernel and no lane alias, and
the one state mutation in reach (`pLnCd.Units` during CIM export) happens after
the last solve in every deck that reaches it — but it was **run anyway and
reported as a measurement**, the RP3.5 and part-(a) precedent: 522 cases,
3 220 247 records, `max |Δ| = 0.000e0` and `max rel = 0.000e0` on all eight kinds
(`conv`, `cur`, `errs`, `iter`, `loss`, `pow`, `v`, `y`), 0 iteration counts
drifted, **VERDICT: PASS**. The default lane stays bit-identical to the parity
lane, so it keeps the parity lane's oracle standing.

The full five-command gate is green on the committed tree in both lanes (`fmt`,
both `clippy` runs, both `cargo test --workspace` runs — exit 0 read
individually; nothing `#[ignore]`d, no tolerance touched, no golden
regenerated). One hygiene note for whoever runs the workspace suite next: the
`Test/AutoTrans` decks export to *relative* paths and leave a family of ten
untracked `Auto*_{HL,HT,LT}_{current,losses,power}.txt` files inside the tracked
corpus mirror — a corpus-deck defect, not ours; they were deleted and not
committed (`lane_diff.ps1`'s own artifact sweep cleans the same class).

Recorded, not chased — each with an owner, none silently dropped. **(i) The
object-ref miss path.** r4133's `FetchLineCode` else-arm is a single
`DoSimpleMsg('Line Code:' + Code + ' not found for Line object Line.' + Name,
180)` (`:466`) that leaves `CondCode`, the flag and the impedances untouched; the
port routes `linecode=` through the generic `ObjectRef` parse, which logs
0.14.5's #401 text and stores an **empty** name, so a failed `Edit` after a good
`linecode=` erases `CondCode` where r4133 keeps it. The codebase already has the
mechanism for the r4133 shape (`PropDef::ref_miss_message`, used by AutoTrans
`XfmrCode`), but its `{prefix}{name} not found.` format cannot express r4133's
` for Line object Line.<name>` suffix, so adopting it is a message-parity change
rather than a one-liner — it belongs with the r4133 diagnostics work, not here.
Unreachable in the gated corpus and in every golden deck: a sweep of all
`linecode=` tokens against every `New LineCode.<name>` in `tests/corpus`,
`tools/golden`, `tests/golden` and `crates/dss-core/tests` finds exactly one
miss, `reconductor … linecode=oh_750_aac` in
`Examples/Scripts/ReconductorExample.dss`, which is `not_an_entry_point`.
**(ii) `MakeLike`.** Both oracles copy **none** of the linecode state — r4133
`TLine.MakeLike` (`Line.pas:735-787`) and 0.14.5
(`src/PDElements/Line.pas:889-930`) copy the impedances, `Len`,
`SymComponentsModel` and `FCapSpecified` only, so a `like=` line renders
`linecode = ''` on both — while the port copies name, handle, flag,
`line_code_units`, `units_convert`, `length_units` and `user_length_units`. The
new flag joins that copy set so the port's state stays internally consistent
(the pin asserts the pair cannot drift); narrowing the whole set is a separate
change with numeric reach (`length_units`/`units_convert` feed `FUnitsConvert`),
it has no census cell — no corpus deck writes `like=` on a coded line, which is
why `line.linecode` is 5 switch-shaped cells and nothing else — and it belongs
with a class-wide `MakeLike` sweep. **(iii) The
`Conductor.length`/`LineCodeRefNode` branch is unreachable for a switched line
on all three engines** — it sits in the `else` of `if IsSwitch then`
(`ExportCIMXML.pas:3709`), measured on the probe's deck C, where the switch
exports as a bare `LoadBreakSwitch` identically everywhere — so repointing
`has_line_code` onto the flag has no observable consequence today and is a
faithfulness change only. **(iv) Name casing.** r4133 stores `LowerCase(Code)`
and the port stores the *resolved object's* name (lowercased at creation), so
the two agree; the CIM comparison stays `eq_ignore_ascii_case` because both
engines compare two already-lowercased strings, and no corpus deck names a code
in mixed case. **(v) `FetchConductorList`'s flag clear has no port counterpart.**
r4133's `Conductors=` (property 34, dispatched at `:654`) enters
`FetchConductorList`, which opens `FLineCodeSpecified := False;
KillGeometrySpecified;` unconditionally (`:1853-1854`) — a third rule, distinct
from `SetWires`' `FPhaseChoice = Unknown` guard (`:1952`) and from the `switch=`
arm's silence — where the port's `set_conductors` only fills the array. **This
paragraph originally continued "and cannot acquire an observable one", argued
that `FetchLineCode` kills the spacing at its tail so r4133 reaches the statement
with a nil `FLineSpacingObj`, and recorded the counterpart as written, pinned and
then reverted as vacuous. That argument was wrong and is retracted: `:590-591` is
*dss_capi 0.14.5*'s `FetchLineCode` tail (`src/PDElements/Line.pas:581-582`),
not r4133's — r4133's arm-3 side effect is a *plain* `SpacingSpecified := False`
(`:663`), so the spacing object survives the code and the statement is reached
normally.** Ported, with the measurement, by the audit settlement below.

**RP3.6 audit settlement (2026-08-29, one commit over `4b146ab9`).** Fourteen
findings across the two audit agents. Both re-derived the mechanism on the live
oracles and confirmed the sub-step's own classification — outcome stays `FIX` in
both lanes, the census still decomposes to 5 cells / 5 in scope, both
`capi_v0145` entries, their cause and the two lock lines are unchanged, no
upstream bug is reproduced anywhere, no `TODO(compat)` was added, no test was
deleted, `#[ignore]`d or loosened, and no golden byte, frozen extract or ledger
scope moved. What the settlement adds is **three real product defects fixed**
(five statements, all in the family the sub-step opened), **one new pin and two
extended ones**, **one new machine guard**, and seven corrections to the record.

| # | finding | verdict | evidence | action |
|---|---|---|---|---|
| 1 | `FetchConductorList` (`:1853`) has no port counterpart, and (v)'s justification is false | **REAL** | r4133 DLL: `spacing=sp1 linecode=lc1 conductors=[…]` → `? linecode` = `''`, `r1` = `'----'`, **no error**; the port answered `'lc1'`/`'0.1'` plus a spurious #402 | ported (`code.rs::set_conductors`), (v) retracted above |
| 2 | root cause: `fetch_line_code` kills the spacing where r4133 clears a flag | **REAL** | the same probe: reaching `conductors=` at all proves `FLineSpacingObj` survived; `:663` is a plain assignment, `:581-582` is capi's | `spacing_specified` field; the tail is now the flag only |
| 3 | arm 15 calls `Kill*Specified` where r4133 assigns two Booleans (`:696`) | **REAL** (spacing) / **REAL but unobservable** (geometry) | A/B on identical decks: after `switch=yes` r4133 runs a following `conductors=`; after `r1=` it **access-violates**. For geometry every r4133 reader of `FLineGeometryObj` is itself flag-gated, and `? …geometry` reads `''` on both engines | spacing → plain flag drop; geometry left as-is, argued and cited in the arm |
| 4 | the `FUnitsConvert` consequence is pinned only through the rendered `r1` | **REAL** | with the RP3.6(a) kill restored the port solves `A.1 = B.1 = 7197.75809229906`, 6.4e-6 off r4133's `A.1` — 6 400× the pin's tolerance | solve leg added, pinned against the r4133 DLL's `YNodeVarray` |
| 5 | a landed `capi_v0145` property entry has no machine-checked witness pin | **REAL** | deleting `switch_yes_keeps_…` left the suite green (the entry pins both sides, nothing pinned that the port's side is *right*) | `LANDED_PROPERTY_ENTRY_PINS` + `every_landed_property_entry_has_a_witness_pin_that_exists`, both halves proven red |
| 6 | the new `Save Circuit` emission is unpinned; STATUS said `R1=1` | **REAL** | the port emits `… LineCode=lckft … Switch=Yes R1=0.00328083989501312 …` — the getter's value, i.e. `1/304.8` | Save leg added to the pin (+ an arm-6 control); the STATUS sentence corrected in place |
| 7 | the third deliberate CIM-writer divergence is not listed with the other two | **REAL** | zero-footprint today (every `New LineCode` in `tools/golden/cim_decks/*.dss` declares `units=`), so a fourth rewrite would red `cim_writer_divergences_are_pinned` | recorded in `expected_cim`'s doc with the reason it is not a rewrite |
| 8 | "the six other `kill_line_code_specified` call sites" | **REAL** (arithmetic) | seven remained after part (a); r4133 has eight; the port now has all eight | corrected in `ledger.json`'s cause, `STATUS` §RP3.6(a), the routing comment |
| 9 | `line_code_ref` is dead product state with a doc promising a reader | **REAL** | `grep line_code_ref crates/` → writes only, plus one test reader; every product site reads the flag | doc rewritten to say exactly that; the field stays as this class's member of the typed-handle family |
| 10 | four Pascal citations point at capi while reading as r4133 | **REAL** | `:544-547` is capi's `NoPropertyTracking` block; `:590-591`, `:2141`, `:2042` likewise | all four re-cited (capi labelled as capi, r4133 line numbers fixed) |
| 11 | the pin asserts `line.cp.linecode == "lcnone"`, which both oracles contradict | **REAL, OUT-OF-SCOPE** | r4133 DLL: `like=` copies the impedances (`? r1` = `'0.1'`) but not the source (`? linecode` = `''`), on a plain and on a switched coded line | assertion relabelled a **divergence lock**; owner created — `ORPHANED_GAPS.md` §1.12 |
| 12 | the `FIX`-shape guard does not reach `RP22_ROUTING`/RP3.5+ | **REAL, recorded** | `the_bin7_root_cause_pairs_are_routed_to_their_sub_steps` walks `RP3_ROUTING` only; `line.linecode` lives in `RP22_ROUTING` under `Owner::Rp35` | left as-is deliberately — finding 5's guard is the obligation that actually bites, and widening the shape guard is a WP-RP3 accounting change, not an RP3.6 one |
| 13 | `assert_eq!(swk.r1, "0.00328083989501312")` is a self-comparison, not an oracle value | **REAL** | r4133 renders the same f64 as `0.00328084` (`%-.7g`) — the literal is the port's own width | the numeric `1/304.8` assertion now comes **first**; the literal follows, labelled as a render-width lock |
| 14 | `has_line_code`'s repoint onto the flag is unverifiable | **REAL, recorded** | flag and handle rise and fall together at every site, and the `Conductor.length` branch is unreachable for a switch (`ExportCIMXML.pas:3709`) — measured on all three engines by the sub-step's probe | left as a faithfulness change, as part (b) already said; finding 9's doc rewrite is what keeps the handle honest |

**What changed in the product (both lanes, five statements).** The port now
models `SpacingSpecified` the way it models `FLineCodeSpecified` since part (b):
a Boolean field (`Line::spacing_specified`) beside the objects, not a predicate
over them. r4133 raises it only in the `21..22, 24..25, 34` block and only once
both the spacing and a conductor list exist (`:704-713`); `KillSpacingSpecified`
is guarded by it (`:2268`) and takes the objects with it (`:2266-2276`); the
`linecode=` side effect (`:663`) and the `switch=` arm (`:696`) drop the flag
alone. dss_capi 0.14.5 cannot express the difference — its `SpacingSpecified` is
`Assigned(LineSpacingObj) and Assigned(LineWireData)` (`src/PDElements/
Line.pas:2112-2115`) — and the port had copied it. `set_conductors` gained
r4133's `FLineCodeSpecified := False; KillGeometrySpecified;` (`:1853-1854`), so
all **eight** of r4133's clear sites now have a counterpart and the `switch=` arm
still has none. Four measured divergences close with it, all four new here:

* `spacing=sp1 linecode=lc1 conductors=[…]` → `linecode ''`, `r1 '----'`, no
  diagnostic (was `'lc1'`, `'0.1'`, spurious #402);
* `spacing=… wires=[…]` + `switch=yes` + `conductors=[…]` runs (was #402);
* `spacing=` alone keeps `SymComponentsModel`, so `? r1` = `'0.058'`, the class
  default (was `'----'` — 0.14.5's timing);
* `spacing=` + `r1=0.55` + `wires=[…]` runs (was 0.14.5's #18102), because the
  kill is a no-op while the flag is down.

r4133's own answer to the destroyed-spacing cases is an **access violation**
(`FWireDataSize := FLineSpacingObj.NWires` after a `DoSimpleMsg` that does not
`Exit`, `:1850-1856` and `:1948-1955`, and the same shape in `FetchCNCableList`
`:2014-2022` and `FetchTSCableList` `:2073-2081`) — reproduced nowhere: the port
keeps its clean #402 / #18102, and the pin asserts that. Written up as
`investigations/to_opendss/
46-line-fetchconductorlist-nil-spacing-access-violation.md` (gitignored,
local-only; RP3.5 took **45**, so the next free number is **47**), with the
`switch=yes`-vs-`r1=` A/B as its reproduction.

**Blast radius: none, and swept rather than assumed.** Grouping each `New`/`Edit`
with its `~` continuations into one logical command, `tests/corpus`,
`tools/golden`, `tests/golden` and `crates/dss-core/tests` hold **2 651** `Line`
edits that name a `spacing=`, across nine files (`line_spacing_asym`,
`IEEE13_Assets` ×2, `IEEE13_LineAndCableSpacing`, `IEEE13_LineSpacing`,
`ieee9500_base`, `makeposseq_line`, `upgrade_spacing_ratings`, `cim_lines.dss`).
**Zero** of them lack a `wires=`/`cncables=`/`tscables=`/`conductors=` in the
same edit, and **zero** combine `spacing=` with `linecode=` — so the flag rises
and falls at exactly the instants the old predicate did. The one interleaving
that could have disturbed it is real and was checked: 2 622 of those edits are
`ieee9500_base`'s `spacing=… units=ft` / `~ normamps=… emergamps=…` /
`~ wires=[…]`, i.e. the ratings land BETWEEN the spacing and the conductors.
Block 2 used to fire twice (at `spacing=` and again at `wires=`) and now fires
once, at `wires=` — after the ratings either way — so its
`clear_seq(SEASONS…C0)` and `got_ratings_after_spacing_conds = false` land on
the same state, which the 521-case gate then confirms.

`cargo test --workspace` is green in both lanes (4 219 tests, 74 `test result:
ok` blocks each) with **no** golden, `golden.lock.json`, `ledger.json` scope or
`population.lock.json` line moved;
`lane_diff.ps1` was re-run as a measurement rather than argued — the solved state
of the gated corpus is out of reach by the sweep above, but this settlement moves
`spacing_specified`, which `CalcYPrim` branches on (`line/solve.rs`): 522 cases,
3 220 247 records, **`max |Δ| = 0.000e0` and `max rel = 0.000e0` on all eight
kinds** (conv 2 150, cur 1 169 500, errs 518, iter 2 150, loss 366 320,
pow 1 169 500, v 375 744, y 1 738 048), 0 iteration counts drifted,
**VERDICT: PASS**.

**Pins.** `exec::tests::line_fetch::
conductors_clears_the_linecode_flag_and_the_switch_arm_spares_the_spacing` is
new: six legs, each an r4133 measurement, including the `switch=yes`-vs-`r1=` A/B
on identical decks. `switch_yes_keeps_the_linecode_and_its_units_conversion`
gained the solve leg (three r4133 node voltages at rel ≤ 1e-9 — measured gap
2.3e-12 — plus the `1 : 304.8` drop ratio) and the `Save Circuit` leg.
`props_r4133_replay::every_landed_property_entry_has_a_witness_pin_that_exists`
is the new guard. **Non-vacuity measured for every one**, by reverting each
product statement and reading the failure text: the `set_conductors` kills →
`left: "lc1" / right: ""`; the `fetch_line_code` tail → `Line.p.Conductors: No
objects are expected!` (#402); the `switch=` arm → the same on `Line.s`; block
2's `idx != SPACING` → `left: "----" / right: "0.058"`; the `KillSpacingSpecified`
guard → `You must assign the LineSpacing before the Wires Property ("Line.g")`;
re-adding `clear_seq(LINECODE)` to the `switch=` arm → the emitted `New
"Line.swk" …` line without `LineCode=`; restoring part (a)'s kill → the solve leg
reads `A.1 = B.1`; renaming the witness pin → `line-switch-keeps-linecode-zone2-
capi-props (RP3.6) names the witness … but … defines no such #[test]`; emptying
`LANDED_PROPERTY_ENTRY_PINS` → `left: {} right: {…three ids…}`.

**Recorded, not chased.** (a) The geometry half of arm 15: r4133 leaves
`FLineGeometryObj` alive with `GeometrySpecified` down, the port nils it. Every
r4133 reader of that object is flag-gated (`:721`, `:1051`, `:1211`, `:1284`,
`:1371-1393`, `:1403`, `ExportCIMXML.pas:3741`), and `FZFrequency` is re-armed by
the next `FetchGeometryCode`, so the difference is unobservable — probed, `''` on
both engines — and the port has no separate geometry flag to spend on it. Argued
in the arm itself. (b) `MakeLike`'s copy set → `ORPHANED_GAPS.md` §1.12, with the
r4133 measurement. (c) The port's arity check on `wires=` (`Line.<x>: Unexpected
number (n) of wires; expected m objects.`) is 0.14.5's addition
(`src/PDElements/Line.pas:841`); r4133 validates nothing and silently runs an
empty loop. Pre-existing, unrelated to the flag, and the pin avoids the shape.
(d) The `? …spacing` render moved *toward* r4133 as a side effect (it now answers
the surviving object's name where the port used to answer `''`); r4133 echoes
`PropertyValue[21]` there — no getter arm at `:1347-1429` — so this narrows the
`line.spacing` census pair (`Owner::Rp23`) and widens nothing.

**RP3.7 (per-phase switch and relay state) landed 2026-09-02 — `FIX` in both
lanes for all three parts, and the widest RP3 sub-step so far: 50 engine, test,
golden and ledger files (plus this record and three docs), the two control classes
rebuilt on r4133's `pStateArray` model, ten overlaid golden cells and five live
`capi_v0145` ledger entries.** The record below is measured
throughout: every byte quoted from r4133 comes from the vendored EPRI DLL
(`Version 11.0.0.1 (64-bit build) - Charlottesville`) through `epri-worker`, and
every count is derived by a test rather than transcribed.

**The probe ran first, on all three engines, and no kill criterion fired.**
r4133 DLL, the pinned dss-python 0.15.7 / dss_capi 0.14.5, and the port built
out-of-tree by path, over seven purpose-built micro-decks plus the vendored
`civanlar` / `makeposseq_ctrl` shapes; no repo byte was written by the probe.
**(1) Per-phase independence is real, on two independent observables.**
`edit swtcontrol.sw1 state=(open, closed, closed)` renders
`[open, closed, closed, ]` and, after `solve`, zeroes exactly phase 1 while
phases 2/3 keep `-6.953452-12.035939j` / `-6.935750+12.030079j`; the bare ganged
`state=open` zeroes all three. **(2) The render is one token per
controlled-element phase** — `[closed, ]` on a 1-phase element, three tokens on a
3-phase one, four on a 4-phase one (which r4133 accepts although it allocates
three entries), and `[]` when there is no controlled element. **(3) The (a2) lock
rule is exactly the source reading**: under `lock=yes` a ganged `normal=open`
moves `Normal` to `[open, open, open, ]` and so does a *quoted* locked
`normal=(open, closed, open)` → `[open, closed, open, ]` — the guard is on the
property **name**, not the value shape — while locked `state=` / `action=` move
nothing. **(4) Relay's resync is the getter's live bound**, not a state-array
write: `MakePosSequence` (`Relay.pas:1008-1027`) does not touch the arrays, and
`GetPropertyValue` 39/40 loop the live `ControlledElement.NPhases` (`:1407-1428`),
as `Sample`, `RecalcElementData` and `Reset` do. **(5) capi 0.14.5 refuses the
per-phase list on two independent channels**, which the plan text had merged into
one: the locked-write channel (`ConditionalReadOnly`) and the enum-mismatch
channel — `SwtControl`'s `StateEnum.DefaultValue := ord(CTRL_CLOSE)` makes a
quoted list fall back to closed **silently**, while `Relay` sets no default and
raises `#303` (`Common/DSSClass.pas:2440-2532`).

**(a) The per-phase port.** `FPresentState`/`FNormalState` become
`[ControlAction; 7]` arrays bounded by `SW_MAX = 6` (`SWTCONTROLMAXDIM`,
`SwtControl.pas:14`) beside `normal_state_set` (`NormalStateSet`, `:42`), both
initialized all-CLOSED by `Create` (`:299-307`) — which is why a fresh control's
`Normal` moves off the port's old `''`. `interpret_switch_state` is
`InterpretSwitchState` (`:410-482`) ported whole: the name-based lock guard, the
always-ganged `Action` arm, the ganged 1..6 fill for an unquoted token, and the
phase-by-phase branch through a **fresh** `dss_parser::Parser` with r4133's
five-token cap (`i < SWTCONTROLMAXDIM`, `:461`), first-character matching and
unlisted slots left unchanged. It is the **single write mechanics** for all three
properties: `Normal`/`State` reach it through a new raw hook
(`set_enum_array_raw`) and `Action` through `set_i32`, which maps its decoded
ordinal back to the canonical first character — so the interpreter carries no
production-dead branch. The drive model is r4133's *parse-time* one: `set_States`
drives `ControlledElement.Closed[i]` mid-Edit (`:532-549`) and
`RecalcElementData` re-drives every phase at `EndEdit` (`:234` → `:347-355`), the
last write of the two, so the port defers a per-conductor
`RefAction::SetConductorsClosed` (was the whole-terminal `SetSwitchClosed`) at
`recalc`. **Bounds: in-bounds by decision.** r4133 allocates three entries and
reads/writes up to six — a latent heap OOB absorbed by FastMM — so the port keeps
six initialized in-bounds slots and reproduces only the observables: the render
and drive bound `state_size()` = `min(6, controlled-element phases)` (`0` for a
nil element) and the five-token parse cap, which stays deliberately *different*
from the six-token render bound. **`WasQuoted` is plumbed** to the property seam
(`obj/props/class_props/{parse,typed}.rs`, `obj/props/engine.rs`,
`exec/{command,json_import,make_pos_seq}.rs`, `obj/base/mod.rs`) because a
**single** token is the one shape where quoting changes the meaning: measured on
r4133, `state=(open)` gives `[open, closed, closed, ]` where `state=open` gives
`[open, open, open, ]`. Seams with no outer parser (JSON import, the
`MakePosSequence` applier, direct calls) reconstruct it — `value_implies_quoted`:
a leading `(` `[` `{` or quote, or more than one token — and a bare single token
resolves to *ganged*, the only spelling any corpus deck writes. Finally the two
enum registry entries gain `allow_longer` and a `Keep` default, so `state=bogus`
is **silently unchanged** as on r4133 — neither the port's pre-RP3.7 error nor
capi's silent close.

**(a2) The locked-`normal` rule landed with it, in both lanes.** The scalar-era
`Locked` gates are gone from `set_i32` and from `side_effects`; the only guard
left is r4133's own, inside the interpreter. Two facts the sub-step measured
rather than read: the guard refuses `Action`/`State` by property name
(`:416-417`, comment *"Only allowed to change normal state if locked"*), and the
`{Supplemental Actions}` block (`:220-228`) sits **outside** the arm, so
`NormalStateSet` latches even on a write the guard refused — the discriminating
pair is a locked `state=open` followed, after `lock=no`, by another `state=open`
(Normal stays `[closed, closed, closed, ]`) against the same sequence unlocked
(Normal follows to `[open, open, open, ]`). `docs/upgrade/DIVERGENCES.md` §D12,
which asserted the opposite refusal and the scalar field mapping, now carries a
`Settlement (2026-09-02)` paragraph with the per-phase model, the lock rule
verbatim, five pin names and the ledger consequences.

**(b) Relay: one fix, two symptoms — and four more mismatches on the same seam.**
The plan framed (b) as a render gap; the probe showed that a single frozen
`ctrl_snap` produces **both** the three-token render *and* a `Sample` that
resyncs phases 2..3 off the end of a 1-conductor element, which is exactly why
the frozen census cell reads `[closed, open, open, ]` and not
`[closed, closed, closed, ]`. Refreshing `ctrl_snap` inside `make_pos_sequence`
(the same refresh SwtControl took) settles both. Under "port gaps immediately"
the probe's four further r4133 mismatches on that seam were fixed with it: a
quoted single token was read as ganged, a refused or unmatched `action=` did not
run the normal-defaults supplemental (`Relay.pas:616-619` covers internal 19 as
well as 40), and the generic tokenizer honored six tokens where `:1286` honors
five. Relay took the same raw write seam (`interpret_relay_state`,
`Relay.pas:1237-1308`), and its old `values.len() == 1 → ganged` heuristic — the
bug behind the quoted-single-token rows — is deleted. r4133's own 6-phase
initialization OOB is **not** reproduced, per the 2026-08-02 policy: `Create`
allocates three entries and the getter loops the controlled element's six phases,
so slots 4..6 are an out-of-bounds heap read with **no defined value**. The audit
settlement measured it both ways on the same DLL — on `decks/b1_relay6.dss`, which
ends in a `solve`, a fresh relay reads
`[closed, closed, closed, open, open, open, ]` (reproducible 2/2), and on the same
construction stopped before the `solve` it reads all-closed; the flip happens at
the `solve`, i.e. it tracks heap content, not the model. The pins therefore gang to
a known baseline first and assert nothing about a fresh 6-phase render.

**The cell arithmetic — and the measurement that decided it.** `bins.tsv` gives
`swtcontrol.normal` and `swtcontrol.state` **59 cells / 40 in scope** each (the
plan's 80), `relay.normal`/`relay.state` **1 / 0** each. The 80 in-scope cells sit
on three `engines=r4133` decks — `controls:swtcontrol/midi_swtcontrol.dss` (12),
`controls:swtcontrol/swtcontrol_time.dss` (12) and the `civinlar model`
`civanlar.dss` (16 ties × 1 step) — i.e. **not** on the decks the fix reds. The
first census run over those three decks (every earlier sweep had covered only
out-of-scope cases) reports **zero** `Normal`/`State` rows on the r4133 channel:
the 160 SwtControl divergence rows that remain there are `Reset` 40, `enabled`
40, `Delay` 24 (RP3.1's staged entries), `Action` 16 (RP2.3 `EchoParse`) and
`SwitchedObj` 40 — none of them this pair, and RP3.7 moved none of them. Read
independently off the r4133 DLL on the same decks — all 16 civanlar ties (13
closed / 3 open) and both duty decks before and after the manifest post — the
renders are **byte-identical** to the port's. So the disposition of the 80
in-scope cells is **80 / 80 COMPARE**, not excluded and not pinned by an
exclusion, and **no r4133 `property` ledger entry is needed, staged or drafted —
zero, as a measurement rather than a judgement**; a landed one would fail
`assert_all_hit` as NEVER APPLIED at RP4.1. Until the RP4.1 unmask lets a channel
witness them, the port's value there is held by plan §1.1(c)'s holder pin
`exec::tests::controls::swtcontrol_state_renders_per_phase_on_the_r4133_only_decks`.
*(**Witnessed 2026-09-03**: the RP4.1 unmask compared exactly those 80 cells —
40 on `swtcontrol.normal` and 40 on `swtcontrol.state` — and every one matched
without normalization, which is what exposed the two `ArrayForm` rows as stale
and retired them into `RP37_SUPERSEDED`; §RP4.1 record. The holder pin stays.)*
The other **19 cells** are out of scope, on five `capi_v0145` cases, and they are
excluded by **five landed entries under one new cause**
(`swtcontrol-per-phase-state-render`): `controls:swtcontrol/swtcontrol_lock.dss`
carries **both** exposure channels — its `state`/`normal` probes *and* its
full-property compare, over 12 steps ⇒ 48 hits —
`modes:makeposseq/makeposseq_ctrl.dss` 2 (the 1-token `[closed, ]` render after
`MakePosSequence`), and the three `IEEE_519.DSS` copies 4 each (two controls ×
two properties). Every cell is a non-numeric exact pair, so both `oracle` and
`rust` are pinned and each entry goes stale the day the port converges to the
0.14.5 scalar. `ledger.json`: causes **26 → 27**, entries **40 → 45**, hits
**1442 → 1504**; `population.lock.json` moved **exactly five lines**, one
`ledger=` field per case. `relay.normal`/`relay.state` need **no** entry on either
channel: Relay is whole-element-skipped on capi (re-measured on `makeposseq_ctrl`
— 2 elements / 80 cells skipped) and both cells are now byte-identical to r4133.
**`DECLARED_RP35` stays `(8, 6, 5)`** — the contract's own answer, verified twice
(the replay was 131 green before the routing rewrite and 132 after): `declare`
reads the **frozen** `rust` column, so a settled sub-step does not shrink the
bucket, exactly as RP3.5 and RP3.6 recorded. `CLAIMED_ARRAY_FORM` (203),
`ROWS_FROZEN`, the other `DECLARED_*` counts and the props count locks 51 / 322 /
8343 are all unchanged. The four routing rows (`swtcontrol.normal`/`.state`,
`relay.normal`/`.state`) are rewritten as settled `FIX` verdicts, and a new
derivation test
`props_r4133_replay::the_rp37_census_decomposition_is_read_off_the_corpus`
reconciles the decks, `population.lock.json`, the frozen census (per spelling —
58 + 1 and 31 + 27 + 1 — and in total) and `ledger.json` against each other, so
"exactly five entries" is derived rather than asserted in prose.

**Goldens: ten cells, one schema block, one prose line, three digests.** The ten
`Normal`/`State` cells of `tests/golden/props/swtcontrol.json` were **predicted
first, then measured on the authority** — the five scenarios replayed through the
r4133 DLL with the gate's own `clear` + `new circuit.propsprobe` preamble — and
the port matches r4133 byte-for-byte on all ten and the committed golden on all
45 unmoved cells; the artifact's diff is those ten lines plus its provenance
prose. `Action` moves in none of the five scenarios. In
`tests/golden/json/schema_full_port.json` (regenerated in its producing parity
lane) `SwtControl.properties.{Normal,State}` become `type: array` + `items: $ref`
with **no** `default` key — the same **array** shape `Relay` already had, though
the audit settlement corrected the stated reason for the missing default: the
emitter elides one for any property flagged `NO_DEFAULT` (`State`) or
`DYNAMIC_DEFAULT` (`Normal`), decided **before** the sample object is read
(`schema/classes.rs::no_default`), and the empty sample array would elide it
independently. That is why the "shape Relay already had" held for `Normal` but not
for `State`, which carried a 3-element default until the settlement ported r4133's
NIL-element getter guard to Relay (below) and emptied its sample array too.
`schema_full_oracle.json` is untouched and `schema_divergences.json` gains one
amended `cause` line. `golden.lock.json` moved
three digests and no anchor. **The anchor decision A2b left open is: keep
`capi015`.** Precedent and evidence point the same way — WP-U2.4 already overlaid
r4133-only behavior on this same artifact, and the G0.1 provenance lock, six weeks
later, registered it `capi015` *with* that overlay in place; `CAPI015_REASON` is a
statement about the unreproducible 0.15.0b4 generator environment, still exactly
true; and r4133 is demonstrably **not** this artifact's value authority — its
`Action`, `Reset`, `Enabled`, `SwitchedObj` and `Delay` cells still hold
capi-side values r4133 diverges from, so an `R4133_FAMILIES` row
(`props/fuse.json`'s shape) would be a false statement and a `DEANCHORED`
"born-self" row a second one. The ten overlaid cells are recorded in the
artifact's own `oracle.engine` block with their r4133 line citations — and, since
the audit settlement, in the **lock row** as well: the shared `CAPI015_REASON`
could not carry a per-artifact note, so a new register `CAPI015_OVERLAYS` appends
one (the construction `props/fuse.json`'s `R4133_FAMILIES` reason states, now
available to an artifact that stays `capi015`). `golden_lock` (4) and
`props_roundtrip` (1) are green in both lanes with the anchor unmoved.

**Pins.** Unit level, both lanes: `swt_control` 26 → **51**, `relay` 63 → **68**,
`dss-core --lib` **1464** (1462 before B2's two engine-level pins) — the
before-counts re-measured by the audit settlement, which corrected the 30/64 this
paragraph first carried and added two more relay pins (`relay` **70**, lib
**1466**; see the settlement paragraph). The load-bearing ones are
`render_is_one_token_per_controlled_element_phase`,
`nil_controlled_element_renders_the_empty_array`,
`a_quoted_single_token_is_per_phase_a_bare_one_is_ganged`,
`per_phase_write_renders_the_r4133_bytes_through_both_seams`,
`the_property_seam_caps_the_per_phase_parse_at_five_tokens`,
`a_multi_token_value_without_the_quote_flag_is_still_per_phase` (the JSON-import
value-string seam), `the_render_bound_follows_makeposseq`,
`a_locked_state_write_still_runs_the_normal_defaults_supplemental`,
`locked_normal_applies_locked_state_and_action_do_not` (plan §RP3.7(a2)'s required
pin — the renamed `locked_ignores_normal_and_state_writes`, now the full probe
byte sequence through the executive),
`per_phase_state_write_through_the_executive_opens_only_its_phase` and
`per_phase_open_zeros_only_its_phase_currents_on_a_micro_deck` (the solved
per-phase drive), plus the Relay twins
(`a_refused_or_unmatched_action_still_runs_the_normal_defaults_supplemental`,
`the_render_bound_follows_makeposseq`, the cap and quoted-token pins). Two
engine-level pins carry the corpus:
`exec::tests::controls::swtcontrol_state_renders_one_token_per_controlled_phase`
— the witness for all five landed entries, reading the two gated decks from disk
and closing with a 1/2/3-phase discriminator so it cannot pass against a
hardwired three-token string — and the §1.1(c) holder above; both were proven
non-vacuous by corruption. `props_r4133_pins::swtcontrol_action_renders_the_live_
switch_state` moved its two `State` lines to the r4133 DLL's own bytes (`13_14` →
`[closed, closed, closed, ]`, `10_14` → `[open, open, open, ]`) and gets *sharper*
by it: `Action` still renders one word beside an array, and the two still agree.

**The verify-A1 round (12 findings, settled inside this sub-step).** One is
**refuted by measurement and must not be carried anywhere**: the "silent no-op
after a failed edit" defect the probe reported (`probe.md` §8.3/§11.6/§12) was a
probe-harness artifact — the harness printed a delta against a cumulative error
high-water mark while every `Compile` in its loop cleared the error list; with
absolute error lists printed, the later edit **applies** in all six variants and a
second failing edit is reported. Two findings were already fixed by the render
part (the JSON seam's per-phase reconstruction; the `min(6, nphases)` clamp), five
landed as code or doc records — r4133's `Else`-without-`Begin` AuxParser
fall-through (`SwtControl.pas:452-455` and `Relay.pas:1277-1306`, with
`Fuse.pas:569-597`'s correct `Else Begin` as the counter-example, which is what
makes it an upstream slip rather than a design); the lock guard keying on
`LowerCase(ParamName[1])` of an **empty** string for a positional write; the
mid-edit `switchedobj=` re-point that makes r4133 drive the *old* target too; and
the registry comment that called `NO_DEFAULT` "the Pascal default 0" — and one
tolerance was **tightened**: the micro-deck current band went from an ad-hoc
`5e-4` *relative* (≈6.9e-3 A, blind to a ~7 mA per-phase drive error) to the
calibrated `abs 1e-6` element-current floor from `tests/TOLERANCE_NOTES.md`, ~500×
tighter, against full-precision r4133 magnitudes whose worst measured delta is
3.66e-7. The first three are `investigations/to_opendss/` candidates, left
unowned. Two findings are **recorded, with both engines measured**. (i) The
retained 0.14.5 `Sample`/`DoPendingAction` machinery arms on a `normal=` write:
after `edit swtcontrol.sw normal=open` on an unlocked control the port opens the
switch at step 3 of the duty run (`State = [open, open, open, ]`, event log
`Hour=0, Sec=0.5, ControlIter=1, Element=SwtControl.sw, Action=OPENED`) where the
r4133 DLL leaves it closed for all eight steps and logs nothing — r4133 comments
both bodies out. A **locked** `normal=` arms it too, which is new with (a2): the
scalar era refused that write outright, r4133's guard lets `n`ormal through, and
the port now applies it — the switch stays shut (`do_pending_action` is
`!locked`-guarded) but the queue push and the `armed` latch happen (audit
settlement; both arms are in the tripwire). Retiring the machinery has **one**
blocking channel, not two: `controls/swtcontrol/swtcontrol_lock.dss` is gated
`capi_v0145` with `compare_ctrlqueue`, so the capi lane pins the spurious
`CTRL_LOCK` push, and retiring the body means re-gating that deck onto `r4133` —
giving up the only capi deck that both probes and property-compares a SwtControl,
and retiring one of the five entries RP3.7 just landed. That is a channel
decision RP3.7 **chose not to take**, not one it could not make (it edited both
`population.lock.json` and `ledger.json` for other reasons); and the capi015
props golden's `Action` readback is **not** a second blocker — it reads
`current_action` through `get_i32(ACTION)`, which the property side effects
maintain and neither `Sample` nor `DoPendingAction` writes. Corpus exposure is
zero (every corpus `normal=` is a ganged `normal=closed` over an all-closed state,
and `swtcontrol_lock.dss` types `normal=closed` *before* `lock=yes` on the same
`New`), the divergence is held by the tripwire
`swt_control::tests::sample_arms_on_a_normal_write_the_retained_capi_channel`,
whose doc says it must be **deleted, not re-baselined**, when the body goes, and
it is now registered as `ORPHANED_GAPS.md` §1.16 — the only RP3.7 item that moves
a solved result, so a test doc was too weak a home for it.
(ii) The render bound's residual staleness is observable on both engines: after
`edit line.swk phases=1` the r4133 DLL's `? swtcontrol.sw1.state` follows
immediately to `[closed, ]` (its getter loops the live element) while the port
keeps three tokens until the ref is re-resolved by a `switchedobj=` write. That is
architectural — the port's classes hold ref *snapshots* by design — shared with
Relay, and has zero corpus exposure; it is now `ORPHANED_GAPS.md` §1.13. Two
smaller ones: `props_roundtrip` has no write-back path at all, so the audit's
suspicion there is void; and the enum `Keep` default can be narrowed for `State`
but never for `Action`, whose scalar seam needs it to reproduce r4133's no-else
fall-through.

**Recorded, not chased** (each with its evidence; the `tmp/rp37/out_*.txt`
transcripts named here are local, gitignored probe artifacts). (a) The
`RecalcElementData` bits the port never had (`SwtControl.pas:333-344`): the
`ElementTerminal > NTerms` check (`DoErrorMsg` 384) is a **genuine validation
gap** — the port silently clamps while emitting the sibling 387 for a missing
element — and goes to `ORPHANED_GAPS.md` §1.15 together with the `FNphases >
SWTCONTROLMAXDIM` warning; `HasSwtControl := TRUE` (`:344`) is **dead upstream
state** (declared `CktElement.pas:99`, initialized `:213`, set here, never read),
so that half closes by reading rather than deferring. (b) The sibling state seams
RP3.7 did not touch — Recloser's twin defects (the same `values.len() == 1 →
ganged` heuristic and a whole-object lock that refuses `Normal` too, where
`Recloser.pas`'s guard is the same name-based rule), Fuse's missing ganged path,
Relay's absent `ControlledElement = NIL → []` render and its `set_States`
`ArmedForReset` — are `ORPHANED_GAPS.md` §1.14; none has corpus or census
exposure. The NIL **render** was deliberately not landed here on a hunch
(`state_size()` has 14 call sites) and the audit settlement landed it after
measuring it: r4133's guard lives in the getters alone (`Relay.pas:1407`/`:1418`),
so the port carries it in a render-only `render_size()` and leaves the twelve
sensing/reset loops on `state_size()`; §1.14(c) keeps the behavioral half (r4133's
`Reset` restores nothing with a nil element, `:1447`) and gains the same unported
guard on Recloser and Fuse. (c) `Dump`'s store-vs-live echo for
properties 6/7 goes to **§RP3.11** (*settled 2026-09-03 —
`KEEP_LIVE_PINNED`: the port keeps the live render on `Dump` too, and the
divergence is pinned rather than reproduced; record below*): r4133's
`DumpProperties` (`:563-571`) echoes
the stored parse text (`~ Normal=` when never written) while the port's generic
dump renders the live value, consistent with `?`, `all_properties` and `Save`; no
golden and no gated cell renders a SwtControl `Dump` today. `Save` itself
round-trips the array (`save_roundtrip` 9 green — the writer emits
`Normal=[closed, closed, closed, ]` and the outer parser reads the brackets back
as a quoted per-phase list). (d) `MakeLike`: r4133 copies **every** other piece of
SwtControl state and omits `NormalStateSet` alone, whose observable is a clone's
first `state=` write silently collapsing the base's declared `Normal` (measured on
the DLL); the port copies the flag, i.e. does not reproduce it, and the omission
is written up as `investigations/to_opendss/47-swtcontrol-makelike-drops-
normalstateset.md` (gitignored, local-only — RP3.6 took 46, so the next free
number is **48**). Verifying that report turned up a **second, previously
unrecorded r4133 defect**, included in it: the same copy loop bounds itself with
`ControlledElement.Nphases` on a pointer copied verbatim one line earlier with no
nil check, so cloning a base whose `switchedobj=` never resolved
access-violates — `Error 303 … Access violation … Read of address
000000000000007C`, caught, the clone left half-built with `? …Normal` → `[]`. The
port has no equivalent (its clone path is snapshot-based and the `[]` render is
pinned), so there is nothing to fix in-engine. (e) `? relay.x.switchedobj` still
renders the defaulted name where r4133 renders `''` — a pre-existing
`CaseFold`-claimed census pair, not this sub-step's.

**A hazard worth carrying: compiling `IEEE_519.DSS` in place rewrites tracked
vendored corpus files.** The deck ends in `export monitor MPCC` + `show monitor
MPCC`, so a hand probe of the three copies left
`.../HarmonicsTMode/IEEE_519_Mon_mpcc_1.csv`,
`.../HarmonicsTMode/IEEE_519_SavedVoltages.dbl` and
`.../HarmonicsVariableLoad/IEEE_519_Mon_mpcc_1.csv` modified; they were restored
by exact path. The corpus gate is protected by its own `CorpusGuard`, but
`lane_dump` is not: the `lane_diff` run reported "restoring overwritten" on three
tracked files (`Test/LineConstantsCode.DSS` and the two
`IEEE_519_Mon_mpcc_1.csv`) and deleted 24 `Test/AutoTrans/*.txt` plus the
`modes/autoadd` outputs — and, worth recording, its hygiene pass correctly
recognised `ledger.json` and `population.lock.json` as **pre-dirty** and left them
alone, so the 2026-08-02 blind-`git restore` accident did not recur. Two
consequences taken: the entry witness pin **rebuilds** the IEEE_519 control block
instead of compiling the deck (the three copies are byte-identical there, the same
two lines at `:45-46`), and `TESTING.md` §Procedures now says in one sentence that
a hand probe of such a deck runs on a copy.

**Corrections this sub-step forced on its own earlier text.** (i) The corpus
carries **eight** SwtControl decks, not five: the first sweep used
`--include=*.dss` and missed the uppercase `IEEE_519.DSS` copies, which carry 6 of
the 19 out-of-scope cells (2 controls × 2 properties × 3 copies). (ii) The
prediction that the homogeneous cells would "fold into the existing `ArrayForm`
rows" had the right conclusion and the wrong mechanism: the replay reads the
**frozen** `rust` column, so no live render can move a bucket there
(`CLAIMED_ARRAY_FORM` stayed 203), and on the live r4133 channel the two renders
are byte-equal, so folding is never reached. (iii) The plan's part (b) is not
render-only — one frozen snapshot produces two symptoms (the three-token render
*and* `Sample` reading past a 1-conductor element), which is why the frozen census
cell reads `[closed, open, open, ]`; both fall to the one refresh.

**Gate.** All five commands green in both lanes, each exit code read individually:
**4 252 tests per lane** (0 failed; the five `ignored` are pre-existing manual
golden-generator and doctest markers, none in a file this branch touches), corpus
gate **523/523** in both lanes (136.2 s / 135.7 s) with the ledger at **45 entries
/ 1504 hits**, every entry hit and none stale. Both predicted reds closed — the
five capi SwtControl cases on the new entries, and the pre-existing
`props_r4133_pins` red on the two-line pin move — and `props_r4133_replay` 132,
`props_r4133_pins` 40, `props_roundtrip` 1, `golden_lock` 4, `golden_schema` 104,
`population_lock` 1, `oracle_parity_cfg_gate` 10. `lane_diff` was re-run because
the drive shape changed (`SetSwitchClosed` → per-conductor
`SetConductorsClosed`): **PASS**, both dumps rebuilt (216 MB each), 523 cases /
3 220 861 records / ~4.83 M compared values, and **`max |Δ| = 0.000e0` and
`max rel = 0.000e0` on every gated kind** — conv 2 162, cur 1 170 100, errs 519,
iter 2 162, loss 366 476, pow 1 170 100, v 375 816, y 1 738 084, all reported
"(identical)", 0 iteration counts drifted — so the 2026-07-31 baseline holds
exactly and the default lane keeps precisely the parity lane's oracle standing.
No solved-state byte moved in either lane.

**RP3.7 audit settlement (2026-09-02) — 11 findings, all minor, all settled;
`FIX` in both lanes.** Two independent audits (audit-code, audit-tests) raised 12
raw findings; deduped to **11** (no overlap — three of them are different asks on
one subject, the retained 0.14.5 `Sample` glue). **Nine fixed, two fixed with a
sub-claim refuted, none dropped.** Every claim was re-derived here: the r4133
source read line-by-line, four probes replayed on the vendored EPRI DLL
(11.0.0.1), three mutations run and restored, and the pre-commit test counts
re-measured off `82022dab^`.

*Code fixes.* **(1) Relay's `ControlledElement = NIL → '[]'` render is ported.**
r4133 puts that guard in the getters and nowhere else (`Relay.pas:1407`/`:1418`);
re-measured on the DLL, a relay with `switchedobj=line.nosuch` answers `'[]'` for
both properties where the port printed `[closed, closed, closed, ]`. The port
carries it in a render-only `Relay::render_size()` used by `array_size` +
`get_enum_array`, leaving the twelve sensing/reset/`MakeLike` call sites on
`state_size()` — which is what `ORPHANED_GAPS.md` §1.14(c) was really deferring,
and what it keeps (r4133's `Reset` restores nothing with a nil element, `:1447`;
`Sample` faults outright, `:1071`, so there is no observable to port). Pinned by
`relay::tests::nil_controlled_element_renders_the_empty_array`, proven
non-vacuous by mutation. One golden byte follows: the schema emitter reads the
sample object, so `Relay.State` loses its `default: ["closed","closed","closed"]`
in `schema_full_port.json` — five lines, regenerated in the producing parity lane,
predicted before it was run. §1.14 gains (d): the same guard is unported on
Recloser (`Recloser.pas:1377`/`:1388`) and Fuse (`Fuse.pas:690`/`:701`), which
belong with those classes' interpreter ports. **(2) The dead ordinal setters are
now held to their interpreters.** `SwtControl::set_enum_array` (and Relay's twin,
which the audit's framing had as deleted — it is not) is a production-dead second
implementation of the lock guard, the five-slot cap and the `Keep` rule, whose doc
claimed the two "can never drift" while its test asserted literals only: the
auditor's mutation of the interpreter's lock guard left it green with the two
paths disagreeing. Both tests are now **differential** — each row drives two
identical controls, one through the ordinal setter and one through the
interpreter, and asserts the r4133 bytes *and* that the two agree. Re-running that
mutation now reds the SwtControl test on `Normal locked=true`, and the Relay
twin — which had **no test at all** — reds on the same shape. The two docs say
what actually holds them together. **(3) The tripwire covers the locked write.**
`side_effects(NORMAL)` is `sample()`'s arming condition, and (a2) made a locked
`normal=` reach it for the first time (the scalar era refused the write). Measured
in-port: `lock=yes` then `normal=open` arms and queues **2** (the `CTRL_LOCK` push
plus an action push r4133 never makes) while the switch itself stays closed. Both
arms are now in `sample_arms_on_a_normal_write_the_retained_capi_channel`.

*Record fixes.* **(4)** The retained-glue blockers were overstated: there is
**one** channel, not two, and RP3.7 **chose not to** re-gate rather than could
not — both corrections are written into the paragraph that describes the
divergence (above, "Recorded, with both engines measured" (i)), not restated
here. **(5)** That divergence also gets the register row it lacked,
`ORPHANED_GAPS.md` §1.16, with its blocker named. **(6)**
The civanlar pin's stated mechanism was wrong: `Normal` is not untyped there —
every one of the sixteen `New` lines declares `Action=c`, so the Edit supplemental
(`SwtControl.pas:219-228`) copies the closed Present into Normal and latches
`NormalStateSet`, which is exactly why the three later `action=o` edits cannot
move it. **(7)** The unit-pin deltas did not reproduce: the before-counts are 26
and 63 (not 30 and 64), re-measured off `82022dab^` and against `--list`.
**(8)** Three artifacts the commit itself edited still named the deleted pin
`locked_ignores_normal_and_state_writes`; all three now name the rename
(`props/swtcontrol.json`'s provenance block, `DIVERGENCES.md` ×2,
`props_r4133_replay.rs`). `R4133_PROPS_PLAN.md` §RP3.7(a2) and
`docs/phase-records/phase-7-wp2.md` keep the old name deliberately: the first is
the pre-landing instruction that *asked* for the re-point, the second is a frozen
phase record of when the test existed. **(9)** `golden.lock.json`'s
`props/swtcontrol.json` row said only "captured on … 0.15.0b4" although ten of its
cells are now the r4133 DLL's bytes. Since the reason is register-derived (all
eleven capi015 artifacts share `CAPI015_REASON`), the fix is a new register,
`CAPI015_OVERLAYS`, appended per artifact, with its own stale sweep and
well-formedness invariants — the shape `R4133_FAMILIES` uses for
`props/fuse.json`'s derivation. The anchor stays `capi015` (r4133 is not this
artifact's value authority: its `Action`/`Reset`/`Enabled`/`SwitchedObj`/`Delay`
cells still hold capi-side values), and the row now says a regen must repeat the
overlay.

*Two findings whose sub-claim is refuted, with the evidence.* **(10)** "The
missing `default` is the nil-element asymmetry" is **wrong**: the emitter's
`no_default` short-circuits on the property flags — `NO_DEFAULT` on
`SwtControl.State` (pre-existing, untouched by RP3.7) and `DYNAMIC_DEFAULT` on
both `Normal`s — before the sample object is read. The proof is `Relay.Normal`
itself: its sample array is non-empty and it still has no default. What the
finding got right is that "the shape Relay already had" was only half true, since
`Relay.State` carries no flag and did have a default; both statements are
corrected in this record and in `schema_divergences.json`. **(11)** "Re-measurement
contradicts the recorded r4133 6-phase render" is **half right**: replaying the
very deck the record cites reproduces
`[closed, closed, closed, open, open, open, ]` 2/2, and the counter-reading came
from a deck without the trailing `solve` — the same session reads all-closed
before `solve` and the OOB bytes after. So the record was not a mis-measurement
but an over-claim: those three tokens are an uninitialized read with no defined
value, deck- and heap-dependent, and both the STATUS text and the two pin docs now
say so. No assertion moved — the pins gang to a known baseline first.

*Gate after the settlement.* All five commands green in both lanes, each exit code
read individually; unit level `swt_control` **51**, `relay` **70** (+2), `dss-core
--lib` **1466** (+2), workspace **4 254** per lane (0 failed, the same five
pre-existing `ignored`), the corpus gate green in both lanes (`corpus_gate` 131,
144.7 s) over an **untouched** `ledger.json` — 27 causes / 45 entries, every entry
hit, none stale — and `population.lock.json`, `golden_lock` 4 (three digests
moved: `props/swtcontrol.json`, `schema_full_port.json`,
`schema_divergences.json` — all predicted), `golden_schema` 104,
`props_r4133_replay` 132, `props_r4133_pins` 40, `props_roundtrip` 1,
`oracle_parity_cfg_gate` 10. `lane_diff` was **not** re-run: the settlement moves
no solved state — the render guard fires only on an object with no controlled
element, which no corpus deck builds, and everything else is tests, docs and
provenance text.

**RP3.8 (the five read-only text surfaces r4133 renders live) landed
2026-09-02 — `FIX` in both lanes, one engine flag, 25 files, and zero golden
bytes moved.** The five pairs RP2.3's kill ruling re-routed here
(`indmach012.pf`, `storagecontroller.kwhtotal`/`kwtotal`/`kwhactual`/
`kwactual`, 1 064 frozen cells / 772 in scope) now render the live computed
value on `?`, `Dump`, `element_properties` and — since the audit settlement
below — `Save`, in both lanes, with no `cfg`.
Every number below was measured — the r4133 bytes come from the vendored EPRI
DLL (`Version 11.0.0.1 (64-bit build)`) through `epri-worker`, the capi bytes
from the pinned dss-python 0.15.7 / dss_capi 0.14.5, and every count is read
back from a test rather than transcribed.

**The probe ran first and the kill criterion did NOT fire.** All five r4133
renders are reproducible from the port's own state, measured cell-for-cell over
9 decks / 100 solved steps + 9 golden scenarios + 2 pin decks, with zero
exceptions, no dependency on r4133's fleet-iteration order and no stale cache on
the port side. The authority arms, read line by line:
`Controls/StorageController.pas:991-994` → `GetkWhTotal`/`GetkWTotal`/
`GetkWhActual`/`GetkWActual` (declared `:136-139`, bodies `:1162-1197`, **all
four** `Format('%-.8g',…)`), whose live inputs are the *properties*
`FleetkW`/`FleetkWh` (`:188-189` over `Get_FleetkW` `:1019-1029` = `Σ
PresentkW` and `Get_FleetkWh` `:1032-1042` = `Σ kWhStored`); and
`PCElements/IndMach012.pas:1790` `Format('%.6g',[PowerFactor(Power[1])])`,
which is state variable **#21** (`:1988`) by construction. **Two source
corrections the probe forced:** `PowerFactor` lives in
`Common/Utilities.pas:1821`, **not** `Shared/mathutil.pas` (the plan text's
pointer was off by a unit — there is no `PowerFactor` there at all); and
r4133's `Var Sum` write-back (`GetkWhTotal`/`GetkWTotal` are handed the object's
own `TotalkWhCapacity`/`TotalkWCapacity`, `:81-82`/`:991-992`) is a **dead
store** — a whole-tree grep returns exactly six lines (two declarations, the two
property arms, two dead `RecalcElementData` calls `:1107-1108`) and **nothing
reads those fields**, dss_capi 0.14.5 having commented them out outright. It is
the `VSConverter.GetCurrents` hazard in miniature and is **not reproduced**: the
getters re-sum the fleet from scratch on every call, so the store feeds no
render either (measured — a bare `edit storage.sa kwhrated=9999 kwrated=2222`
moves the render to `'15999'`/`'3722'`/`'14799'` with no fleet rebuild). The
capi `''` was re-confirmed on both of its surfaces (`? name.prop` and
`Properties(p).Val` are the same `GetObjPropertyValue` path): 0.14.5 flags the
five `[SilentReadOnly, ReadByFunction]`
(`src/PCElements/IndMach012.pas:288-289` read fn `:264-267`;
`src/Controls/StorageController.pas:416-423` read fns `:309-338`) and never
assigns their `PropertyOffset`, so `DSSObjectHelper.pas:2189` exits at its
`PropertyOffset[Index] <> -1` guard (`:2203-2204`) with the string still empty —
on a solved circuit as much as an unsolved one. **The exposure list needed three
corrections**, all measured by walking every manifest case's full
`Redirect`/`Compile` closure: `Test/indmachtest/Master.DSS` and
`4wire-Delta/Kersting4wire_{Lagging,Leading}.dss` hold **no** IndMach012 (their
only hit is `UserModel=IndMach012a` on a **Generator** — a DLL name matched as a
substring); `StoCtrl_SeasonTarget/IEEE13NodecktMOD.dss` holds no
StorageController (the controller comes from include fragments of
`Run_example.dss`); and three cases the brief missed do hold one
(`controls/combo/midi_controls.dss`, `modes/makeposseq/makeposseq_ctrl.dss`,
`controls/relay/relay_generic.dss`). Final exposure: **22 StorageController +
6 IndMach012 cases**.

**The engine change is one new flag consulted at one site.**
`PropFlags::RENDERS_LIVE_RESULT` (bit 19) is carried **alongside**
`SILENT_READ_ONLY` on exactly those five PropDefs, and
`obj/props/class_props/value.rs` is the only reader of the pair
(`SILENT_READ_ONLY && !RENDERS_LIVE_RESULT → ""`). The three other
`SILENT_READ_ONLY` readers — the JSON export omission (`class_props/json.rs`),
the JSON set refusal (`json_set.rs`) and the schema `readOnly`
(`report/export/json/schema/classes.rs`) — are **byte-unmoved**, which is why no
JSON or schema golden can move by accident, and `golden_json` 117 /
`golden_schema` 104 / `golden_lock` 4 green in both lanes are the proof rather
than the argument. The `&self` getter cannot reach the solution or the Storage
arena, so the same flag does the second job: it marks the property for a refresh
at the existing choke point `Dss::refresh_vterminal_if_marked` (the
`READS_VTERMINAL` precedent, which `prop_flags.rs` had already anticipated for
exactly this sub-step), reached by all three per-read render surfaces — and,
since the settlement below, by `Save` too, which renders whole classes and so
refreshes the store once up front instead
(`Dss::refresh_render_caches_for_save`). **StorageController**
caches a `FleetAggregates` refreshed by a loop-for-loop port of the four
getters, reading four plain numbers per fleet member and writing nothing —
`fleet_aggregates_are_a_pure_read` pins that a read leaves every rendered
property of the controller and of both members identical. **IndMach012** caches
`live_pf`, refreshed on the **fresh** `refresh_iterminal` path (GOLDEN_REBASE
G2.3's choice, deliberately made rather than defaulted; for this class the two
paths coincide, because `GetTerminalCurrents` carries its own `SolutionCount`
guard). **The one real finding of the sub-step lives there:** the naive
in-place recompute on an *unstamped* `Iterminal` cache runs `CalcPFlow` and
**advances the slip-Newton** — a JSON export moved the model, and it broke a
committed golden (`json/spectrum_refs.json`, `Slip` `0.007` →
`0.006947528894572309`). r4133 has the same stateful recompute inside
`ComputeIterminal` and **keeps** the advance: reading `pf` there moves the
machine, the `VSConverter.GetCurrents` family again, so under the 2026-08-02
policy it is not reproduced. Skipping the recompute is not the fix either (it is
how both engines get the number); it now runs on a throwaway `self.clone()` and
only the resulting f64 is kept — bit-identical output, discarded state. Proven
non-vacuous: with the clone removed, `pf_is_a_pure_read` fails
(`0.02` → `0.0162762199608059`) and `golden_json` fails on `Slip`.
**Precision is full precision, not r4133's `%.6g`/`%-.8g`** — the plan's
shorthand, overridden by measurement: every other double in the port renders
through `float_to_str_ex`, emitting a Delphi width from these five alone would
put a lossy string on `Dump`/`Save`/export where every sibling is exact, and the
r4133 channel absorbs the digit difference through RP2.4's measured
`R4133_DISPLAY_FLOOR = 2e-4` + `display_is_render` (each r4133 byte **is** the
port's number rounded). The gaps are ≈5e-10 rel on the aggregates and 4.3e-7 on
`pf`; the live census below measured the worst gated cell at **4.029e-08**, four
orders under the floor. The pins carry r4133's own bytes through `fmt_g(v,6)` /
`fmt_g(v,8)`, so a precision regression is still caught.

**The capi side, measured before it was excluded: 24 cases / 84 distinct
(case, element, property) cells / 1 006 (cell × step) comparisons**, every one a
`value_structure` divergence (a number vs `''`), identical in both lanes — 20
StorageController cases × 4 properties + 4 IndMach012 cases × `PF`; per-property
rows 250/250/250/250 + 6. Two silent holes are recorded rather than fixed:
`StoCtrl_Current_PeakShave/master.dss` holds a controller but is `kind: large`,
so the scheduler never property-compares it at all (which is why the exposure
list has 22 SC cases and only 20 red), and the three `r4133`-only cases have no
property compare until RP4.1 (*they gained one on 2026-09-03 — §RP4.1; the
`SKIP_PROPS` group (g) rows keep masking the five properties on both channels*). The disposition is a `SKIP_PROPS` row group **(g)**
— `("IndMach012","PF")` + the four `("StorageController", …)` — mirrored in
`SKIP_PROPS_CAPI_ONLY` so the two lists still **partition** `SKIP_PROPS`
(12 → 17 and 5 → 10 rows; `SKIP_PROPS_BOTH_CHANNELS` unchanged at 7), per
`(class, property)` rather than per case, cited to the 0.14.5 mechanism and to
the r4133 authority arms, with the whole argument written out in
`tests/TOLERANCE_NOTES.md` §"WP8.5b property parity" (+34 lines): the cell is a
**structure** difference, non-comparable by construction and **never** a
tolerance question — no floor is involved and none moved. No `ledger.json` entry
was needed or written (the divergence is mechanical and case-independent; 24
entries would say one thing), and `ledger.json`, `population.lock.json` and the
frozen `tests/corpus/props_r4133/**` extracts are byte-untouched.

**Zero golden bytes moved, and that was proven by regenerating.** The 21 props
cells that would move if the artifact were a capture of *this* engine
(`props/storagecontroller.json` 16 × `''`→`'0'`, `props/indmach012.json` 5 ×
`''`→`'1'`, both r4133's own bytes) are cells of a **0.14.5 capture**, which
answers `''` forever — so a regen moves nothing, measured by importing
`tools/golden/gen_props.py` itself and re-running its `check_pin()` +
`run_scenario()` per class into a scratch directory: both class files came back
**byte-identical** to the committed artifacts. (A *full* `gen_props.py` run was
deliberately not used: it rewrites all 51 class files including
`props/{recloser,relay}.json`, whose committed values are the port's renders.)
The disposition is therefore the exclusion register `props_roundtrip.rs` already
uses for this exact shape — `LANE_SKIP_SCENARIO_PROPS` **2 → 23 rows**, with
`PROPS_CLASS_FILES` 51 / `PROPS_SCENARIOS` 322 / `PROPS_PROPERTY_CELLS` 8 343
unchanged (`compared` 8 341 → 8 320 is an asserted equality, so the green run is
the proof). **The alternative was considered and is reversible**: overlaying
r4133's `'0'`/`'1'` into the two captures (RP3.7's `CAPI015_OVERLAYS`
precedent) would keep the 21 cells compared, but `golden_lock` asserts an
overlay entry *is* a capi015 artifact and these two are ordinary `capi_v0145`
rows, so it needs a new register — a provenance-model change no plan text
authorizes — for constants (`0` on an empty fleet, `1` on an unpowered machine)
three unit pins already assert against r4133's transcripts. Nothing else moves:
no committed `Dump`/`Show`/`Save` byte holds either class (`~ PF=` appears only
in `dump3_{bare,debug}.txt` and `dump_upfc.txt`, none of which builds one), and
`Save` walks only explicitly-set properties.

**Pins 40 → 43, and the replay gains a third accounting state.**
`props_r4133_pins.rs` adds `indmach012_pf_renders_the_live_power_factor`
(r4133's `'0.909167'`/`'0.904914'` after compile, the full-precision render, and
the gate's own step-0 cell), `storagecontroller_fleet_aggregates_render_the_live_fleet`
(`12000/3000/9600/0` after compile *and* after the solve; r4133's live-edit
bytes `15999/3722/14799` — re-summed, never latched; the 7-member
`7350/1550` fleet with `fmt_g(v,8)` `'2627.3806'`→`'2627.3929'` and
`'-18.81068'`) and `the_silent_readonly_capture_cells_are_empty`, which walks
all 21 capture cells asserting `''` with a writable sibling per class so it
cannot pass vacuously — the witness that the 0.14.5 oracle really is what the
skip rows exclude, since no oracle runs inside a pin binary. Two pin numbers
came out different from the pre-implementation predictions and both are
**schedule** facts, not disagreements: the `14799` live-edit reading reproduces
only when the edit follows exactly **one** solve (r4133's own schedule; with two
the fleet has charged and the port answers `15068.9999996377`), and P0's
per-step `pf` bytes are read-sequence-dependent because the *variables* read
perturbs — so each pin takes its expected value from the schedule it pins. In
the replay, the five pairs are **superseded**, not claimed and not declared: the
frozen example rows record `rust = ''` by capture and cannot be re-frozen, so
feeding a counterfactual spelling to the policy chain would prove nothing and
writing the port's new spelling into that column would be a fabricated
measurement. `DECLARED_RP38 (181, 5, 181) → (0, 0, 0)`, its old value **moved**
into the new `SUPERSEDED_RP38 = (181, 5, 181)`, `RP38_ROUTING` became
`RP38_SUPERSEDED` (now carrying the r4133 arm, the measured live disposition and
the pin per pair) plus `RP38_CAPTURE_PIN`, and the totality assert is now
`claimed + declared + superseded == rows`, with the interception **after** the
chain so a link that ever claimed one of these rows would still be credited.
Three tripwires stand against the failure mode that matters (revert the engine
to `''` and the capi compare agrees again while the skip rows mask nothing):
`the_rp38_pairs_are_superseded_by_the_live_render` asks the **shipped** harness
for each pair (`skip_prop` true on capi, false on r4133), the pins assert
literal r4133 bytes, and `every_echo_row_pin_is_a_test_that_exists` reads the
pin columns both ways.

**The r4133 side stays compared, and that too was measured, not assumed** —
with §1.1(e)'s mask bypassed (`DSS_PROPS_CENSUS=claims`, 27 cases: every case in
the population holding either class): **105 divergent cells, 89 in scope, 103
claimed by RP2.4's display floor** (worst rel **4.029e-08**) **and 2 out of
scope**. `indmach012.pf` produces **zero** divergent cells and structurally
cannot: a power factor is bounded by 1, so r4133's `%.6g` is at most 5e-07
absolute from ours while the property compare's floor is `tol.i_abs = 1e-6` at
every tier — the cell matches before any display floor is consulted (verified
non-vacuous: `Capacitor.cuf` on the same case, gap 3.4e-3, *is* reported).
`kwhtotal` is zero too. **The 2 unclaimed cells are RP4.1's inheritance, already
root-caused** (*disposed 2026-09-03: `makeposseq_ctrl.dss` stays
`engines: capi_v0145` per coordinator decision 5, so no entry was landed and the
two cells are accounted out of scope in the §RP4.1 record's per-owner table*): on `modes:makeposseq/makeposseq_ctrl.dss` the port answers
`kWTotal` `33.3333333333333` and `kWActual` `-0.333333333333333` where r4133
says `100` / `-1` — exactly a factor of `Fnphases` — because r4133's
`TStorageObj.MakePosSequence` emits `' kWrating=%-.5g'`
(`PCElements/Storage.pas:3979-3985`) where the class's property is `kWrated`
(`:647`), so that half of its own edit is an unknown parameter and the rating is
never scaled; dss_capi 0.14.5 fixed the same path by ordinal (`:3340`/`:3349`)
and the port follows it. The case is `capi_v0145`-only, so the r4133 channel
does not gate it today — it is an upstream bug newly *observable* only because
the aggregates now render, and if its `engines` key ever changes it needs a
cited exclusion + pin.

*Gate.* All five commands green in both lanes, each exit code read individually:
**4 271 passed / 0 failed / 5 ignored per lane**, the two totals identical and
the five `ignored` the pre-existing ones (no `#[ignore]` and no name filter was
added). `corpus_gate` **131** over the full 523-case population in both lanes
(146.7 s / 134.9 s) with every ledger entry hit and none stale — the 24 predicted
capi cases all green under the new rows, **zero** r4133-channel reds —
`golden_lock` 4, `golden_schema` 104, `golden_json` 117 (so `spectrum_refs.json`
still holds `Slip = 0.007`), `golden_json_import` 107, `golden_reports` 305,
`props_roundtrip` 1, `props_r4133_evidence_lock` 11, `props_r4133_replay` 132,
`props_r4133_pins` 43, `oracle_parity_cfg_gate` 10, `dss-core --lib` 1 480
(+14). No fmt fix, no clippy fix, no test edit, no count lock moved, no
tolerance touched, no golden re-baselined — the tree that arrived is the tree
that passed. `lane_diff` was re-run because the render path now refreshes caches
and clones an element: **PASS**, 523 cases / 3 220 861 records / ~4.83 M compared
values, **`max |Δ| = 0.000e0` and `max rel = 0.000e0` on all eight gated kinds**
(conv 2 162, cur 1 170 100, errs 519, iter 2 162, loss 366 476, pow 1 170 100,
v 375 816, y 1 738 084, every one "(identical)"), 0 iteration counts drifted — so
the 2026-07-31 bit-identical baseline holds exactly and the default lane keeps
the parity lane's oracle standing. One hidden red was found and fixed on the
way: `oracle_parity_cfg_gate::teardown_markers_and_the_register_agree` walks the
**whole repository** and reserves the WP-G2 teardown marker spellings for rows
of that register (decrements of the `SPLIT_ALIAS_POPULATION` /
`Escape::WholeCase` censuses), which RP3.8 does not produce, so its nine markers
were re-spelled `RP3.8 EXPECTED-VALUE PIN [row]` / `RP3.8 LANE EXCLUSION [row]`
(the settlement below gave that loose vocabulary its own enforcement, so it can
no longer shadow the register); the same walk's `SKIP_DIRS` gained `tmp` (the
gitignored scratch root — a throwaway `.rs` there could red the mandatory gate,
and one did), non-vacuous both ways and, since the settlement, **anchored at the
repository root** exactly like `.gitignore`'s `/tmp`, so a future
`crates/…/src/tmp/` cannot leave the walk.

**Recorded, not chased.** (a) `batchedit '… where <prop> > x'`
(`exec/batchedit.rs:259-270`) is a `&self` `get_value` reader, so it sees the
render cache instead of the live value; unexercised by any golden and by all 523
cases (re-confirmed by the census), and widening the refresh needs `&mut self`
plumbing outside this sub-step → `ORPHANED_GAPS.md` §1.17. (The inventory this
item stood on was **wrong by one** — `Save` is a fifth reader, and the audit
settlement below fixes it; §1.17 now records five readers, four refreshed.) (b)
`Dss::element_variables` still perturbs: state variable #21 runs
`terminal_power` on `self`, so reading an IndMach012's variables on an unstamped
cache advances the slip-Newton — pre-existing, mirroring r4133's `Get_Variable`
→ `Get_Power`, and it is what produced the P0 probe's own `0.908755` trajectory
→ `ORPHANED_GAPS.md` §1.18. (c) r4133's `? pf` keeps that advance where the port
does not; under the corpus gate's schedule this is invisible (every property
read follows a solve, both caches stamped, neither engine recomputes — proven by
probe), and it becomes visible only for a schedule that reads a property
**before** a solve and then solves. If RP4.1 introduces a pre-solve property
compare on an IndMach012 case, that is a cited exclusion + pin, never a "fix"
that reproduces the mutation. (d) `refresh_vterminal_if_marked` now does four
jobs; its doc enumerates all four and it was deliberately not renamed (three
call sites, cosmetic). (e) The golden disposition above is reversible, and the
new capture pin must move with it if a later WP overlays the two artifacts.
**Two upstream reports were written** (English, in the gitignored
`investigations/to_opendss/`, both re-measured first-hand on the DLL):
**48** — `? IndMach012.<n>.PF` before `NodeRef` is assigned **access-violates**
inside `GetCurrents` (`Get_Power` guards only on `FEnabled`,
`CktElement.pas:679`; DSS error #641, "Read of address 0000000000000000",
reproducible in a fresh process on all five golden scenarios, with three
siblings reaching `ComputeIterminal` the same way); the port renders `1` there,
safely. **49** — the `MakePosSequence` `kWrating=`/`kWrated` bug above
(`DSS error #560: Unknown parameter "kWrating"`). A landed pin doc had claimed
the port answers `1` after a solve on an *unenergized* bus; re-measured, that is
false — both engines' machine state goes NaN there (r4133 `? slip` = `'NAN'`,
the port's terminal powers NaN) and the port renders `----`, `float_to_str_ex`'s
NaN spelling. That is a render convention over identical state, not a divergence
and not a pinned value; the doc now says so.

**RP3.8 audit settlement (2026-09-02) — 9 raw findings → 8 after dedup (the
escaped-`\n` artifact was raised by both lenses); one major, seven fixed, one
recorded, none dropped; `FIX` in both lanes.** Every claim was re-derived here
before it was acted on: both probe legs replayed on the vendored EPRI DLL
(11.0.0.1) through `epri-worker`, four mutations run and restored, and the
r4133 bytes measured rather than transcribed.

*The major one — `Save` is a **fifth** `ClassProps::get_value` reader, and it
rendered a cache nobody refreshed.* The sub-step's own inventory said four
(`?`, `Dump`, `element_properties` refreshing at the choke point;
`batchedit … where` not). `Save` (`report/save/save.rs::save_write_token`, cited
by name because RP3.11's guards moved the line, reached from
`exec/save_circuit.rs` and `exec/report.rs::write_class_file`) is the fifth, and
it emits every property a deck explicitly **set** — and a write to one of these
read-only properties is silently ignored *yet still marks the property set*, so
the serializer really did reach the render caches. Measured, same decks, port vs
r4133: a deck writing `pf=0.5` saved `PF=1` here and `pf=0.886059` upstream —
and `PF=0.886059022116548` here if a `?` happened to precede the save, i.e. an
output that depended on the session's **read history**, which is the exact
contamination shape the corpus gate's three-run artifact exists to forbid; a
deck writing `kWhTotal=42` saved `kWhTotal=0` here and `kWhTotal=6000` upstream.
The same latency was then found on the older `READS_VTERMINAL` marker, where it
**pre-dates RP3.8**: a deck writing `wdgcurrents=` saved an all-zero buffer here
while r4133 saves the solved currents. Fixed once, uniformly:
`Dss::refresh_render_caches_for_save` runs the existing per-object choke point
over the store before the serializer renders (`Save circuit`) or over one class
(`Save <class>`), so all four of its jobs happen and nothing but a marked
property's cache moves. Pinned by
`exec::tests::report::save_renders_the_live_result_properties` — five legs
carrying r4133's own bytes, including the cold/warm equality that is the
read-history regression's tripwire — proven non-vacuous by reverting each call
site. **No committed byte moved:** no corpus deck and no golden writes any of the
six properties (`wdgcurrents *=` has zero hits in the whole corpus), and
`save_roundtrip` / `golden_reports` / `golden_json` are green. `ORPHANED_GAPS.md`
§1.17 now records five readers, four refreshed, with the `batchedit` filter as
the one that stays.

*The other six fixes.* **(2) The flag's holder set is now tied to the dispatch.**
`refresh_live_result_cache` is a hardcoded two-arm `if`, so a third class given
`RENDERS_LIVE_RESULT` would have rendered a stale cache silently;
`exec::tests::report::renders_live_result_holders_have_a_refresh_arm` walks the
registry the executive actually builds and pins the holder set, and the function
now ends in a `debug_assert!` that fires on a holder reaching no arm — mutation:
giving `Transformer.WdgCurrents` the flag reds both. **(3) The two "ignored by a
JSON load" tests now test the flag they cite.** Both passed with
`SILENT_READ_ONLY` dropped, because these five have no `set_f64` arm and
"nothing was stored" holds either way; each now also asserts the property is not
**stamped** into `PrpSequence` (which is what `edit_property` would do without
the flag — and a stamped property is one `Save` emits, the same finding as the
major one seen from the load side). Mutation: dropping the flag now reds both.
**(4) The sub-step marker vocabulary is enforced.**
`oracle_parity_cfg_gate::substep_markers_are_tagged_and_do_not_shadow_the_register`
requires every loose marker to carry its owning `RP<n>.<n>` tag, forbids it from
naming a row that IS in `TORN_DOWN_ROWS` (the copy-paste hazard: such a row must
wear the reserved spelling the register cross-checks), and requires a pin marker
to sit above a real `#[test]` — non-vacuous on today's nine, and mutation-checked
both ways. **(5) The walk's `tmp` skip is anchored at the repository root**, like
`.gitignore`'s `/tmp`, instead of matching any directory named `tmp` at any
depth: the old form rested on a hand-verified premise and would have let a future
`crates/…/src/tmp/` leave the whole-repository walk. **(6) `write_gate_dump`'s
justification is corrected.** Its claim that the strip "costs the artifact
nothing" is false on the r4133 channel, where the gate DOES value-compare the
(e)/(f)/(g) rows the channel-blind `skip_prop_ub` nulls; the doc now states the
loss precisely — a real contamination still fails the artifact through the
`verdict` string, what is lost is the narrower within-tolerance drift signal for
those eight oracle-side renders — and names the channel-aware strip as the
alternative. **(7) Two record defects:** the plan's "Six points" listed seven,
and the pin doc understated its own literal — re-measured, r4133 read **without**
its perturbing pre-read answers `'0.908916'` on that deck, exactly
`fmt_g(0.908915557341299, 6)`, so the step-0 cell is r4133-**corroborated**; only
r4133's own mutation separates the two schedules. (The escaped `\n` in
`props_norm.rs`'s assert message — the finding both lenses raised — is gone.)

*Recorded, not fixed.* **`SKIP_PROPS` has no liveness guard** (pre-existing, all
17 rows): the disposition tests prove every row is decided and that the two
channel lists partition it, but a row that has stopped masking anything is
accepted silently — and for group (g) the failure mode is the engine reverting to
`''`, after which the capi compare AGREES. What covers it today is out of
harness: the expected-value pins and the r4133 channel, each of which reds on
exactly that revert (re-verified by mutation). A per-row mask counter threaded
through the corpus gate would close it in-harness; that is harness work no
sub-step owns, and it is recorded at the `SKIP_PROPS` declaration itself.

*Gate.* All five commands green in both lanes, each exit code read individually:
**4 274 passed / 0 failed / 5 ignored per lane**, the two totals identical and
the five `ignored` the same pre-existing ones (+3 on the sub-step's 4 271 — the
two new engine-side tests and the marker check; no `#[ignore]`, no name filter).
`corpus_gate` **131** over the full 523-case population in both lanes
(138.9 s / 148.4 s) with every ledger entry hit and none stale;
`oracle_parity_cfg_gate` **11** (+1), `dss-core --lib` **1 482** (+2), and
`golden_lock` 4 / `golden_schema` 104 / `golden_json` 117 / `golden_reports` 305
/ `props_roundtrip` 1 / `props_r4133_pins` 43 / `props_r4133_replay` 132 /
`props_r4133_evidence_lock` 11 all unchanged — no golden re-baselined, no
tolerance touched, no count lock moved. `lane_diff` was re-run because the fix
adds a refresh pass on a new surface: **PASS**, 523 cases / 3 220 861 records,
**`max |Δ| = 0.000e0` and `max rel = 0.000e0` on all eight gated kinds** (conv
2 162, cur 1 170 100, errs 519, iter 2 162, loss 366 476, pow 1 170 100, v
375 816, y 1 738 084, every one "(identical)"), 0 iteration counts drifted — the
bit-identical baseline is exactly where RP3.8 left it.

**RP3.9 (the r4133 round-trip residue) landed 2026-09-02 — `PRECISION_ROUNDTRIP`
on all 27 pairs, in one commit, with zero product-crate lines, zero golden bytes
and zero census cells moved.** RP2.4's display floor
(`harness/props_norm.rs`, `R4133_DISPLAY_FLOOR = 2e-4`, `display_is_render`)
refuses **55 vendored spellings / 27 pairs / 70 live cells** because the two
engines hold different doubles rather than one being a rounded render of the
other. Every one of the 27 pairs is now settled by a cited Pascal round-trip
chain and held by an expected-value pin; **none** is a `PORT_BUG`, an
`UPSTREAM_BUG`, a `STATE_DIFFERS` or a `KILL`, so no engine line, no
`TODO(compat)`, no `ledger.json` row and no upstream report is owed by the
sub-step. The sub-step's own commit is three test/evidence files
(`props_r4133_pins.rs` +1189/−1, `props_r4133_replay.rs` +517/−23,
`tests/corpus/props_r4133/README.md` +64), nothing under any crate's `src/`;
the audit settlement below touched the same three and this record, again with
no product-crate line.

**One mechanism explains all 70 cells, and it was measured before it was
written.** Every cell sits on one of the six `modes:makeposseq/makeposseq_*.dss`
decks and is read **after** `MakePosSequence`, where r4133 converts an element
by building a command string (`Format('%-.5g'/'%-.8g', …)`) and re-parsing it
through its own `Edit`, while the port applies typed setters
(`class_props/typed.rs`, WPG.21). The five-digit round trip therefore happens
*upstream* of the getter, and the getter then derives at full precision —
`Load.pas:2326 → :2331-2332 → :1145 → :2352` (`kW/3` and `kvar/3` re-parsed,
then `kVA` recomputed from the rounded **pair**); `Vsource.pas:1397 → :360 →
:473`, where the rounded input is **`BasekV`**, not Z (`R1`/`X1` re-parse
exactly on all six decks), then `:752`/`:766-768`/`:837-842` for
`puz*`/`mvasc*`/`isc3`; `Line.pas:1585-1591 → :1597-1598 → :611 → :1406/:1407`
(`b0`/`b1` from a round-tripped `C`); `Reactor.pas:1158-1162 → :1200-1201 →
:657`/`:666`/`:670-675` → `:1092-1100`; `Capacitor.pas:798-832 → :606/:645-646 →
:1098-1109` (with `Common/Utilities.pas:2600-2607`);
`generator.pas:3054-3066 → :641/:699/:734 → :3018-3021/:3130-3137`;
`Transformer.pas:1982-1994 → :512 → :1119-1130 → :1842-1843`;
`AutoTrans.pas:2021/:2027/:2030 → :1863 → :1662-1690`. The rawest reading is
`load.kw`: the port answers `(400/3)/3` where r4133 answers `44.443` =
`%-.5g(%-.5g(400/3)/3)` — `133.33` re-parsed, divided again and re-rounded —
because `makeposseq_pc.dss` runs `makeposseq` **twice**, which is why no single
`%.Ng` render explains the gap and why the floor was right to refuse it.

**The per-pair verdicts** (`props_r4133_replay.rs`: `RP39_ROUTING` :294 rows,
`RP39_PINS` :4610 verdicts; the table is machine-extracted from those two
consts, not transcribed). Chains: **A** `load.*`, **B** `vsource.*`, **C**
`line.b*` + `autotrans.wdgcurrents`, **D** `reactor.*`/`capacitor.*`, **E**
`generator.*`/`transformer.*`. All 27 verdicts are `PRECISION_ROUNDTRIP`, all
27 dispositions `PIN`.

| pair | rows | in scope | pin (chain) |
|---|---|---|---|
| `load.kva` | 13 | 0 | `load_kva_after_makeposseq_is_the_exact_typed_conversion` (A) |
| `load.kw` | 2 | 0 | `load_kw_kvar_and_xfkva_after_makeposseq_are_the_exact_typed_conversion` (A) |
| `load.kvar` | 1 | 0 | `load_kw_kvar_and_xfkva_…` (A) |
| `load.xfkva` | 1 | 0 | `load_kw_kvar_and_xfkva_…` (A) |
| `vsource.puz0` | 4 | 0 | `vsource_isc3_and_puz_after_makeposseq_use_the_full_precision_basekv` (B) |
| `vsource.puz1` | 4 | 0 | `vsource_isc3_and_puz_…` (B) |
| `vsource.puz2` | 4 | 0 | `vsource_isc3_and_puz_…` (B) |
| `vsource.isc3` | 3 | 3 | `vsource_isc3_and_puz_…` (B) |
| `vsource.mvasc1` | 2 | 2 | `vsource_mvasc1_and_mvasc3_after_makeposseq_use_the_full_precision_basekv` (B) |
| `vsource.mvasc3` | 2 | 2 | `vsource_mvasc1_and_mvasc3_…` (B) |
| `line.b1` | 2 | 2 | `line_b1_and_b0_after_makeposseq_use_the_full_precision_c1` (C) |
| `line.b0` | 2 | 2 | `line_b1_and_b0_…` (C) |
| `autotrans.wdgcurrents` | 1 | 0 | `autotrans_wdgcurrents_after_makeposseq_solve_the_exactly_converted_circuit` (C) |
| `reactor.normamps` | 1 | 0 | `reactor_amps_after_makeposseq_are_the_exact_typed_conversion` (D) |
| `reactor.emergamps` | 1 | 0 | `reactor_amps_…` (D) |
| `reactor.lmh` | 1 | 1 | `reactor_amps_…` (D) |
| `reactor.x` | 1 | 1 | `reactor_amps_…` (D) |
| `reactor.z` | 1 | 1 | `reactor_amps_…` (D) |
| `capacitor.cuf` | 1 | 0 | `capacitor_cuf_and_amps_after_makeposseq_are_the_exact_typed_conversion` (D) |
| `capacitor.normamps` | 1 | 0 | `capacitor_cuf_and_amps_…` (D) |
| `capacitor.emergamps` | 1 | 0 | `capacitor_cuf_and_amps_…` (D) |
| `generator.kva` | 1 | 1 | `generator_ratings_after_makeposseq_are_the_exact_typed_conversion` (E) |
| `generator.kvar` | 1 | 0 | `generator_ratings_…` (E) |
| `generator.maxkvar` | 1 | 1 | `generator_ratings_…` (E) |
| `generator.minkvar` | 1 | 1 | `generator_ratings_…` (E) |
| `transformer.normamps` | 1 | 1 | `transformer_amps_after_makeposseq_are_the_exact_typed_conversion` (E) |
| `transformer.emergamps` | 1 | 1 | `transformer_amps_…` (E) |

**Ten pins (`props_r4133_pins.rs:2193-3402`) cover all 27 pairs, and each
asserts the chain rather than narrating it:** (a) the port's literal render off
`? Class.Name.Prop`; (b) r4133's census literal **recomputed from the port's own
number by the chain's arithmetic inside the test**; (c) a discriminating second
reading — for chains D/E the port is fed r4133's own five-digit token through
`edit` and then prints r4133's literal itself, after which a third reading
restores the exact nameplate and moves the cells back, with `? kv`/`? kW`/
`? kVs` read each time so the edit cannot pass vacuously; (d)
`Version8/Source/*.pas:` cites in the doc comment. The AutoTrans pin hard-codes
none of r4133's command strings: it **computes** every `%-.5g` token from the
port's own post-conversion doubles, replays them as `edit`s, solves twice, and
reproduces r4133's census byte `44.00054, (161.26), 29.32187, (161.26), `
exactly, with the AutoTrans-only replay (`44.00086, …`) asserted as the
discriminator that the chain is deck-wide. Seven local helpers were added —
chiefly `round_g`/`text_g` (FPC `Format('%-.Ng')` as a value and as text, both
through the engine's own `dss_core::util::fmt_g`, the F-FMT seam, which is
literally what `Parser.CmdString := S; Edit` does) and `render`
(= `float_to_str_ex`, the function the `?` getter itself calls).

**A lane trap was found and closed rather than papered over.** The parity lane's
`fmt_g_fpc_impl` and the default lane's `fmt_g_native_impl` spell the *same
stored double* differently on `Load.ld_wye.kW`: `44.4444444444445` vs
`44.4444444444444`. The state is `(400/3)/3 = 44.444444444444446`, which is
**not** `400/9 = 44.44444444444444`, and the FPC kernel distinguishes them at 15
digits where the native one does not. The pin asserts that byte as a **value**
(`render(400.0/3.0/3.0)`) plus a second assertion naming both lane spellings —
**no `cfg`, no tolerance, no `#[ignore]`**; every other repeating-decimal
literal in chains D/E was then checked in both lanes and none differs.

**The accounting shrinks where a measurement allows it and nowhere else.**
`RP39_ROUTING` (`props_r4133_replay.rs:294`) gained a fifth column, the per-pair
**disposition** (all 27 = `PIN`), against the documented set
`RP39_DISPOSITIONS = ["PIN", "FIX", "LEDGER", "OPEN"]` (:703 — `KILL`
deliberately has **no** tag: a killed pair leaves the sub-step and is reported,
not recorded); `RP39_PINS` (:4610) carries 27 rows of `(pair, pin, verdict)`,
all `PRECISION_ROUNDTRIP`, read by `every_echo_row_pin_is_a_test_that_exists`
as a fourth cited set; `the_rp39_pin_list_is_pinned` (:4765) now checks
completeness **both** ways (every pin's pair is `PIN`-disposed, every
`PIN`-disposed pair names a pin, any other disposition names none); and
`the_display_floors_round_trip_residue_is_owned_by_rp39` (:8595) additionally
rejects an unknown disposition tag by name. The new `OPEN_RP39` (:683) is the
shrink: **`(55, 27, 19) → (0, 0, 0)`**. **`DECLARED_RP39` stays `(55, 27, 19)`
on purpose:** it is not a declaration but a *measurement* — `account()` walks
the vendored corpus and buckets whatever `display_class_but_not_a_render` routes
to `Owner::Rp39`, a property of the two engines' doubles. No port render changed
(all 27 verdicts are "the port is exact"), so writing a smaller number would
either fail the guard's own equality or force it to be loosened — exactly the
silent-progress claim the accounting exists to prevent. The rows retire by hand
at RP4.1, as `DECLARED_RP3`'s and `DECLARED_RP35`'s notes already spell out for
the earlier settled sub-steps.

**The census was re-measured after the work, and every bucket is unchanged**
(full walk, both channels, no `DSS_GATE_ONLY`: 440 cases / 1 059 178 rows /
63.7 s; 6 r4133 + 22 capi oracle errors, as before): r4133 `UNCLAIMED`
534 cells / 30 in scope / 292 spellings / 49 pairs, `under-floor`
49 484 / 46 627 / 2 045 / 71, `echo-row` 488 019 / 468 046 / 170 / 82,
`ledger-hit` 0, `mixed_disposition_spellings` 0; capi `ledger-hit` 21 (9 / 15 /
12), `UNCLAIMED` 177 (141 / 29 / 11), **0 cells on every r4133 disposition** —
every figure equal to the pre-work run, delta **0** in every bucket. All 27
RP3.9 pairs appear in `claims_unclaimed_pairs.txt` with `cells_in_scope = 0` and
cell/spelling counts equal line for line. A zero delta is the correct outcome —
a pin does not make a `Link` claim a row — and a non-zero one would have meant a
pin moved a render. **No `property` ledger entry is staged**, because with
`count_in_scope = 0` on all 70 cells an r4133 entry would have nothing to
exclude; the drafts, should a deck's `engines` key ever change, are staged in
`tests/corpus/props_r4133/README.md` (the audit settlement moved them there
out of the gitignored dossiers, where the forward reference would have dangled). One structural note: the census has **no disposition
meaning "settled by an RP3.9 pin"** — the 70 cells still file as `UNCLAIMED`.
That costs nothing today and is RP4.1's to decide, not this sub-step's to
invent. *Decided 2026-09-03 (RP4.1):* no census disposition was added —
acceptance is read as zero UNCLAIMED cells *in scope*, and these 70 cells are
accounted out of scope, owner by owner, in the §RP4.1 record below.

*Gate.* All five commands green in both lanes, each exit code read individually:
**4 285 passed / 0 failed / 5 ignored per lane** over 74 test binaries, the two
totals identical binary for binary and the five `ignored` the same pre-existing
ones (+11 on RP3.8's settled 4 274; no `#[ignore]`, no name filter).
`corpus_gate` **131** over the full 523-case population in both lanes
(142.9 s / 139.7 s) with every ledger entry hit and none stale, zero
NEVER-APPLIED entries and zero reds on either channel; `props_r4133_pins`
43 → **53**, `props_r4133_replay` 132 → **133**, and `props_r4133_evidence_lock`
11 / `oracle_parity_cfg_gate` 11 / `dss-core --lib` 1 482 / `golden_lock` 4 /
`golden_schema` 104 / `golden_json` 117 / `golden_reports` 305 /
`props_roundtrip` 1 all unchanged — no golden re-baselined, no tolerance
consulted or moved, no count lock moved other than the new `OPEN_RP39`, no
`ledger.json` edit and no `TODO(compat)` added.
`tests/corpus/props_r4133/README.md` gained a dated supplement
`### What RP3.9 settled (2026-09-02)` (:871, before `## Files`, on RP3.8's
precedent) and **no recorded number in any earlier section was rewritten**.
`lane_diff` was **not** re-run and does not need to be: the sub-step touched
three test/evidence files and not one line under any crate's `src/`, so no
engine path, `compat` kernel, lane alias or solver moved and the 2026-07-31
`max |Δ| = 0` bit-identical baseline — reproduced by RP3.8 the same day —
stands untouched.

**Recorded, not chased.** (a) r4133's `TStorageObj.MakePosSequence` emits
`' kWrating=%-.5g'` where its own property is `kWrated`
(`PCElements/Storage.pas:3979-3985` vs `:647`), so half of its own edit is DSS
error #560 on `makeposseq_pc.dss` — already reported as
`investigations/to_opendss/49` by RP3.8 and re-confirmed here; the case is
`capi_v0145`-only, so the r4133 channel does not gate it. (b) The **34**
`controls:autotrans/*` `wdgcurrents` cells (`autotrans_both`, `autotrans_reg`,
`midi_autotrans`, `midi_autotrans_both`; 3–7.5 % apart, e.g. `151.5029` vs
`156.2997` A on winding 2) involve **no** `makeposseq` and are a different
mechanism from RP3.9's single tiny `makeposseq_xfmr` cell — out of scope
(capi-only cases), filed `OutOfScope` by the RP2.4 re-filing, and still owed
their own root cause; **nobody owns them yet** and a multi-percent gap in a
solved current is not display-class. The audit round added a lead: those 34
cells are exactly the row count of `numeric_pairs.txt`'s
`autotrans.tap | '1.03125' | '1' | 34` and
`autotrans.taps | '[1, 1.03125, ]' | '[1, 1, ]' | 34`, i.e. the same four
regulator decks, and the divergence is confined to the series/common winding
while winding 1 agrees to 0.04 % — so a **RegControl tap** divergence is the
first thing its owner should read. It is a lead and not a settled cause: the
per-row current ratios are 1.032 / 1.056 / 1.069 / 1.082, not one uniform
1.03125. **Owned and settled 2026-09-03 by §RP3.12** (record below): the lead
was right — the cause is an r4133 `RegControl`-to-`TTransfObj` typecast,
`UPSTREAM_BUG`, never reproduced. (c) The post-`makeposseq` `Save`/`Dump`
surface belongs to **§RP3.11** (*settled 2026-09-03 — `KEEP_LIVE_PINNED`, so
the port keeps saving the live doubles; this is one instance of the recorded,
pinned divergence from r4133's serializer, and no channel compares it*): r4133
saves the five-digit tokens its getter hands back for those indices while the
port saves the exact doubles, so a saved-and-
reloaded converted circuit differs at ~5e-6 on exactly these elements. (d) The
`RP39_ROUTING` `cite` column was left byte-identical here and **corrected in
the audit settlement below** — its vsource rows named a round-tripped `Z`, its
transformer rows a round-tripped `kVA` and its `load.kva` row a round-tripped
`pf`, none of which is the rounded input this sub-step proved. (e)
`elements/pc/vsource/solve.rs:173` and `mod.rs:44-45` cite dss_capi 0.14.5 line
numbers rather than r4133's `Vsource.pas:1390` — product-doc cosmetics,
unacted.

**RP3.9 audit settlement (2026-09-03).** Two fresh auditors (`/audit-code`,
`/audit-tests`, both read-only, over `16edbc2f..648ce284`) confirmed all 27
verdicts independently on both live oracles — every r4133 literal is what the
r4133 DLL prints, every port literal what the pinned 0.14.5 backend prints, and
every chain reproduces arithmetically — and found **no port bug, no weakened
test and nothing lost against the plan**. Their fifteen findings (nine + six,
four of them the same item seen from both sides) were settled against the r4133
source and re-derivation, never against plausibility:

* **FIXED — 11 wrong r4133 line citations** (the one `[Major]`). The formulas
  named in the chain-C and chain-D pin docs are genuinely in r4133; the line
  numbers were not. Verified by dumping the r4133 tree: the `R1=… C1=%-.5g`
  `Format` is `Line.pas:1591` (`:1593` is a comment), `Parser.CmdString := S;
  Edit` `:1597-1598`, `c1 := Parser.Dblvalue*1.0e-9` `:611`, the symcomponents
  `C1_new := C1*1.0e9` branch `:1560-1563`; `Reactor.pas` `kvarPerPhase` `:657`,
  1-phase `PhasekV := kVRating` `:666`, `X` `:670`, `L` `:671`, `NormAmps`
  `:674`, `EmergAmps` `:675`, `CmdString`/`Edit` `:1200-1201`. Corrected in the
  pin docs and inline comments, in this record's mechanism paragraph and in the
  vendored README's chain table.
* **FIXED — three stale or false prose numbers.** The pin block header said
  "all thirteen pairs" (the chains-A-C draft state) where 27 landed; the
  vendored README's count-delta table gave `RP39_PINS` a "before" of "13 rows"
  for a constant that does not exist at `16edbc2f` (`git show
  16edbc2f:…/props_r4133_replay.rs | grep -c RP39_PINS` → 0), now "— (new
  constant)"; and this record's mechanism paragraph said the port answers
  `400/9` and spelled r4133's `44.443` as one `%-.5g`, contradicting its own
  lane-trap paragraph and the pin. The port's double is `(400/3)/3 =
  44.444444444444446` (≠ `400/9`), and r4133's number needs **both** `%-.5g`
  prints, which is what `props_r4133_pins.rs` asserts.
* **FIXED — `RP39_ROUTING`'s `cite` column stated causes this sub-step
  disproved.** Seven vsource rows blamed a round-tripped `Z` (the rounded input
  is `BasekV`; `R1`/`X1` re-parse exactly on all six decks, asserted in the
  pin), the two transformer rows a round-tripped `kVA` (it is the winding kV,
  `Transformer.pas:1982`), the `load.kva` row a round-tripped `pf` (it is the
  `kW`/`kvar` token pair, `:2326 → :1145`), and the four generator rows cited
  `MakePosSequence :3058`/`:3059`/`:3060` where the tokens are at
  `:3059`/`:3060`/`:3061` — and all four of those cells derive from the **kW**
  token, not from their own. All twelve rewritten; the guard only requires
  `.pas:`, so no count moved.
* **FIXED — the 16 vendored spellings that carried a verdict but no
  assertion.** 10 of the 13 `load.kva` spellings and 2 of the 4 spellings of
  each `vsource.puz0/1/2` were covered only at pair level. Both pins now assert
  every vendored spelling: `load_kva_…` gained a (deck, load) loop over the
  remaining ten rows on `makeposseq_line`/`_report`/`_shunt`/`_xfmr`/`_pc`,
  each recomputing r4133's literal from the port's own live `kW`/`kvar` through
  the same round trip (doubled on `_pc`, which converts twice), and
  `vsource_isc3_and_puz_…` gained `pc`/`source` and `shunt`/`source`. All 55
  spellings are now asserted; both lanes stay green, so no lane-dependent `%g`
  spelling hides among them.
* **FIXED — the generator pin's hardcoded `pf`.** It built the kvar chain from
  `g_plain`'s kW (whose own pf is 1.0) plus a literal `0.95`, the pf of the
  elements the cells belong to. It now reads both factors live off
  `Generator.g_kva` and asserts they agree with the kW it already had.
* **FIXED — the one port-vs-port assertion** (`props_r4133_pins.rs`, chain C):
  `fmt_g(2π·f·c1, 13) == fmt_g(b, 13)` compared two live port reads. Both sides
  are now compared against a recorded 13-digit literal per line
  (`4.086610309067` / `4.087172109558`), which is also the honest width — the
  recomputation from the 15-digit-truncated `c1` render lands 2 ulp away.
* **FIXED — the staged ledger drafts pointed at gitignored scratch.** Plan
  §1.1(e) drafts lived only in `tmp/rp39/dossier_*.md`, which the handoff itself
  lists as safe to delete. The draft entry shape (channel `r4133`, one
  `property` row per case/class/name/prop, with the per-chain head of the round
  trip) is now staged in `tests/corpus/props_r4133/README.md`, still **not** in
  `ledger.json`.
* **RECORDED, not fixed — nine port-side expectations written as
  `render(<expr>)`** (`Load.ld_wye.kW`, the reactor/capacitor/generator/
  transformer nameplates and two discriminating reads). The `?` surface renders
  15 significant digits, so the exact double cannot be recovered by parsing it
  back, and a hardcoded byte would red in one lane; every *residue* cell is a
  recorded literal, and `load.kw` — the only residue cell among the nine —
  additionally carries the two-lane `matches!` whose value assertion is
  bit-discriminating in the parity lane. The auditor reached the same reading
  ("mostly unavoidable"); strengthening it belongs to a renderer-level pin, not
  here.
* **RECORDED, not fixed — the 34 `controls:autotrans` `wdgcurrents` cells.**
  New evidence (note (b) above): they are exactly co-populated with the
  `autotrans.tap`/`taps` divergence on the same four capi-only regulator decks,
  which makes a RegControl tap divergence the lead. Still nobody's, still not
  RP3.9's — a multi-percent gap in a solved current is not display-class.
  (**Owned 2026-09-03 by §RP3.12**, record below — verdict `UPSTREAM_BUG`,
  the lead confirmed as the root cause.)
* **Confirmed as correct, no action:** `DECLARED_RP39` staying `(55, 27, 19)`
  while `OPEN_RP39` went to `(0, 0, 0)` (it is a measurement of what
  `display_class_but_not_a_render` refuses, and no port render moved — both
  auditors re-derived the same reading from `account()`); the two amps families
  recorded as `PRECISION_ROUNDTRIP` rather than the plan's shape-2
  `STATE_DIFFERS` (their state differs *because* of the same upstream round
  trip, and r4133's value is reproducible from the port's, which is outcome 1's
  own test); `RP39_DISPOSITIONS` living beside `RP3_SETTLED_SHAPES` (different
  routing tables, both pinned literally); and the chains-D/E pins mutating and
  restoring deck state (every restore is read back, and the tree is clean after
  the run).

*Settlement gate.* All five commands green in both lanes, each exit code read
individually, at the **same totals as the sub-step's own gate — 4 285 passed /
0 failed / 5 ignored per lane over 74 test binaries** (the settlement adds
assertions to existing pins, not tests): `props_r4133_pins` **53**,
`props_r4133_replay` **133**, `props_r4133_evidence_lock` **11**,
`oracle_parity_cfg_gate` **11**, `corpus_gate` green on both channels with every
ledger entry hit and none stale, no `#[ignore]` and no name filter. `lane_diff`
was again not required: the settlement touched the same three test/evidence
files plus this record, and no product crate.

**RP3.12 (the `controls:autotrans/*` `wdgcurrents` gap) landed 2026-09-03,
audit settled the same day — `UPSTREAM_BUG` in r4133, never reproduced, with
zero product-crate lines.** The
sub-step exists because of RP3.9's P0 open item (note (b) and the "RECORDED, not
fixed" bullet above): the **34** `autotrans.wdgcurrents` cells on the four
`controls:autotrans/*` regulator decks (`autotrans_both`, `autotrans_reg`,
`midi_autotrans`, `midi_autotrans_both`; 3-7.5 % apart) had no owner and no root
cause, only RP2.4's display-class `OutOfScope` filing — which a multi-percent
gap in a solved current cannot be.

**Root cause, measured on the live r4133 DLL rather than inferred.** r4133's
`RegControl` reaches its controlled element through five unchecked
`TTransfObj(ControlledElement)` casts (`RegControl.pas:926`, `:1026`, `:1296`,
`:1370`, `:1479`) although `TAutoTransObj = class(TPDElement)`
(`AutoTrans.pas:88`) is not a `TTransfObj` (`Transformer.pas:92`) and
`TAutoWinding` (`AutoTrans.pas:59`) lays its fields after `Rdcohms` at different
offsets than `TWinding` (`Transformer.pas:62`), so `Increment :=
TapIncrement[TapWinding]` (`RegControl.pas:1249`) reads the winding's
`MaxTap` = 1.1 pu and `PendingTapChange := Round(BoostNeeded / Increment) *
Increment` (`:1250`) rounds every realistic boost to zero. The control therefore
never arms — **0** r4133 event-log lines on all four decks against the port's
10-13 — so r4133's numbers are the **unregulated** circuit while the port's (and
the pinned 0.14.5 oracle's, which the port matches exactly) are the regulated
one. The defect is identical in r3723, r4088 and r4133; the DSS-Extensions fork
fixed it while refactoring and the port already carries the fixed shape
(`ControlledTransformer` virtual dispatch,
`elements/pd/transformer/mod.rs:472`), so **no engine line changes**. Upstream
report, gitignored and local-only:
`investigations/to_opendss/50-regcontrol-autotrans-ttransfobj-typecast.md`.

**Exposure: zero gated exposure today, in both lanes and on both channels.** All
four decks are `engines: "capi_v0145"`
(`tests/corpus/manifests/population.lock.json:74-77`) and no `r4133`-gated case
pairs a RegControl with an AutoTrans, which is why the gate is green with
nothing excluded. The divergence is whole-case (node voltages 2.2-2.6 %, the
assembled Y, the AutoTrans branch currents/powers, meters, monitors, the event
log, the control queue and all three manifest probes), so the right instrument
is a case-level `kind: "skip"` on the `r4133` channel, not a field exclusion.
Four such entries are **drafted only** (`tmp/rp312/staged_ledger.md`,
`cause_ref: "regcontrol-autotrans-typecast"`, citing `RegControl.pas:1026`,
`:1296`, `:1479`, `AutoTrans.pas:88` and probes E1/E2/E5): landing one today
would be stale on arrival and fail-on-stale would red the gate, so they wait for
the day a deck gains the channel.

**The pin names both numbers on both legs.** `props_r4133_pins.rs:2997`,
`autotrans_wdgcurrents_stay_regulated_where_r4133_never_taps_the_autotrans`. On
`controls/autotrans/autotrans_reg.dss` the port renders `66.95905, (-28.006),
151.5029, (151.99), ...` with `taps = [1, 1.03125, ]` and
`RegControl.rat.TapNum = 5`; the same deck with `edit RegControl.rat enabled=no`
and the tap put back reproduces r4133's census literal `66.99186, (-28.029),
156.314, (151.97), ...` byte for byte with `taps = [1, 1, ]`. On
`midi_autotrans.dss` it is `73.11371` vs `78.15456` A on the **common**
(wye, winding 2) winding, while the **series** winding agrees to 0.018 %
(`117.2108` vs `117.2323`) — that asymmetry IS the signature of a different
landed tap (`[1, 1.06875, ]` / `TapNum = 11` vs `[1, 1, ]`). Both edits are
read back before the re-solve, so "nothing moved" cannot pass on an edit that
never landed. A second, discriminating reading holds each leg to the ampere-turn
identity `|I_c|/|I_s|` vs `VBase_s*tap_s/(VBase_c*tap_c)` (rel < 1e-5), so the
pin also reds if the winding-current derivation drifts without the tap moving —
proven by a mutation probe that feeds the regulated leg tap 1.0 and reds
(2.2626 vs 2.3333).

**Count locks re-derived from the census artifacts, not edited blind.** New
`DECLARED_RP312 = (8, 1, 0)`: 8 vendored spellings
(`tests/corpus/props_r4133/examples_supplement.txt:127-134` =
`tmp/props_census/r4133/claims.txt:1035-1042`) whose cells are
9+8+3+3+3+3+3+2 = **34**, one pair, and **0** in scope on every spelling and on
the pair total. `DECLARED_OUT_OF_SCOPE` (229, 22, 0) → **(221, 21, 0)**:
229 − 8 = 221 rows, 22 − 1 = 21 pairs — the pair's only other row, the
`makeposseq_xfmr` residue, is already RP3.9's, so the pair leaves the bucket
entirely — and the third column stays 0, because a verdict does not create
scope. RP3.9's `DECLARED_RP39` (55, 27, 19), `OPEN_RP39`, `RP39_ROUTING`,
`RP39_PINS` and `RP39_SETTLED_VERDICTS` are byte-unchanged, and the RP2.4-dated
(229, 22, 0) lines further down — the §RP2.4 record's `DECLARED_OUT_OF_SCOPE`
(134, 18, 0) → (229, 22, 0) paragraph, its `OutOfScope` bucket row and its
`RP24_OUT_OF_SCOPE_ROWS` note; searched by string, not by line, because every
later record shifts them — stay as *that* sub-step's record. Both new locks are
live-enforced: a mutated `(9, 1, 0)` / `(222, 21, 0)` copy reds three walks.

**The false ledger cause is corrected, not left standing as history.**
`tests/corpus/ledger.json:25` carried `autotrans-regcontrol-tap` — "a last-ulp
voltage nudges the tap decision across a boundary ... FPC-vs-Delphi, not a port
bug", exactly the conditioning excuse CLAUDE.md forbids, unchallenged since
`9d5852bc` (2026-07-18). The key is renamed to `regcontrol-autotrans-typecast`
and rewritten with the proven mechanism (no entry ever referenced the old key,
and the gate never flags an unreferenced cause —
`corpus_gate/ledger.rs:1452-1463`), and dated `Correction (2026-09-03, RP3.12)`
notes that keep the original readings went into
`docs/upgrade/sweeps/capi015_vs_r4088.md:43`, `docs/upgrade/DIVERGENCES.md:2086`,
`docs/upgrade/known_diffs_burndown.md:70,150` and
`tests/corpus/props_r4133/README.md` §RP1.2 (:213ff).

*Gate.* All five commands green in both lanes, each exit code read individually:
**4 288 passed / 0 failed / 5 ignored per lane** over 74 test binaries (+3 on
RP3.9's 4 285 — `props_r4133_pins` 53 → **54**, `props_r4133_replay` 133 →
**135**, every other binary unmoved), the same five pre-existing `ignored`, no
`#[ignore]` and no name filter. `corpus_gate` **131** over the full 523-case
population in both lanes with every ledger entry hit and none stale, zero
NEVER-APPLIED entries and zero reds on either channel, and the four `ledger::*`
self-tests green after the rename. No golden re-baselined, no tolerance
consulted or moved, no `TODO(compat)` added. `lane_diff` was **not** owed: the
commit is two test files, `ledger.json` and four docs — not one line under any
crate's `src/` — so no engine path, `compat` kernel, lane alias or solver moved
and the 2026-07-31 `max |Δ| = 0` bit-identical baseline stands.

**Open, recorded not chased.** (a) The four staged `skip` entries had **no
tripwire** that would red if a deck gained the `r4133` channel without them (the
existing `the_staged_r4133_property_entries_have_not_landed_yet` is
`property`-scoped) — **closed the next commit** by the audit settlement below,
which landed the `skip`-scoped analogue instead of deferring it to RP4.1.
(b) Two RP2.4-dated tables in `tests/corpus/props_r4133/README.md` (:585, :823
— the RP3.12 correction at :213ff moved them from :569/:807)
still name `§1.3 (autotrans.wdgcurrents)` as the owner — left as history, their
in-scope column is 0 either way, and the dated correction at :213ff points here.
(c) The two test lanes again dropped seven untracked
`tests/corpus/electricdss-tst/Test/AutoTrans/*.txt` files — the known
overlapping-guard snapshot race recorded in §"Standing open follow-ups" as
"`CorpusGuard` can leak deck-written artifacts under concurrency" (cited by
string, not by line, so no later insert can stale it), third sighting, removed
by exact name; no tracked corpus or golden file moved.

**Audit settlement (2026-09-03, `/audit-code` + `/audit-tests`, 12 findings:
1 major, 6 minor, 5 notes — 11 distinct issues, since both auditors raised the
missing `skip` tripwire — 9 fixed, 2 recorded-not-changed, 0 refuted).** No
product crate is touched by the settlement either, so `lane_diff` stays unowed.

* **[major, tests] The pin's "discriminating second reading" read the pinned
  literal, not the engine.** `common_over_series` was handed `leg`'s `&str`
  parameter on both legs, so on the unregulated leg both operands were
  compile-time constants and the ampere-turn assertion could not fail on any
  tree — which made the doc's claim that it "fails if the winding-current
  derivation drifts, not only if the tap does" (and with it the elimination of
  `GetAllWindingCurrents` / `auto_trans/yterminal.rs` as the site) unbacked.
  **Fixed:** both legs now measure `deck.get("AutoTrans.at.WdgCurrents")` and
  predict from `deck.get("AutoTrans.at.Tap")`, so both sides are live reads.
  Non-vacuity probed: perturbing the live string (`151.5029` → `160.0`) reds
  with `2.389520 vs 2.262626`; the test stays green in both lanes otherwise.
* **[minor, code] "2.0–2.6 V outside the band" was wrong in four landed
  artifacts.** Re-measured from the two decks' own `vreg`/`band`/`ptratio` and
  both engines' node voltages: r4133 leaves `LOW.1`/166 = **118.038 V** against
  the band [119, 121] — **0.96 V** under the edge, 1.96 V under the setpoint —
  and `AT69.1`/332 = **119.251 V** against [122.25, 123.75] — **3.00 V** under
  the edge, 3.75 V under the setpoint; 2.12 % / 2.57 % are the *voltage gaps*
  against the port, not volts. The landed text was the first deck's setpoint
  deviation and the second deck's percentage, both presented as band excursions.
  **Fixed** in all four: the pin doc, the `ledger.json` cause, and the
  `DIVERGENCES.md` / `capi015_vs_r4088.md` correction blocks. It stays prose,
  and now says why: `Deck::get` reads `? Class.Name.Prop`, and neither the port
  nor r4133 gives `AutoTrans` a property that renders a bus voltage (no
  `WdgVoltages`), so asserting it would need harness machinery this sub-step has
  no call to build.
* **[minor, code] STATUS called the COMMON winding "the series winding".**
  `73.11371` vs `78.15456` A are winding 2 (`conn=w`); the series winding
  (`conn=s`) is `117.2108` vs `117.2323` and agrees to 0.018 %. That asymmetry
  IS the tap signature, and the sentence inverted it while contradicting
  STATUS's own RP2.2 record. **Fixed**, with the series figure stated beside it.
* **[minor, code] Three stale self-citations.** The record cited pre-commit
  STATUS line numbers (`:5675`, `:5745`, `:5817-5818`) that its own +121 lines
  had already shifted, and README `:569`/`:807` that its own +16-line correction
  had moved to `:585`/`:823`; and `STATUS.md:4155` still carried "no RP3
  sub-step opens while it stays out of scope", the exact sentence whose README
  twin RP3.12 had corrected. **Fixed:** the STATUS-internal citation is now by
  string (the §RP2.4 record's `DECLARED_OUT_OF_SCOPE` paragraph), which no later
  insert can stale; the README pair is corrected; and the RP2.2 line carries the
  same dated correction its README twin got.
* **[minor, code] The staged draft mis-stated the midi iteration counts.**
  `tmp/rp312/staged_ledger.md` read "iterations 9 vs 4" for `midi_autotrans`;
  the port solves `midi_autotrans` in **10** against r4133's 3 and
  `midi_autotrans_both` in **9** against 4 (the report's `6 / 6 / 10 / 9` row
  was read under the neighbouring table's order). **Fixed in the draft** — which
  is gitignored and never staged — together with the event-log counts, now given
  per deck: 10 / 12 / 10 / 13 on the port, 0 on r4133 everywhere.
* **[minor, tests] The new min-gap lock was one-sided and a round number.**
  `min_rel > 100.0 * floor` left the measurement in prose, against the rule this
  same file wrote down after a mutation walked past `RP24_OUT_OF_SCOPE_MIN_RATIO`
  in its one-sided form. **Fixed:** `RP312_MIN_GAP_RATIO = 153.0` with an upper
  bracket at 154.0 and the measurement named (`3.068975820171114e-2` against the
  2e-4 floor = **153.4x**, `examples_supplement.txt:127`; the largest is 376.9x).
  Both directions probed red.
* **[minor, tests] The existence guard's doc still named three citing tables
  while the assertion unions five.** The drift started at RP3.9. **Fixed:** the
  doc now names `RP39_PINS` and `RP312_UPSTREAM_BUG` too, with what each
  witnesses.
* **[note ×2, both auditors] No tripwire for the four staged `skip` entries.**
  Recorded at landing as an RP4.1 candidate; **fixed here instead** — the new
  `the_autotrans_typecast_cases_pair_their_r4133_channel_with_a_skip_entry`
  asserts the *pairing* both ways: each of the four cases gates r4133 **iff** it
  carries an `r4133` `kind: "skip"` entry citing `regcontrol-autotrans-typecast`
  (whose presence in `causes` is asserted too, so the rename cannot be undone
  silently). `RP312_STAGED_SKIPS` carries the four `(case, entry id)` pairs.
  Probed: pointing one row at an `engines=both` case reds with the entry to
  write.
* **[note, tests] The "r4133's census cell, byte for byte" claim was
  unenforced.** House pattern (the RP3.8/RP3.9 pins paste literals too), and
  true — but nothing tied the literal to the vendored extract. **Fixed** rather
  than recorded: `the_rp312_pin_quotes_the_vendored_census_cells` flattens the
  pin file's string continuations and counts the declared rows whose **both**
  columns appear verbatim; `RP312_WITNESSED_ROWS = 2` locks it
  (`examples_supplement.txt:128` and `:134`). Probed at 1 → red.
* **[note, code] RECORDED, not changed: the ownership predicate selects by pair
  name, not by deck.** `regcontrol_autotrans_typecast_row` cannot do better —
  the vendored `Example` row carries no case column (`pair`, `class`, `prop`,
  `rust`, `r4133`, `cells`, `src`), so no per-deck assertion is derivable from
  it. The deck mapping is anchored instead by the census artifacts, the
  `(8, 1, 0)` count lock, the `min_rel` bracket and the pin's two decks; a row
  of this pair arriving from an unrelated deck surfaces as a count mismatch.
* **[note, tests] RECORDED, not changed: the verdict declares 8 spellings, the
  pin witnesses 2** (10 of 34 cells). In spec — the brief asked for one pin with
  two legs — and the six unwitnessed spellings cannot use this pin's
  construction: their decks (the two `*_both` included) open with a snapshot
  `Solve`, so reproducing r4133's unregulated state needs the RegControl
  disabled in the deck SOURCE, i.e. a deck copy the pins harness does not have.
  It is now a *measured* residual (`RP312_WITNESSED_ROWS`) instead of prose.

*Gate (settlement).* All five commands green in both lanes — **4 290 passed /
0 failed / 5 ignored / 0 filtered out** per lane over 74 binaries, `corpus_gate`
unfiltered over the full 523-case population — with the two new guards
(`props_r4133_replay` 135 → **137**; `props_r4133_pins` stays at 54, its pin
strengthened in place). The known overlapping-guard snapshot race dropped six
untracked `Test/AutoTrans/*.txt` again (fourth sighting), removed by exact name;
no tracked corpus or golden file moved.

**RP3.11 (the `Save`/`Dump` re-serialization surface) landed 2026-09-03 —
`KEEP_LIVE_PINNED` on both surfaces: the kill criterion fires on the one cell
that decides it, so the divergence from r4133's serializer is recorded and pinned
rather than reproduced.** The sub-step exists because of the RP3.3 audit
settlement: every echo row in this plan says the same thing about a *compare*,
and RP3.3 measured for the first time what that same difference does where **no
channel compares at all**. It ran after RP4.1, as plan §0 requires, and blocks
§RP5.2 only.

**The premise the plan opened with is corrected, not inherited.** Pascal
`SaveWrite` writes `PropertyValue[iProp]` (`R4133:General/DSSObject.pas:156`),
but that is `Get_PropertyValue` → the **virtual** `GetPropertyValue` (`:45`,
`:117-120`, *"This is virtual function that may call routine"*), whose base body
returns `FPropertyValue` (`:112-115`) and which r4133 overrides in **49** units
of the live `Version8/Source` tree (53 counting `Deprecated/`;
`grep -rniE "^[[:space:]]*function[[:space:]]+T[A-Za-z0-9_]*\.GetPropertyValue"`
over `Version8/Source`, the `CMD_Lazz` duplicate tree and the base `TDSSObject`
excluded — 54 raw hits; the character class has to admit digits, `_` and a
leading indent, or `TGeneric5Obj`/`TTCC_CurveObj`/`TDynamicExpObj` drop out) —
each answering **live** on a hand-picked index set. So "the store" is not a
*meaning* of `Save` in r4133 at all. Verified first-hand on the one class the
plan quotes: `R4133:PCElements/generator.pas:3007-3038` arms 3 `kv`, 4 `kW`,
5 `pf`, 13 `kvar`, 19/20 `maxkvar`/`minkvar`, 26/27, 34/36 and 37-46 live, and
**has no arm 6** — which is the only reason it prints `model=3` beside a live
`kv`/`kW`/`maxkvar`. `Storage`'s arm list *contains* `propMODEL`
(`R4133:PCElements/Storage.pas:1525-1596`): same property, opposite treatment,
same engine. The live/store split is an artifact of which arms each class's
author happened to write, **not a rule** — which is what kills the per-index
"split" option as well as the store one. The only thing r4133's own comment
documents about `Save` is *membership*: *"Write only properties that were
explicitly set in the final order they were actually set"* (`:138-139`).

**The kill criterion fires, explicitly, and on both surfaces.** Matching r4133 on
`modes:ncim/ncim_pv_pq.dss` means emitting `model=3` while `gen_model == 4` in
the same process — printing a value the engine knows to be superseded, which the
2026-08-02 policy forbids; the compare-side exclusion for that very cell already
exists and is already pinned (`tests/harness/props_norm.rs:1815-1817`,
`generator_model_renders_the_live_pv2pq_conversion`). `Dump` shares the one
getter with `Save`, `?`, the property API and batchedit — exactly as both Pascals
route all of theirs through the one virtual getter — so the same cell decides it,
with the difference that `Dump` prints **all** properties with no `PrpSequence`
filter (`R4133:PCElements/generator.pas:2489-2500`) and therefore has all **88**
committed `dump*` artifacts (44 `.txt` + 44 `.meta.json`, 162 lines) behind the
alternative. Measured on the worked example, of the 10 differing `Dump` rows
exactly **one** is the store-vs-live axis; the other nine belong to axes that
already have owners (r4133's deleted `DumpProperties` overrides, the RP3 name
census, `fmt_g`/WP-G4 float spelling, header quoting, four already-pinned echo
rows).

**The rule the port now states positively, in one paragraph**
(`report/save/save.rs` module doc): **values** = the live field through the one
`ClassProps::get_value`; **membership + order** = the explicitly-set chain
(`prp_sequence` / `next_property_set`), including 0.14.5's property-tracking
stamps; **structure and ordering guards** = the *union* of both upstreams'
`SaveWrite` overrides, because every one of them exists to make the emitted deck
re-compile; and no branch that prints a value the engine knows to be superseded.
Four such guards were unported and landed here, all lane-unconditional, none of
them touching *which* value is printed: **P1** `BusVoltageBases.dss` now ends in
r4133's unconditional `CalcVoltageBases` (`R4133:Common/Circuit.pas:2716-2740`,
two plain `Writeln`s) instead of 0.14.5's `! CalcVoltageBases` — the comment is
that engine's compat-flag side (its own `DSS_CAPI_NOCOMPATFLAGS` branch writes it
uncommented), and RP3.11 measured what it cost: every circuit the port saved
re-compiled with `kVBase = 0` on **every** bus (`bus_kvbase(genbus)` **0.0**
against **7.199557856794634** from r4133's own save), the ncim tree came back NOT
CONVERGED at 15 iterations, and the `expcontrol` PV moved from −0.0060 kvar / 14
iterations to **+307.94 kvar / 53** (per-unit-driven controls read those bases;
the old doc claim *"only affects per-unit reporting, not the absolute-volt
re-solve"* is retracted in place, `exec/save_circuit.rs:572-597`); **P2** the
LoadShape `npts`-first branch of `SaveWrite`
(`R4133:General/DSSObject.pas:144-173`; **both** upstreams guard this class —
0.14.5 does it from the other end, `TLoadShapeObj.SaveWrite` stamping
`PrpSequence[npts] := -999; // make sure Npts prop is first`,
`CAPI:General/LoadShape.pas:2376-2380` — and only this port had neither);
**P3**
`TXYcurveObj.SaveWrite` (`R4133:General/XYcurve.pas:978-1003`, new
`elements/general/xy_curve/save.rs`); **P4** `TRegcontrolObj.SaveWrite`
(`R4133:Controls/RegControl.pas:1399-1421`, new
`elements/control/reg_control/save.rs`), both dispatched beside the four
0.14.5-derived overrides in `report/save/save.rs`. `get_value` is untouched,
`dump.rs` is untouched, and **no `set_as_next_seq`/`clear_seq` site was added or
removed**.

**The `PF=0.88`-class sequence item is EXPLAINED, with no product change — and
the counterfactual is costed, not asserted.** `PrpSequence` is stamped in r4133
**only** by `Set_PropertyValue` (`R4133:General/DSSObject.pas:213-221`), and
`InitPropertyValues` ends in `ClearPropSeqArray` (`:122-129` → `:62-69`), so no
constructor mark survives; `SetAsNextSeq` **does not exist in r4133** (0 hits
over `Version8/Source`). 0.14.5 introduced it as its documented *property
tracking* feature (124 call sites / 20 files), guarded by
`DSSCompatFlag.NoPropertyTracking` whose OFF state is *"following the original
OpenDSS implementation"* (`include/dss_capi.h:497-504`), and the port carries the
six creation seeds verbatim — which is exactly why its ncim line reads
`PF(2) Bus1(3) Phases(4) kV(5) kW(6) Model(7) …`. Nothing is stale: `0.88` is the
live `PFNominal` on **both** engines and r4133's own `Dump` prints `~ pf=0.88`.
The seeds stay because the earlier costing ("0 golden lines") was **incomplete**:
the same bitmap is walked by the **AltDSS JSON export**
(`report/export/json/build.rs:61-70` ← `CAPI_Obj.pas:665-733`), a surface with
**no r4133 counterpart at all**, and the pinned oracle's own bytes are the seeded
chain — `tests/golden/json/vsource_micro.json` emits
`{"Name","MVASC3","MVASC1","BasekV","Bus1"}` for a deck that types neither
`mvasc3` nor `mvasc1` (**15 of the 31** committed AltDSS JSON goldens carry
`MVASC3` — 9 of the 25 in `tests/golden/json/`, all 6 in `json_import/`; and
`circuit_micro.json` likewise emits `Ratings`/`NormAmps`/`EmergAmps` for a line
whose deck typed none of them). Dropping the seeds would trade a measured loss of
agreement with the only oracle that surface has for a hybrid — r4133's membership
over 0.14.5's live values — that matches **neither** upstream. The membership the
port ships is r4133's ∪ {0.14.5 tracking} ∖ {RegControl `tapwinding`}: r4133 does
stamp `Line`'s `Seasons/Ratings/NormAmps/EmergAmps`
(`R4133:PDElements/Line.pas:358-366`), which the port emits in the committed
`tests/golden/adiakoptics/midi_torn_tree.txt:10-11`, and its three
`PrpSequence^[i] := 0` unmark sites all have port `clear_seq` twins. The reverse
direction is pinned too: r4133 stamps `tapwinding` when `winding=` is typed
(`R4133:Controls/RegControl.pas:480-483`), 0.14.5 deliberately dropped it
(*"not really required"*, `CAPI:Controls/RegControl.pas:417`) and the port
followed — round-trip-safe, because re-parsing `Winding=2` re-fires the same
`TapWinding := winding` side effect, asserted by the pin. **No sequence-axis row
has a measured consequence in either direction**: substituting the port's
`Line.dss`, `Vsource.dss`, `ExpControl.dss`, `PVSystem.dss` or `Master.dss` into
r4133's own save changes nothing; only `BusVoltageBases.dss` (P1) and
`Generator.dss` move a number.

**Exposure, measured before the decision and not after it.**

| answer | committed cells it moves | echo-table / channel effect | why not |
|---|---|---|---|
| **A — the store, through `get_value`** (all five readers at once) | **2 049** = 162 `dump*` golden lines + 1 322 feeder-JSON cells + 565 `props/` cells | all **82** `PROPS_ECHO_R4133` rows go stale; **60** `Capi(n)` witnesses over 3 093 comparing cases start failing | the kill criterion — and every one of those goldens is a capture of an oracle that renders **live**, so no regeneration can reconcile them |
| **A2 — a `Save`-only store** | the same `save*` bytes, through a serializer neither upstream has | — | needs a per-property `String` shadow + `InitPropertyValues` tables for ~50 classes; 0.14.5 deleted `FPropertyValue` outright and the port has neither, and `Save` would then disagree with the AltDSS JSON export of the same object |
| **C — split by echo category** (`EchoParse` + `EchoDefault`) | **1 480** = 128 dump lines + 914 feeder cells + 438 props cells | 58 rows retire, 24 stay | it is a transcription of ~49 hand-written `CASE` blocks, not a rule (`model` = store on Generator, live on Storage), and `EchoParse` *is* the print-a-known-stale-value case |
| **KEEP_LIVE_PINNED — landed** | **1** content line + **1** lock digest | none | — |

**Goldens: 2 artifacts, 1 content line + 1 digest — and the capi-oracle question
answered rather than skipped.** `tests/golden/adiakoptics/midi_torn_tree.txt:5`
moves `! CalcVoltageBases` → `CalcVoltageBases` (P1 reaching the A-Diakoptics
`Torn_Circuit` tree through the same writer, `exec/tearing_save.rs:64`), and
`tests/golden/golden.lock.json` re-digests that one row of 737. On this line capi
0.14.5 and r4133 **disagree** and the port follows r4133 — but the moved artifact
is a **self-golden** produced by the port's own partitioner and save writer
(`tests/adiakoptics.rs:565-596`; the lock row's own `"anchor": "self"` /
`"born-self: … no oracle emits a comparable tree"` is the proof), so the
RP3.5/RP3.6 "regenerate from the port, with the argument" precedent applies
vacuously and **no `tools/golden/*.py` was run**. Nothing numeric moved with it:
`adiakoptics` is 34 passed / 1 pre-existing ignored in both lanes *after* the
regeneration, the AD solve gates included, so the P1 A-Diakoptics risk the spec
told I1 to measure is closed on the observed side too. **No
`tests/corpus/ledger.json` entry was drafted or written** — the corpus gate
compares model properties through `compare_all_properties` (`exec/view.rs:394`)
and never reads a `Save` or `Dump` byte on either channel, measured rather than
assumed (`corpus_gate` green on the full population in both lanes, every entry
hit, none stale).

**Literal tests moved: 0. Pins added: 8.** Every `Save`/`Dump` literal keeps its
bytes, because the sequence axis is unchanged and the render axis is unchanged —
`save_class_disabled_load_writes_enabled_no` (`golden_reports.rs:6217-6244`,
keeps `PF=0.88` ×2), the RP3.6 `Save` leg (`exec/tests/line_fetch.rs:1010-1052`,
keeps `R1=…`), RP3.8's `save_renders_the_live_result_properties`,
`save_writes_the_stub_names_like_r4133`,
`save_writes_the_code_name_while_dump_hides_it`, `ckt_model_render_round_trips`,
`save_circuit_writes_master`, `save_forms_structural_file_set`,
`stub_rows_are_absent_from_dump_and_json` and `props_roundtrip`'s
`LANE_SKIP_SCENARIO_PROPS` register with its count assertion — the first two are
re-purposed as *sequence* pins by the new module docs, not edited. The four
re-compilability pins are `save_writes_calcvoltagebases_like_r4133`
(`exec/tests/report.rs:975`), `save_write_puts_npts_first_for_loadshape`
(`:1072`), `xycurve_save_write_puts_npts_first`
(`elements/general/xy_curve/tests.rs:375`) and
`regcontrol_save_write_puts_the_transformer_first`
(`elements/control/reg_control/tests.rs:584`); the four **divergence** pins each
name *both* serializations — `save_renders_the_live_model_after_ncim_pv2pq`
(`report.rs:1202`: the port's `New "Generator.g1" PF=0.88 Bus1=genbus Phases=3
kV=12.47 kW=800 Model=4 Maxkvar=1500 Minkvar=-1500 Vpu=1.01` against r4133's
`New "Generator.g1" bus1=genbus phases=3 kv=12.47 kW=800 model=3 maxkvar=1500
minkvar=-1500 Vpu=1.01`), `dump_renders_the_live_model_after_ncim_pv2pq`
(`:1302`, `~ Model=4` against `~ model=3`),
`save_membership_follows_property_tracking_not_prpsequence` (`:1420`, the
`PF=0.88` head plus the JSON reader's key sequence) and
`save_omits_the_tapwinding_that_r4133_stamps` (`:1507`, the reverse direction,
with the re-parse asserted). Every r4133 byte they quote comes from this
sub-step's `epri-worker` probes (`OpenDSSDirect.dll` 11.0.0.1, rev r4133); the
port's bytes are re-derived live, so a regression on either side breaks the test
rather than the record. One deviation from the spec's letter, measured not
preferred: pin 3 asserts the JSON **key sequence** instead of
`vsource_micro.json`'s verbatim bytes, because the float spelling is the lane's
display kernel (`2.0000000000000000E+003` in parity, `2e3` in default) and is
already pinned by the 123-test `golden_json` binary in both lanes; the oracle's
full parity bytes are quoted in the failure message.

**The round-trip measurement, stated plainly.** r4133's own save round-trips
exactly on `ncim`, `isource_both` and `expcontrol`, and fails
**engine-agnostically** on `capcontrol_pf` (switched-cap step state is in no
`Save`) and `autotrans` (both engines write `Redirect RegControl.dss` before
`AutoTrans.dss` → error 124). The port's save damage was dominated by P1, on
whichever engine re-compiled it; after P1 the one remaining difference on those
five decks is the ncim generator line, where `Model=4` re-compiles to a PQ
machine at pf 0.88 (Q = 800·tan(acos 0.88) = **431.79** kvar) instead of the
authored Q-limited PV machine clamped at 1500 kvar — `|V| genbus.1`
7213.235350 → **7161.277193**, both engines reproducing each other's numbers.
The honest reason is in the pin and belongs in this record too: `Save` renders
live values over an *authored* membership, so on a property the solve mutates it
reproduces neither the authored problem (r4133's answer) nor the full solved
state (the live `kvar` is not in the chain, so it is not printed). That hybrid is
inherent to r4133's design as well — it is what its partial getters produce — and
the port's version of it is the one that never prints a value known to be
superseded.

*Gate.* Landed in **one commit** — P1–P4, the eight pins, the two golden
artifacts and this record together. All five commands green in **both** lanes,
each exit code read individually: **4 435 passed / 0 failed / 5 ignored / 0
filtered out** per lane over **74** test binaries, all 74 `test result: ok` and
the two lanes identical binary for binary (+8 on RP4.1's 4 427, exactly the eight
pins), with the library binary moving 1 482 → **1 490** and every other binary
unmoved — `adiakoptics` 34, `save_roundtrip` 9, `golden_reports` 311,
`golden_json` 123, `props_r4133_pins` 54, `props_r4133_replay` 147,
`props_roundtrip` 1, `corpus_gate` **138** per lane (143.2 s default / 140.5 s
parity, the full 523-case population unfiltered) green on both channels with
every ledger entry hit and none stale. The five ignored are the pre-existing set,
and the 25 `golden_*` binaries were re-run after the gate in both lanes with a
SHA-256 manifest of all 728 files under `tests/golden/` identical before and
after — no byte drift. No `#[ignore]`, no name filter used to claim green, no
tolerance consulted or moved, no `TODO(compat)` added. `pwsh -File
tools/lanes/lane_diff.ps1` was **owed** (the commit touches product `src/`) and
came back **VERDICT: PASS, Δ = 0** on every gated kind over 523 cases /
3 220 861 records — `conv`, `cur`, `errs`, `iter`, `loss`, `pow`, `v`, `y` all
`max |d| = 0.000e0`, 0 iteration counts drifted — so the default lane stays
bit-identical to the parity lane and keeps precisely its oracle standing (the
2026-07-31 baseline, unchanged). Expected, and now measured: P1-P4 change the
bytes an emitted deck carries, not the solved model the dump stream compares.

**Open, recorded not chased.** (a) **P0, release-blocking:** a plain user script
panics a `#![forbid(unsafe_code)]` product crate —
`solution/solution/ncim.rs:683`, *index out of bounds: the len is 1 but the index
is 1*, the PQ→PV arm writing `gobj.delta_q_nom[j]` for `j < nphases` while the
vector is still length 1, because the sizing happens in a branch a
model-4-at-birth generator never enters; an 11-line repro with no `Save`
involved, and r4133 on the identical deck does not crash. (b) **NCIM PV→PQ
reporting violates KCL**: on the unmodified `ncim_pv_pq.dss` the port reports
`Generator.G1` −800 kW / **−431.8 kvar** (42.0103 A) while the solve injects the
clamped −1500 kvar (78.5593 A on r4133) — node voltages and the Line/Load rows
match r4133 digit for digit, so KCL at `genbus` is off by 1068.2 kvar; same
family as the Newton stale-`Iterminal` bug, and it is what makes the saved
`Model=4` line look self-consistent. (a) and (b) wanted one dedicated sub-step,
and got it: **§RP3.13 was accepted, executed and landed 2026-09-03** — both are
`PORT_BUG`, fixed in both lanes (with two more defects of the same family found
on the way), so **(a) and (b) are closed**, kept here as the measurement that
opened them; record below, plan §RP3.13. (c) `Master.dss` divergences with **no
measured consequence**: the port prepends `! Saved by dss-rs`, `Set
DefaultBaseFreq=60`, `Set EarthModel=Deri` (r4133 writes none), emits `Redirect
Vsource.dss` after the library block (r4133 writes it first,
`R4133:Common/Circuit.pas:2652-2668`) and never writes `GISCoords.dss`/its
`GIScoords` line (`:2776-2779`). (d) Both engines' saves put `Redirect
RegControl.dss` before `AutoTrans.dss`, so an AutoTrans+RegControl deck
round-trips on **neither** — an upstream defect worth an
`investigations/to_opendss/` report, not a port change. (e) Two `Dump`
store-vs-live rows measured **outside** the 82-row echo table —
`capacitor.faultrate` (r4133 `0` vs port `0.0005`) and `autotrans.tap` (r4133 `1`
vs port `1.06875`, the live regulator tap); since `compare_all_properties` reads
the same getter, §RP5.2 should confirm whether the owning cases are r4133-gated
and, if so, whether a row is missing. (f) r4133 has **64** `DumpProperties`
overrides to the port's 0.14.5-derived **20** (`!DQDV=`, the 34/36 double-paren
wrap, the hardcoded `~ Refuel=False` that contradicts r4133's own getter —
`generator.pas:2495` vs `:3028`): 88 goldens sit on the answer, it has **no**
bearing on the store-vs-live verdict, and it is recorded as an unowned scope
question rather than silently answered. (g) The port has no counterpart of
r4133's `Set_NumPoints` *"keep properties in order for save command"* re-stamp
(`R4133:General/LoadShape.pas:631-636` + `:1665-1677`, `PriceShape.pas:303` +
`:910-916`, `TempShape.pas:302`, `XYcurve.pas:1005-1019` — the setter re-stamps
the sizing property **and then** the array property, so the two stay adjacent in
that order). P2/P3 made it invisible on `Save` for `LoadShape` and `XYcurve`
only; the audit settlement measured the other five classes and closed them with
the port's own sizing-property hoist (P7 below), so `Save` is now guarded on all
seven. What stays open is the **AltDSS JSON export**, which walks the same
un-re-stamped bitmap and is not touched by a Save-time hoist. (h) The divergence runs both ways: r4133's own `SaveWrite`
emits `windgen.kvar=0`, which on reload flattens `PFNominal` to 1.0 and
`kvarMax/kvarMin` to 0 (`tests/props_r4133_replay.rs:1197-1199`) — an upstream
defect on this very surface, where the port is already right; cross-referenced
from the first divergence pin.

**RP3.11 audit settlement (2026-09-03) — 14 raw findings from the two auditors
(6 code + 8 tests), 12 distinct after dedup (both raised the missing `XfmrCode`
override and P2's false doc claim): 10 fixed, 2 recorded, 0 refuted. The declared
"union of both upstreams' `SaveWrite` overrides" is now actually shipped, and the
sizing-property guard covers every curve class instead of two.** Both auditors
landed on the same substantive gap from opposite sides, and both were right: the sub-step stated a
policy it implemented for 6 of the 8 upstream overrides, and the biggest thing
that policy would have removed — a silent, converging wrong circuit — was still
in the tree. Three product changes, all lane-unconditional, none of them touching
*which* value is printed: **P5** `TXfmrCodeObj.SaveWrite`
(`CAPI:General/XfmrCode.pas:667-745`, new
`elements/general/xfmr_code/save.rs`) — without it a 3-winding code saved as
`New "XfmrCode.xc" … Wdg=3 Conn=wye kV=4.16 kVA=5000 %R=0.7 Tap=0.975`, the
active winding only, and re-compiled into a *different* code that converges;
r4133 has the identical defect (measured: `New "XfmrCode.xc" phases=3 windings=3
Xhl=7 Xht=9 Xlt=8 wdg=3 conn=wye kV=4.16 kVA=5000 %R=0.7 tap=0.975`), 0.14.5
fixed it, and the 2026-08-02 policy forbids reproducing r4133's side. **P6**
`TDynEqPCE.SaveWrite` (`CAPI:PCElements/DynEqPCE.pas:252-273`, the `UserDynInit`
tail, dispatched in `report/save/save.rs` because Pascal appends it after
`inherited`) — closes the Phase-8 deferral named in `elements/pc/dyneq_pce.rs`;
on the vendored `Dynamic_KundurDynExp-steady-state-only.dss` r4133 writes
`… DynOut=[speed,dpshaft,]` and stops, losing all six state-variable
initializers (r4133 has no `UserDynInit` at all), while the port now emits and
re-reads `damp=0 pshaft=P0 pterm=P speed=0 theta=Edp mass="3.5 2 * 2220000000
376.99112 / *"`. **P7** the sizing-property hoist
(`report/save/save.rs::sizing_property`): P2/P3 guarded `LoadShape` and
`XYcurve`, but `TCC_Curve`, `GrowthShape`, `PriceShape`, `TShape` and `Spectrum`
still emitted `npts`/`numharm` **last** whenever a deck re-set it after the
arrays, and that line reloads as zeros. Measured on the live r4133 DLL, three of
the five have the same defect upstream (`New "TCC_Curve.z" C_array=[ 1 2]
T_array=[ 10 5] npts=2`, `New "GrowthShape.g" year=(1, 2, ) mult=(1.05, 1.06, )
npts=2`, `New "Spectrum.sp" harmonic=(1, 3, ) %mag=(100, 30, ) angle=(0, 0, )
NumHarm=2`) and two come out safe only through the `Set_NumPoints` re-stamp this
port does not have (item (g)) — so the guard is the port's own, *hoisting* a
sizing property the deck actually set and never adding a token. Exposure: **0**
golden bytes (every committed `Save` line already carries its sizing property
first, and no golden holds a saved `XfmrCode` or a `DynInit` tail), **0**
tolerances, **0** ledger rows, **3** pins added
(`save_rewrites_xfmrcode_windings_like_capi_0145`,
`save_writes_the_dyn_init_tail_like_capi_0145`,
`save_puts_the_sizing_property_first_for_every_curve_class`, all in
`exec/tests/report.rs`), each with a re-compile leg that asserts the recovered
data.

*Findings, one line each.* **AC-1/T2 (major, FIXED)** — the missing
`TXfmrCodeObj.SaveWrite`: P5 above. **AC-2/T3 (major, FIXED)** — P2's doc claimed
*"r4133 never hits that case"* and *"r4133-only: neither dss_capi 0.14.5 nor this
port had it"*; both are false and both are now corrected in the product doc, in
the pin and in this record: 0.14.5 **does** guard `LoadShape`
(`PrpSequence[npts] := -999`, `CAPI:General/LoadShape.pas:2376-2380`), and r4133
**does** print the token twice on any shape that parses no array property
(measured: `New "LoadShape.ls3" npts=5 npts=5`, `New "LoadShape.ls4" npts=4
npts=4 interval=2`). The port's one-`npts`-first output is therefore a
deliberate non-reproduction, now pinned naming both serializations by the two new
legs of `save_write_puts_npts_first_for_loadshape`. **AC-3 (minor, FIXED)** —
`TDynEqPCE.SaveWrite`: P6 above. **AC-4 (minor, FIXED, doc)** — the two new
overrides route through `save_write_token`, which trims and skips the `----`
sentinel where their Pascals do neither; both deviations are now written down at
the call sites (they can only suppress a token that would not re-parse, and no
reachable property of either class renders blanks or the sentinel). **AC-5
(question, RECORDED)** — the round-trip measurement is pinned, not closed: the
port's saved `ncim` deck still re-compiles to `|V| genbus.1` 7161.277193 against
the original 7213.235350 on **both** engines. That is what the plan's kill
criterion prescribes (the alternative is printing `model=3` beside a live
`gen_model == 4`), and closing it in substance depends on open item (b), owned by
§RP3.13 (**accepted and landed 2026-09-03**: the reported Q is live now, but that
sub-step did not itself re-measure this round trip — its own open item (a)) — not
re-opened here. **AC-6 (note, FIXED)** — the four pin
line citations pointed at the `#[test]` attribute; they now name the `fn` lines
and were re-anchored after this settlement's edits. **T1 (major, FIXED)** — the
five unguarded sizing classes: P7 above, and open item (g) is rewritten (its
claim that P2/P3 make the missing re-stamp *"invisible on Save"* was true for two
classes out of seven; what remains open is the AltDSS JSON export, which a
Save-time hoist cannot reach). **T4 (minor, FIXED)** — none of the pins was
enumerated by anything, so a rename left the suite green while STATUS cited the
name: `props_r4133_replay.rs::RP311_SERIALIZATION_PINS` (11 rows) +
`every_rp311_serialization_pin_exists_and_is_cited` now assert both halves —
every row names a real `#[test]` **and** is still cited by name in `STATUS.md`.
**T5 (minor, FIXED)** — `save_roundtrip`'s snapshot compared only absolute-volt
quantities, so reverting P1 left all nine feeder round trips green;
`bus_kv_bases` now compares every bus's `kVBase` **exactly** across the round
trip, with a pre-save vacuity guard. Proven discriminating: with `!
CalcVoltageBases` restored, **7 of 9** cases red with
`bus "611" kVBase changed across save round-trip (2.4017771198288433 -> 0)`.
**T6 (minor, FIXED)** — the `Dump` verdict pin was three `contains` over a
~160-line artifact; it now reads the live field first (`? generator.g1.model` ==
`4`), asserts the header, asserts the **48** `~` rows are all there, and compares
the `Model`, `kvar` and `PF` rows by exact line. Its `431.79425771047` message no
longer calls that number *"the live dispatched kvar"* — it is the
`PFNominal`-derived nominal (800·tan(acos 0.88)), identical on both engines, and
the message now cross-references open item (b), whose 1068.2 kvar KCL gap is the
same number seen from the Powers side. **T7 (note, FIXED for the deciding pin)** —
`save_renders_the_live_model_after_ncim_pv2pq` now *derives* r4133's half: it
reads the `model=` token out of the deck's own bytes, asserts it is `3` (r4133's
getter has no arm 6, so its `SaveWrite` echoes the parsed token verbatim) and
asserts the live value differs, so both numbers of "the port prints 4 where r4133
prints 3" are measured in-test. **T8 (note, RECORDED, evidence closed)** — the
golden re-run script covered the 25 `golden_*` binaries but not
`tests/adiakoptics.rs`, the owner of the one golden that moved; `adiakoptics` +
`golden_lock` were run explicitly (34 + 4 green, the digest equal to the file's
own SHA-256), and they are part of the full `cargo test --workspace` gate below
in any case. *Gate:* all five commands green in **both** lanes on the
settled tree, each exit code read individually — **4 439 passed / 0 failed / 5
ignored / 0 filtered out** per lane over **74** binaries, all 74 `test result:
ok` and the two lanes identical binary for binary (**+4** on RP3.11's 4 435: the
three new pins, library 1 490 → **1 493**, plus the new citation guard,
`props_r4133_replay` 147 → **148**); `corpus_gate` green on the full unfiltered
population in both lanes with every ledger entry hit and none stale;
`save_roundtrip` 9, `adiakoptics` 34 (+1 pre-existing ignored), `golden_lock` 4.
**0** golden bytes moved (`git status --short tests/golden` empty after the run),
0 ledger rows, 0 tolerances, no `#[ignore]`, no name filter. `pwsh -File
tools/lanes/lane_diff.ps1` was owed (product `src/` moved) and came back
**VERDICT: PASS, Δ = 0** on every gated kind over 523 cases / 3 220 861 records
(`conv`/`cur`/`errs`/`iter`/`loss`/`pow`/`v`/`y` all `max |d| = 0.000e0`, 0
iteration counts drifted) — expected, since the settlement changes the bytes an
emitted deck carries, not the solved model the dump stream compares.

**RP3.13 (the two NCIM port bugs) landed 2026-09-03 — verdict `PORT_BUG` × 2,
both fixed lane-unconditionally, with zero `ledger.json` entries, zero golden
bytes and zero tolerances moved.** The sub-step exists because of RP3.11's own P0
open items (a) and (b) above: its mandated `Save` round trip on
`modes:ncim/ncim_pv_pq.dss` turned up two engine-side defects that have nothing
to do with serialization (`tmp/rp311/probe.md` §6, `tmp/rp311/spec.md` §7) — a
panic in a `#![forbid(unsafe_code)]` product crate reachable from an 11-line user
script, and a reported generator power that misses KCL by 1068.2 kvar. The plan
owner accepted the drafted proposal and scheduled it the same day (plan §RP3.13).
A read-only investigation decomposed both before any line was written, and found
**two more** defects of the same family, which the same sub-step fixed.

**Root cause A — the panic (`ncim.rs:683`, *index out of bounds: the len is 1 but
the index is 1*).** `ncim_init_pq_gen` sized every non-PV generator's
`delta_q_nom` to **one** element (`solution/solution/ncim.rs:540`,
`vec![gobj.q_nominal_per_phase]`) — a faithful port of r4133's `InitPQGen`
(`R4133:Common/Solution.pas:1678-1679`) — while all three *writers* index it per
phase: the PV arm's own stamp (`:2107`), the PV→PQ clamp (`:2152-2154`) and the PQ→PV
promotion (`:2254-2256`), each running `j := 0 to NPhases-1`. The per-phase sizing the
port did have (`ncim.rs:277`, `vec![0.0; nphases]`) sits on the model-3 branch,
which a generator born `model=4` never enters, so the first promotion of such a
machine wrote past the end. Minimal repro `tmp/rp313/repro_pq2pv.dss` /
`tmp/rp311/repro_panic.dss` — a 3-phase `model=4` generator with
`maxkvar`/`minkvar` under `Set algorithm=NCIM`, no `Save` involved.

**Root cause B — the reported power ignores the NCIM dispatch.** Under NCIM
r4133 keeps **one** live reactive dispatch and lets every reader see it:
`UpdateGenQ` stamps `ITerminal[j] := -conj((Pnom + j·deltaQNom[j]) / V)` at the
just-updated `NodeV` (`Solution.pas:2108` on the PV arm, `:2305` on the ELSE arm)
and `TGeneratorObj.GetCurrents` **returns that stamp** instead of running the
machine's power-flow kernels (`R4133:PCElements/generator.pas:1406-1410`, an arm
that precedes everything else, the `LastSolutionWasDirect` shortcut included).
The port had the stamp but not the reporting arm, so `Export Powers`/`Currents`,
`Show`, the CLI and the monitors all fell through to `do_fixed_q_gen`, whose
reactive part is `var_base`/`yq_fixed` — frozen at `SetNominalGeneration` time
from the **declared** `kvar`. Only the corpus gate's private
`exec/view.rs::ncim_generator_currents` override reproduced r4133's formula, so
the engine disagreed with itself and the one reader that was right was the one
nothing shipped. Same family as the Newton stale-`Iterminal` bug (CLAUDE.md
§"Known upstream bugs"), and the same principle settles it as RP3.8's: render the
live result, not a stale cache.

**Two further defects the investigation found, fixed in the same sub-step.**
**Bug C** — a second panic (`ncim.rs:234`, *the len is 3 but the index is 3*): the
NCIM flat-start slack override wrote `node_v[1..3]` unconditionally, a faithful
port of `DOForceFlatStart` (`Solution.pas:1650-1654`), which any circuit with
fewer than three nodes overruns (repro `tmp/rp313/repro_1ph_nogen.dss`). **Bug
B′** — `TVsourceObj.GetCurrents` has the same NCIM arm the generator has
(`R4133:VSource.pas:1194`, `if (Algorithm = NCIMSOLVE) and (NodeRef[1] = 1) then
CalcInjCurrAtBus`) and the port had none, so at NCIM's ideal-EMF swing bus the
ordinary `Export Currents` fell through to the `YPrim·V − Iinj` recompute, which
cancels there to ~3e-5 A where r4133 reports 64–125 A on the `modes/ncim` decks.
It was measured and recorded **open** in this record's first draft; the coordinator
then had it fixed inside the same sub-step, before the commit (a measured gap is
never parked, CLAUDE.md / memory rule), so all **four** defects land together —
the numbers are in the fix paragraph and the exposure table below, and the pin is
`ncim_vsource_export_currents_match_oracle` (P6).

**The fix, in one paragraph — one live state, every reader, both lanes.**
`deltaQNom` is now sized per phase for every NCIM generator
(`solution/solution/ncim.rs:566`); the flat-start override is clamped to the node
count (`ncim.rs:244`); a new `SysCtx.ncim` (`elements/traits.rs:542`, seeded in
`solution/solution/state.rs:704` from `Algorithm = NCIMSOLVE`) carries the flag to
the reporting arms; `TGeneratorObj.GetCurrents`' NCIM arm is ported at
`elements/pc/generator/accessors.rs:363`, in Pascal's order — ahead of the
`LastSolutionWasDirect` shortcut — and copies `cd.iterminal[..nphases]`; and the
gate-only override `ncim_generator_currents` is **deleted** from `exec/view.rs`
(the block comment left in its place, `view.rs:249`, says why and names the two
pins that prove both deletions lossless), so the gate reader and the element
reader are one path. The swing `VSource` gets the same treatment, and it needs one extra step, because
`TVsourceObj.CalcInjCurrAtBus` (`R4133:VSource.pas:1085`, reached from
`GetCurrents` at `:1194`) is a KCL sum over *every* element at the swing bus,
which one element cannot reach from inside its own `get_currents`: the sum is
ported as `ncim_stamp_swing_source_currents`
(`solution/solution/ncim.rs:772` — the same lookup, the same PD `Round(Yorder/2)`
/ PC `NPhases` strides and the same unshifted read the deleted override had),
called once as the last statement of `do_ncim_solution` (`ncim.rs:963`) and
**stamped** into the source's `Iterminal`; `TVsourceObj.GetCurrents`' NCIM arm
then returns that stamp (`elements/pc/vsource/solve.rs:315`, guarded as Pascal is
— `sys.ncim && node_ref.first() == Some(&1)`, i.e. `NodeRef^[1] = 1` — **plus one
condition r4133 does not have**: the stamp must be this element's and current for
this `SolutionCount` (`VSource::ncim_swing_stamped_at`,
`elements/pc/vsource/mod.rs:268`, written by the stamp). r4133 needs no marker
because it re-runs `CalcInjCurrAtBus` on every read, and pays for it — with two
sources on the slack node each one's sum calls the other's `GetCurrents`
(`VSource.pas` l.1158; l.1149 excludes only the element itself), so the pair
recurses until the stack dies: own probe 2026-09-03 (micro-part S2), the r4133
DLL prints `thread 'main' has overflowed its stack` and the `epri-worker` process
is killed on `export currents`. Without the marker the port did not crash but
printed `Vsource.SRC2, 0, 0.00, …` for a second slack-node source carrying
`1388.97 A ∠104.04°` (its own `YPrim·V - Iinj`, the value the port printed before
the arm existed), and that zero propagated into the swing sum; the marker restores
both, pinned by `ncim_second_slack_node_vsource_reports_its_own_current` (:1179).
On every gated deck — one source at the slack node — the marker changes nothing:
all five `Vsource.SOURCE` rows stay digit-identical to r4133).
The once-at-convergence stamp is the same object as r4133's on-demand recompute
because nothing between the stamp and a read moves `NodeV` or a connected
element's state; the only shapes that differ are reads with no stamp behind them
(an aborted solve, or `Set algorithm=NCIM` typed after a normal `Solve` with no
re-solve), which the function's doc comment records. With that, the *second*
gate-only override — `ncim_swing_source_currents` — is **deleted** from
`exec/view.rs` as well, and `snapshot_elements` is left with no NCIM special case
at all. Nothing is `cfg`-gated and no `compat::` kernel is touched: both lanes get the same values.
Neither upstream overrun is reproduced (CLAUDE.md 2026-08-02): r4133's per-phase
write over the length-1 `InitPQGen` array **corrupts the r4133 DLL** on an 8-line
deck — the solve still answers (`converged=True`, 5 iterations, node voltages the
port matches to the digit) and the DLL then hangs on the first *element* access
after it (`set_active_element` never returns) — and `DOForceFlatStart`'s
`NodeV[1..3]` **corrupts its heap** on a 2-node circuit (it answers, then cannot
`quit`). Both are pinned against r4133's solve-level answer where it has one and
by "does not panic" for the element rows it cannot report.

**Exposure, measured before the fix and after it — no gated field moved.** The
gate's element channel is `dss.snapshot_elements()`, whose NCIM generator rows
were *already* produced by the deleted override's r4133 formula; the fix makes
the element path produce the same numbers, so the compared bytes are unchanged.
All five gated NCIM cases are `engines: "r4133"` (0.14.5 has no NCIM at all —
`ledger.json` cause `ncim-oppoint`), so r4133 is the only channel in play.

| case | channel / field | before | after | r4133 | floor |
|---|---|---|---|---|---|
| `modes:ncim/ncim_pv_pq.dss` | **r4133, gated**: `snapshot_elements` `Generator.g1` powers per conductor | `(-266.6666666666667, -500.0)` | **unchanged, bit for bit** | `(-266.667, -500.0)` | not consulted (Δ = 0) |
| `modes:ncim/ncim_pv_pq.dss` | *no channel*: `Export Powers` `Generator.G1` t1 | `-800.0, -431.8` | `-800.0, -1500.0` | `-800.0, -1500.0` | n/a |
| `modes:ncim/ncim_pv_pq.dss` | *no channel*: `Export Currents` `Generator.G1` | `42.0103 ∠151.09` | `78.5593 ∠117.52` | `78.5593 ∠117.52` | n/a |
| `modes:ncim/ncim_pv_pq.dss` | physics: Σ terminal powers at `genbus` | `(0, +1068.2)` kvar | `(-2.3e-13, +2.8e-12)` | `(0, 0)` | 1e-6 (pin P4) |
| `modes:ncim/ncim_midi.dss` | *no channel*: `Export Powers` `Generator.G1` | `-600.0, -323.8` | `-600.0, -400.0` | `-600.0, -400.0` | n/a |
| `modes:ncim/ncim_midi.dss` | *no channel*: `Export Currents` `Generator.G1` | `31.9542 ∠150.60` | `33.7957 ∠145.27` | `33.7957 ∠145.27` | n/a |
| `modes:ncim/ncim_midi.dss` | physics: Σ terminal powers at `b5` | `(0, +76.2)` kvar | `(-4.7e-11, -6.3e-11)` | `(0, 0)` | 1e-6 (pin P4) |
| `modes:ncim/ncim_pq.dss` | *no channel*: `Export Currents` `Vsource.SOURCE`, phase-A magnitude | `3.24074e-05` | `90.0718 ∠158.27` | `90.0718 ∠158.27` | n/a (digit-identical) |
| `modes:ncim/ncim_pv_pq.dss` | *no channel*: `Export Currents` `Vsource.SOURCE`, phase-A magnitude | `3.24074e-05` | `64.2127 ∠-150.28` | `64.2127 ∠-150.28` | n/a (digit-identical) |
| `modes:ncim/ncim_midi.dss` | *no channel*: `Export Currents` `Vsource.SOURCE`, phase-A magnitude | `4.42577e-05` | `124.964 ∠161.49` | `124.964 ∠161.49` | n/a (digit-identical) |
| `modes:ncim/ncim_{pq,pv_pq,midi}.dss` | **r4133, gated**: `snapshot_elements` `Vsource.source` currents | the override's values | **unchanged, bit for bit**, and now `==` the element path | `(-83.67029, +33.34980)`, `(+70.71692, +55.78569)`, `(+12.95337, −89.13549)` A on `ncim_pq` | 1e-4 / 1e-2 (pin `ncim_vsource_reported_currents_match_oracle`), and exact `==` for the two readers (P6) |
| `modes:ncim/ncim_pq.dss` | **r4133, gated**: every other field (no generator on the deck) | unchanged | unchanged | — | not consulted |
| `feeders:Xmission_System_Kundur2Area` | **r4133, gated**: every field, the swing `Vsource` current/power/loss included | unchanged | unchanged — 4/4 green in both lanes after the fix, i.e. the element path now carries what the override did | `20295.6` A (r4133's own `Export Currents` phase-A magnitude; port-before `0.0106809`) | the live whole-model compare at the tier floor, no ledger entry |
| `IEEETestCases/IEEE118Bus/master_file.dss` (`kind: large`) | **r4133, gated**: every field | unchanged | unchanged — 1/1 green in both lanes after the fix | — | not consulted |

`Xmission_System_Kundur2Area` is the one row whose "after" is read off the **live
gate** rather than off an own `Export Currents` run: its `Master.dss` ends in
`export`/`show`/`summary` commands that would write into the vendored corpus tree,
so no unit test drives it — but the case is live-gated on r4133 over the whole
model *including* the swing `Vsource`'s current, power and loss
(`tests/corpus/manifests/solvable_now.json` says so in as many words), it is
**4/4 green in both lanes after the fix**, and the fix is the only thing that now
feeds that reader. On the three `modes/ncim` decks the fifth `Export Currents`
cell — terminal 1's residual, the three phase currents summed — prints numerical
zero on both engines (port `8.15881E-12` / `2.11546E-12` / `3.63457E-11` against
r4133's `4.3961E-012` / `1.13153E-012` / `1.57857E-011`, ~1e-14 of the phase
current): a three-term cancellation floor, not a divergence, and no channel reads
it. The `Export Currents` conductor-4 cell moves from the port's `1.4571E-14` to an
exact `0`, because the new arm zero-fills the tail r4133 leaves as
shared-`cBuffer` garbage — r4133 prints `5.32907E-015` there, and `853.417 A` in
conductor 4 of all three Kundur generators, while its API path returns `0.0`,
which is what the gate compares and what the port emits before and after. No
golden and no gate cell reads that column. **There are five gated NCIM cases, not
the four the earlier text says**: `IEEETestCases/IEEE118Bus/master_file.dss` also
runs `set algorithm=NCIM` (`population.lock.json:538`, `engines: "r4133"`,
`kind: "large"`); it is green, and its swing-bus machine `Gen_at_89_1` is
commented out, so the swing-bus premise holds there too.

**Zero ledger entries — because the gate reader was already live, not because the
obligation was waived.** No entry in `tests/corpus/ledger.json` mentions ncim (the
`ncim-oppoint` cause at `:36` is documentary and referenced by none), and none was
added, removed or made stale: nothing an oracle channel compares moved, so there
is no observable to exclude field by field. `assert_all_hit` is green on the full
unfiltered population in both lanes. `tests/golden/ncim/` holds only the two
Jacobian CSVs and the two `Show PV2PQ_Conversions` texts — all solve-side and
byte-identical here — and no `Export Powers`/`Export Currents` golden exists for
any NCIM deck, so **0** golden bytes moved. `tests/harness/lane.rs` and its
`LANE_SKIP_*` lists are untouched: nothing is lane-conditional.

**Eight expected-value pins, each naming both engines' numbers** — all in
`crate::exec::tests::ncim` (`crates/dss-core/src/exec/tests/ncim.rs`):
`ncim_pq2pv_promotion_does_not_panic_and_closes_kcl` (:611 — the deck that used to
panic converges in 5 iterations, `gen_model = 4` / `ncim_expv = true` /
`ncim_idx = 1` / `delta_q_nom = [-500000.0; 3]`, i.e. it promoted PQ→PV, overshot
the −1500 kvar limit and converted back at `qMin`, which is positive proof the
promotion arm ran; KCL at `genbus` `(7.5e-12, 2.7e-12)`; the audit settlement
added the r4133 leg the first measurement had wrongly declared unavailable —
`converged=True`, 5 iterations and all six node voltages, live),
`ncim_missing_voltage_bases_does_not_panic` (:684 — `is_solved = false`, 15
iterations, `SOURCEBUS.1..3` equal to r4133's `7199.557856794634 + 0j` /
`-3599.77892839732 - 6234.999999999999j` / `-3599.778928397315 + 6235.000000000001j`,
with r4133's `GENBUS.1..3 = NaN` doc-commented as the degenerate `VBase = 0`
answer both engines give), `ncim_below_three_nodes_does_not_panic` (:744 — its
`SOURCEBUS.1` / `LOADBUS.1` are bit-identical to r4133's, which proves r4133's own
overrun did not perturb its two nodes),
`ncim_generator_reports_the_dispatched_q_not_the_declared_kvar` (:791 — both decks,
both engines' powers and currents quoted, KCL closed),
`ncim_gate_reader_and_ordinary_reader_agree` (:866 — `snapshot_elements` against
the element path, conductor by conductor, exact `==`, which is what makes the
deleted override provably lossless), the tripwire
`ncim_swing_bus_carries_no_pc_element_on_the_gated_decks` (:929 — the stated reason
the gate cannot move; it reds the day a gated NCIM deck puts a generator **or a
second source** on the swing bus), `ncim_vsource_export_currents_match_oracle` (:1018 — the swing
`VSource`, three decks × four legs: the `Export Currents` `Vsource.SOURCE` row
compared against r4133's own `EXP_CURRENTS.CSV` prefix cell for cell, the phase-A
magnitude/angle against `90.0718 ∠158.27` / `64.2127 ∠-150.28` / `124.964 ∠161.49`
with the port-before `3.24074e-05` / `3.24074e-05` / `4.42577e-05` quoted in the
message, KCL at the swing bus through the *power* path — source + partner line
`< 1e-9` kW while the source carries `> 1e3` kW — and `snapshot_elements ==` the
element path, which is what retires the second override) and
`ncim_second_slack_node_vsource_reports_its_own_current` (:1179 — a second
`Vsource` at `sourcebus` reports its own Thevenin current `|E2 - V| / |Z1|` =
`1388.97 A ∠104.04°` rather than the swing stamp, and the swing row stays the
Kirchhoff bus sum `-I(Line.l1 t1) - I(Vsource.src2 t1)` — the `cadd` PC sign the
pin originally left open was settled in the audit settlement below and is now
subtracted; r4133 cannot referee that deck, its per-read recursion overflows the
DLL's stack, so the pin asserts physics and the port's own identity).
**No new corpus case**: the repro decks would be `engines: r4133` entries whose
element rows r4133 cannot report (element-access hang / heap corruption), i.e.
un-gateable, so they stay in-engine physics pins.

**Four r4133 defects proven, none reproduced** (three in the sub-step, a fourth in
its audit settlement) — `UpdateGenQ`'s per-phase write
over the length-1 `InitPQGen` array, `DOForceFlatStart`'s `NodeV[1..3]` write, and
`TGeneratorObj.GetCurrents`' partial fill leaving shared-`cBuffer` garbage in
conductor `NPhases+1` of r4133's own `Export Currents`. Upstream reports, gitignored
and local-only: `investigations/to_opendss/51-ncim-updategenq-deltaqnom-overrun.md`,
`52-ncim-doforceflatstart-nodev-overrun.md`,
`53-ncim-generator-getcurrents-partial-fill.md`. The third costs nothing — the port
already emitted 0 there and the gate already compares 0. **A fourth** was found by
the audit settlement below and is the only one that changes a reported number:
`TVsourceObj.CalcInjCurrAtBus` adds PC-element terminal currents where it subtracts
the PD ones (`VSource.pas` l.1169 vs l.1135), so r4133's swing-source row misses
KCL by exactly twice the PC current at the bus; report
`54-ncim-calcinjcurratbus-pc-sign.md`.

**Open, recorded not chased.** *(Bug B′ was the first draft's item (a),
"measured, pinned in prose and NOT fixed", with the swing-`Vsource` numbers
`3.24074e-05` / `3.24074e-05` / `4.42577e-05` / `0.0106809` A against r4133's
`90.0718` / `64.2127` / `124.964` / `20295.6`. It is **no longer open**: the
coordinator had it fixed inside this sub-step before the commit — see the fix
paragraph, the exposure table and pin P6 above — and the `snapshot_elements`
values it names are unchanged bit for bit, so the pre-existing
`ncim_vsource_reported_currents_match_oracle` is still green on the same literals
`[(-83.67028508386699, 33.3498024511199), (70.71691867579602, 55.78569119895437),
(12.95336640806454, -89.1354936500793), 0, 0, 0]`, magnitude `90.0718`. The
tripwire `ncim_swing_bus_carries_no_pc_element_on_the_gated_decks` keeps its job:
it reds the day a gated NCIM deck puts a generator on the swing bus and the
generator's stamp starts feeding that sum.)*
(a) RP3.11's **AC-5** round-trip question is *not* closed by this sub-step, only
un-blocked: the reported Q is now live, but
RP3.13 did not re-measure the saved deck's re-compile (`|V| genbus.1` 7161.277193
against the original 7213.235350 on **both** engines), so AC-5 stands as written
above, with its dependency on RP3.11's own item (b) now discharged on the
reporting side.
(b) The two `Dump` store-vs-live cells RP3.11 measured outside the 82-row echo
table — `capacitor.faultrate` (r4133 `0` vs port `0.0005`) and `autotrans.tap`
(r4133 `1` vs port `1.06875`) — are **record only** here as well: RP3.13 touched
neither, no oracle channel compares `Dump` bytes and neither cell has a
`PROPS_ECHO_R4133` row, so the exposure is zero on both channels; RP3.11's open
item (e) keeps them, with §RP5.2 to confirm whether the owning cases are
r4133-gated. (c) The `epri-worker` answers the **solve** on both repro decks and
P1/P2/P3 pin that answer (P1's leg was added by the audit settlement); what it
cannot report on them is the **element** rows (the deltaQNom deck hangs on the
first element access, the 1-phase deck corrupts its heap), so the terminal-power
and current legs of those three pins stay physics-and-no-panic.

*Gate, re-run end to end on the final tree (the one that carries the swing-`VSource`
arm too).* Landed in **one commit** — the four fixes, the eight pins and the
prose together, over **0** golden bytes and **0** `ledger.json` bytes. All five
mandatory commands green in **both** lanes, each exit code read
individually — `cargo fmt --all --check` clean, both `clippy` lanes clean with
`-D warnings`, and `cargo test --workspace` **4 447 passed / 0 failed / 5 ignored**
in each lane over **74** test binaries, the two lanes identical binary for binary
(library **1 501** per lane, of which `exec::tests::ncim` is **18** —
the 10 pre-existing tests unchanged plus the eight new pins; the 5 ignored are the
pre-existing golden-generator/diagnostic set, and the diff adds no `#[ignore]` and
no `should_panic`). The full unfiltered corpus
gate was then re-run in both lanes on the settled tree: `corpus_gate` **138
passed / 0 failed / 0 ignored / 0 filtered out** per lane over the whole
523-case population (140.46 s default, 140.70 s parity), both channels, every
ledger entry hit and none stale — that unfiltered run is the only one that
executes `assert_all_hit`, so it is what proves the no-stale-entry claim — with
the `ledger::*` self-tests and the two
`props_norm` liveness guards green; `oracle_parity_cfg_gate` **11**,
`props_r4133_pins` **54** and `props_r4133_replay` **148** (RP3.11's counts,
unmoved — this sub-step adds no test to those binaries), **0 filtered out** in
every binary and both lanes, and re-run after the last prose edit because
`props_r4133_replay` and `oracle_parity_cfg_gate` read `STATUS.md` at runtime.
`DSS_GATE_ONLY` was explicitly cleared before both corpus runs; no `#[ignore]`, no name filter used
to claim green, no tolerance consulted or moved, no `TODO(compat)` added, and
**0** golden bytes moved (`git status --short tests/golden` empty after the
runs).

**One unresolved one-off, recorded not dismissed:** the first unfiltered
`corpus_gate` run of the swing-`VSource` micro-part came back `137 passed;
1 failed` on `corpus_gate_all_cases_match_engines` with the message lost to a
`grep` filter; it did not reproduce in any run since — that micro-part's own four
re-runs, and this settlement's two unfiltered runs (default and parity) plus the
two full `cargo test --workspace` runs, which drive the same binary. The five NCIM
cases pass deterministically in both lanes every time, and the blast radius of
that change is NCIM-only — but the message was never captured, so it is logged
here as unexplained rather than as proven-unrelated. The most likely cause is
the `CorpusGuard`/AutoTrans race below, whose dropping set is demonstrably
nondeterministic run to run. `pwsh -File tools/lanes/lane_diff.ps1` was **owed**
(product `src/` moved — seven files, the swing-`VSource` arm included) and was run
**on the final tree**, the one this record describes: **VERDICT: PASS, Δ = 0** on
every gated kind over 523 cases
/ 3 220 861 records — `conv` 2 162, `cur` 1 170 100, `errs` 519, `iter` 2 162,
`loss` 366 476, `pow` 1 170 100, `v` 375 816 and `y` 1 738 084 records compared, all
`max |d| = 0.000e0` and `max rel = 0.000e0` ("identical"), 0 iteration counts
drifted, no documented divergence present
— so the default lane stays bit-identical to the parity lane and keeps precisely
its oracle standing (the 2026-07-31 baseline, unchanged). That is the expected
result and the one that matters here: the fix is lane-unconditional, so both
lanes had to move together. The runs again left the known untracked
`tests/corpus/electricdss-tst/Test/AutoTrans/*.txt` set behind — the overlapping-guard
snapshot race recorded in §"Standing open follow-ups" as "`CorpusGuard` can leak
deck-written artifacts under concurrency", **seventh** sighting (the sixth is in
the §RP4.1 record below) and every run of this sub-step since. The set is
**nondeterministic**: successive unfiltered runs left 9, then 6, then 6, then an
11-file set carrying mixed-case duplicates (`auto3bus_hl_current.txt` beside
`Auto3bus_HL_current.txt`), and the final runs left **8** — which is itself
evidence for the race. Removed by exact name after every run, `git clean` never
used, no tracked corpus or golden file moved.

***Audit settlement (2026-09-03).*** Both auditors ran at `opus-xhigh` over
`9fbb0abf..2ce1a66e` and returned **twelve** findings (5 code + 7 tests); the
dedicated fix agent settled each one against evidence — a live `epri-worker`
probe, the r4133 Pascal, or a recomputation — never against plausibility.
**Six fixed, six recorded, none refuted.** Two were substantive.

* **AC-2 (major) — FIXED, and it is a fifth defect fixed by this sub-step (after
  A, B, C and B′) — the fourth r4133 defect it proves.** The ported
  `ncim_stamp_swing_source_currents` reproduced r4133's PC-loop `cadd`
  (`VSource.pas` l.1169) while subtracting the PD terms (l.1135), and P8 pinned
  that identity. Settled by measurement, on the one deck shape where the two
  engines can differ and r4133 can still answer — a 1000 kW / 400 kvar load
  bonded straight onto the swing bus (r4133 stack-overflows on P8's *two-source*
  deck, which is why the question had been left open). Both engines diverge
  identically there (any PC element on the slack node breaks NCIM's slack
  constraint; 10 kW / 100 kW / 1000 kW loads and a 500 kW generator all hit the
  iteration limit on both), and at the shared 15-iteration state they agree on
  every node voltage and on the `Line`/`Load` terminal currents to the digit.
  r4133's swing row is then `Vsource.source I1 = -12.910456 + 52.625721j A`
  (`54.1862 ∠103.78°`) = exactly `-I(Line.l1 t1) + I(Load.ldswing)`, leaving a KCL
  residual of `85.035098 - 43.071927j A` = precisely `2·I(Load.ldswing)`. Since
  every OpenDSS `GetCurrents` returns the current flowing *into* the element (PD
  and PC alike — it is what makes a load report `+P` and a generator `-P`, and
  `Solution.pas:2108` stamps the NCIM generator the same way), KCL at the bus is
  `I(source) + Σ I(others) = 0` and **both** loops must subtract: the `cadd` is an
  upstream sign bug, and CLAUDE.md forbids reproducing one in any lane. The PC
  loop now subtracts (`solution/solution/ncim.rs`), so the same read is
  `-97.945554 + 95.697649j A` (`Export Currents` prints `136.936 ∠135.67`) and KCL
  closes to `< 1e-9 A`. Pinned by the new
  `ncim_swing_sum_subtracts_pc_terminals_and_closes_kcl`, which names both
  engines' rows and asserts the residual is exactly twice the load current;
  upstream report `investigations/to_opendss/54-ncim-calcinjcurratbus-pc-sign.md`
  (it also names the two neighbouring defects the port already refused: the PC
  loop's un-reset `myTerm` and its `NPhases` stride). **Zero oracle exposure**,
  re-derived rather than inherited: the divergence needs a PC element other than
  the source on the slack node, no gated NCIM case has one, and `ledger.json`,
  `tests/golden/` and every gated channel are unmoved. P8 keeps its deck but now
  asserts plain KCL — `I(SOURCE) + I(Line.l1 t1) + I(SRC2) = 0`, with
  `Vsource.SOURCE` printing `1332.03 ∠-79.45` where the `cadd` form printed
  `1450.64 ∠107.23`.
* **AC-1 (major) — FIXED; the sub-step's own evidence was wrong.** RP3.13 recorded
  that r4133 "cannot answer `repro_pq2pv.dss` at all — the `epri-worker` never
  replies". It answers: own re-probe 2026-09-03, `converged=True`, **5**
  iterations and six node voltages the port matches to the digit. What the
  `deltaQNom` overrun actually kills is the **first element access after the
  solve** (`set_active_element Line.l1` never returns; the run killed at 150 s had
  burned 0.12 s of worker CPU — blocked, not spinning), which is what the original
  element-reading probe hung on. So P1 gained the oracle leg it should have had —
  `is_solved`, `iteration == 5` and all six `YNodeVarray` entries against live
  r4133 — and the false claim was corrected in all six places that carried it
  (`solution/solution/ncim.rs`, `exec/tests/ncim.rs`, `STATUS.md` ×3, the plan
  §RP3.13 and `docs/upgrade/DIVERGENCES.md`). The overrun is still a proven,
  non-reproduced r4133 defect; only its *symptom* is restated.
* **T1 (major) — FIXED.** The eight (now nine) pins were in no existence/citation
  guard, breaking the convention RP3.11 and RP3.12 both followed, so a rename
  would have orphaned every `STATUS.md` / plan / `docs/` citation with a green
  suite. `RP313_NCIM_PINS` + `every_rp313_ncim_pin_exists_and_is_cited` now sit
  beside `RP311_SERIALIZATION_PINS` in `crates/dss-core/tests/props_r4133_replay.rs`
  (each row must name a real `#[test]` **and** be cited by name here).
* **AC-4 / T3 (minor) — FIXED.** The tripwire checked three of the five gated NCIM
  cases and argued the other two in prose. It is renamed
  `ncim_swing_bus_carries_no_pc_element_on_the_gated_decks` and widened twice
  over: on the three small decks it now checks **every** enabled PC element
  (loads and sources included, not just generators) against the swing source's own
  bus — which is exactly the set `CalcInjCurrAtBus`' PC loop sums — and
  `Xmission_System_Kundur2Area` (`b1`) and `IEEE118Bus` (`89_clinchrv`) are
  covered by a source scan of every `.dss` file in their directories for an
  uncommented PC-class `bus1=`/`bus=` binding to the swing bus. They cannot be
  compiled in a unit test (their masters end in `export`/`show`/`summary`, which
  would write into the vendored corpus tree), and the scan honours `!` comments,
  `~` continuations and `bus1=89_clinchrv.1.2.3` node lists.
* **T4 (minor) — FIXED.** P8's Thevenin band was `1e-3` relative where the
  measured agreement is `8.4e-10`; it is now `1e-6` (three orders of margin) and
  its angle band `5e-2 → 5e-3`, the 2-decimal print half-ulp its sibling legs
  already use. Tightened only — no band anywhere was loosened.
* **T2 (minor) — FIXED.** The gating ledger's `ncim-oppoint` cause still named
  `exec/view.rs::ncim_swing_source_currents`, deleted by this sub-step. It now
  names `ncim_stamp_swing_source_currents`, the `vsource/solve.rs` arm and the PC
  sign decision. (Documentary text only: the cause is referenced by no entry, so
  nothing gated moved — which is also why it could go stale unnoticed.)
* **T7 (note, pre-existing and outside the audited range) — FIXED anyway,** since
  it cost an auditor a round-trip and blocks reproducing the gate from any
  out-of-tree copy: `corpus_gate/engines.rs::epri_worker_bin` hard-coded
  `<workspace>/target/<profile>/epri-worker` and ignored `CARGO_TARGET_DIR`, which
  is where its own fallback `cargo build` actually writes. It now tries
  `CARGO_TARGET_DIR` first when set, then the in-tree path.
* **AC-3 (minor) — RECORDED, not fixed, and it is now an open item.** `SysCtx.ncim`
  is r4133's **global** `Algorithm`, not a per-solve flag, so the new generator
  NCIM arm — and its precedence over the `LastSolutionWasDirect` shortcut — also
  governs a `direct`/`dynamics`/`harmonics` solve run while `Set algorithm=NCIM`
  is still in force: the machine reports the last NCIM stamp evaluated at the new
  voltages. Not a divergence — the auditor measured **live r4133 returning the
  identical numbers** in the same sequence (`ncim_pv_pq` + `Set mode=direct;
  Solve` → `Generator.G1 78.5593 ∠117.52°`, `(-783.9 kW, -1504.8 kvar)`, KCL at
  `genbus` off by `(1216.1, -704.8)`) — and unreachable from every gated case. Not
  fixed here because, unlike AC-2, there is no measured "correct" answer to move
  to: what a generator *should* report in a direct solve while the NCIM stamp is
  the only current the solver wrote is its own question, and inventing one would
  leave the sole live NCIM oracle on an untested surface. Documented at the arm
  (`elements/pc/generator/accessors.rs`) with both engines' numbers; the candidate
  fix is a `ncim_stamped_at` marker mirroring `VSource::ncim_swing_stamped_at`.
* **AC-5 (note) — RECORDED.** The generator NCIM arm zero-fills conductors at and
  beyond `NPhases`, and `compute_iterminal`/`refresh_iterminal` copy that buffer
  back, so the machine's neutral `Iterminal` slot is zeroed where Pascal's aliased
  self-copy leaves it as it was. No observable moves (r4133's own API path
  zero-fills, and that zero is what every gate channel and `Export Currents` cell
  compares); the alternative — reproducing r4133's untouched tail — is reproducing
  shared-`cBuffer` garbage. Noted at the arm.
* **T5 (note) — RECORDED.** In P6 the `Export Currents` row leg discriminates the
  swing *stamp* but not the VSource NCIM *arm*; the arm's removal is caught by the
  snapshot-vs-element identity leg instead. Both legs stay; recorded so nobody
  "simplifies" the identity leg away as redundant.
* **T6 (note) — RECORDED.** P2 pins `genbus = NaN` as the shared degenerate answer
  for the `VBase = 0` deck. It pins the *shape* (not-a-number, not a panic and not
  a plausible-looking wrong answer), which is the point; if the engine ever
  validates `Set VoltageBases` without `CalcVoltageBases`, that pin is rewritten
  as an improvement, not read as a regression.
* **AC's "suspicious / too convenient" — no finding, but recorded:** the claim
  "the gate's element channel already read the deleted overrides' formulas, so no
  oracle observable moved" is true **empirically** (the gate runs are the
  evidence), not structurally — the retired override computed from `delta_q_nom`
  while the new path returns the `UpdateGenQ` `Iterminal` stamp, and the two can
  differ on a pass where the PV→PQ conversion is the last iteration. The new path
  is the more faithful of the two; the sentence should be read as measured, not
  guaranteed.

Everything above is lane-unconditional (no `cfg`, no `compat::`, no
`TODO(compat)`), and the settlement moved **0** golden bytes and **0** gated
`ledger.json` entries — the one `ledger.json` line it touched is the documentary
`ncim-oppoint` cause text.

*Settlement gate.* All five mandatory commands green in **both** lanes, each exit
code read individually — `cargo fmt --all --check` clean, both `clippy` lanes
clean with `-D warnings`, and `cargo test --workspace` **4 449 passed / 0 failed /
5 ignored** in each lane over **74** test binaries, the two lanes identical binary
for binary (library **1 502** per lane, of which `exec::tests::ncim` is **19** —
one more than the sub-step's 18, the new P9; `props_r4133_replay` **149**, one
more than RP3.11's 148, the new citation guard; `props_r4133_pins` **54** and
`oracle_parity_cfg_gate` **11** unmoved). The corpus gate ran **unfiltered** in
both lanes — `DSS_GATE_ONLY` explicitly cleared — **138 passed / 0 failed / 0
ignored / 0 filtered out** per lane over the whole 523-case population (146.54 s
default, 144.47 s parity), both channels, every ledger entry hit and none stale.
No `#[ignore]`, no `should_panic`, no name filter used to claim green, no
tolerance consulted or moved, no `TODO(compat)` added, and **0** golden bytes
moved. `pwsh -File tools/lanes/lane_diff.ps1` was **owed** (product `src/` moved —
the PC sign) and came back **VERDICT: PASS, Δ = 0** on every gated kind over 523
cases / 3 220 861 records (`conv` 2 162, `cur` 1 170 100, `errs` 519, `iter`
2 162, `loss` 366 476, `pow` 1 170 100, `v` 375 816, `y` 1 738 084 — all
`max |d| = 0.000e0` and `max rel = 0.000e0`, 0 iteration counts drifted), so the
default lane stays bit-identical to the parity lane and keeps precisely its oracle
standing. **Discrimination, proven by mutation** rather than asserted: reverting
the PC sign to r4133's `cadd` reds P9 *and* P8 with the exact pinned residual
(`85.035098 - 43.071927j A` = `2·I(Load.ldswing)`) and makes the port's swing row
`-12.910456091136894 + 52.62572124287604j` — live r4133's own value to ~1e-13,
which is the sharpest available proof that the port's arithmetic is otherwise
identical and only the sign differs; uncommenting `Generators.DSS:5` or
`generators.dss:44` in a scratch copy of the vendored corpus reds the widened
tripwire naming the file, the line and the element (both restored with
`git restore`, no tracked corpus byte moved). The runs again left the known
untracked `tests/corpus/…` deck-written set behind — the `CorpusGuard`
overlapping-guard race in §"Standing open follow-ups", **eighth** sighting —
removed by exact name afterwards; `git clean` never used.

*Commits.* `2ce1a66e` (the sub-step — the four fixes, eight pins and this
record), `217355da` (the audit settlement — the PC-sign non-reproduction, pin P9,
the `RP313_NCIM_PINS` citation guard and the corrected AC-1 symptom, 9 files
+805/−132) and this docs commit (the §RP3.13 / plan / follow-up sync). Nothing
else on `r4133-props` between them; the plan's §0 and §RP3.13 dated lines name
the same three.



**RP3.10 (the reproduced `QMode=0` dispatch) landed 2026-09-04 — verdict `FIX`,
the kill criterion did NOT fire.** The last reproduced upstream bug this plan
uncovered is gone from **both** lanes: `TWindGenObj.SetNominalGeneration` now has
the constant-Q arm r4133 never wrote, at the price of four r4133 `exclusion`
ledger entries under one new cause, one new gate exclusion field (`variables`)
and five expected-value pins — four new, one (`windgen_force_inj_freezes_iterminal`)
re-centred. Zero golden bytes, zero
tolerances moved, zero `cfg`/`compat::` in the fix. The sub-step exists because
RP3.2's audit settlement refused to leave item **D2** merely flagged (plan
§RP3.10, user go-ahead 2026-09-02), and it ran as four micro-parts (E1 engine +
pins, E2 the `variables` exclusion field, E3 measurement/ledger/lock/corpus
prose, E4 this record and the invalidated prose).

**The finding, restated from the spec that opened the sub-step.**
`SetNominalGeneration`'s steady-state `case WindModelDyn.QMode`
(`EPRI r4133 Version8/Source/PCElements/WindGen.pas:1276-1322`) implements arm 1
(PF, `:1277-1288`) and arm 2 (volt-var, `:1289-1319`) and has **no arm 0**, so
`Else kvarCalc := 0` (`:1320-1321`) zeroes the reactive dispatch — while `QMode`
*defaults* to 0 (`Create`, `:1020`), the property help documents it as
`'Q control mode (0:Q, 1:PF, 2:VV).'` (`:429-430`), and the dynamics model spells
the same mode `QMode := 0; // 0 -> Constant Q` (`WTG3_Model.pas:252`) and
implements it as `Qord := Qref` (`:1059-1061`). A default-configured WindGen
therefore injects **zero vars** in power flow however its `kvar=`/`pf=` reads,
and the port reproduced the arm verbatim (`windgen/nominal.rs`, `_ => kvar_calc =
0.0`) — exactly what the 2026-08-02 policy forbids.

**Why it is an omission and not a design — both halves of the kill criterion
refuted, statically and live.** *(a)* A complete 20-hit write census of
`Qnominalperphase` in `WindGen.pas` leaves three live writers: `:1245`
(cut-in/cut-out → 0, mode-independent), `:1325` (the dispatch itself, which under
`QMode=0` is `1e3*0*…` = exactly 0) and the two "init to something reasonable"
seeds `:3002`/`:3025`, both overwritten by `:1325` before any solve;
`InitDQDVCalc`/`BumpUpQ`/`ResetStartPoint` have **no** WindGen caller
(`Common/Solution.pas:963-1000` walks `Generators` only) and `WTG3_Model.pas`
never touches `Qnominalperphase` or `kvarBase`. *(b)* **Probe headline** (live
EPRI r4133 DLL 11.0.0.1 through `epri-worker`, `tmp/rp310/probe.md`): under
`QMode=0` r4133 dispatches **exactly 0** on all five corpus decks and in every
configuration probed — including decks that type `kvar=`, `pf=` **and** `kVA=`
(`kW=1000 pf=0.9`, `kW=1000 kvar=400` and `kW=1000 kvar=-400` all give
`P=-1000.0000016773234, Q=-8.437050548309344e-06`); `edit … QMode=0` is inert and
re-entrant; and the **same engine dispatches the base kvar the moment the `case`
is bypassed or an arm exists** — `model=4`/`DoFixedQGen` (`:1797`, comment
`:1775` *"Q is always kvarBase"*) gives `-726.4287342535065` on
`windgen_snap_delta` and `-985.9891404764404` on `windgen_daily`, and arm 2 with
a flat `y=+1` volt-var curve gives **exactly `kvarBase`**. Arm 1's own saturation
fallback *is* `kvarBase` (`:1284`), and the parent class writes the dispatch
outright (`Generator.pas:1163`), which WindGen could not transcribe because its
`ShapeFactor` carries the wind **speed**, not a pu multiplier (`:1241`). Upstream
report: `investigations/to_opendss/55-windgen-qmode0-zero-var-dispatch.md`
(gitignored, local-only).

**The fix — one arm, both lanes, no `cfg`.** `0 => kvar_calc = self.kvar_base` in
`crates/dss-core/src/elements/pc/windgen/nominal.rs`, with the `_` arm keeping
upstream's `Else` for out-of-range modes (reachable: `q_mode` is a plain `i32`
and `set_wgen_variable(15)` accepts any integer). The doc comment carries the
five independent grounds and the three deliberate omissions: **no `kVArating`
clamp** (`|kvarBase| <= kVArating` is a `RecalcElementData` invariant,
`:1375-1384`, so arm 1's clamp is dead by construction — measured), **no
`LeadLag`** (the sign already lives in `kvar_base`; re-applying it would
double-negate, and arm 1 reaches the same signed answer by putting the sign in
`LeadLag` over a non-negative `sqrt` — probed: `pf=-0.9` gives `+484.32` through
either path), and **`Factor` (`GenMultiplier`) still applies**, because `:1325`
sits outside the `case` (probed: `Set genmult=0.5` halves the mode-0 dispatch
exactly as it halves arm 1's). Dynamics and harmonics are untouched:
`WindGen.pas:1254` already skips the whole P/Q block, so the new arm never runs
there and the two dynamics decks move **only** through the snapshot `solve` their
own deck line performs before `Set mode=dynamic` — measured, not predicted.
`rg "cfg\(feature|oracle-parity|compat::"` over `elements/pc/windgen/` returns
zero hits, so both lanes compute the identical `q_nominal_per_phase`.

**Dispatched values after the fix** (each from its own deck tokens through
`RecalcElementData`; `q_nominal_per_phase == 1e3 * kvar_base / 3` bit-exactly on
all four): `windgen_snap_delta` **726.4831572567788 kvar**, `windgen_daily`
**986.0523155365896 kvar** (constant in wind, unlike arm 1's
**414.89529289898772** at the daily shape's hour-1 10 m/s), both dynamics decks
**854.95263026673 kvar** (`kWBase 1584`, `kVArating 1800`, `PFNominal 0.88` — the
kVA-set branch), and `windgen_snap` **0** (`pf=1.0` ⇒ `kvar_base = 0`, so the new
arm is a literal no-op there). r4133 dispatches 0 on all five.

**A gate field had to be built first: `variables` exclusions.**
`compare_variables` was called unconditionally with no ledger hook, so the WTG3
state variables the fix moves could not be excluded field by field. E2 added
`"variables"` to `LEDGER_FIELDS` (13 → 14), `EXCLUSION_ONLY_FIELDS` (4 → 5, so a
`divergence` naming it is refused at load) and `EXCLUSION_FIELDS` (9 → 10),
taught `compare_variables` a per-index `skip` predicate — **the count assertion
stays unconditional and runs first** — and gave the runner the key
`"<element>:<variable>"`, both halves lowercased, the port's spelling for the
variable. The mask is name-selected, so the harness **asserts** that the oracle
calls each dropped index the same thing (ASCII-case-insensitive) and reds loudly
otherwise; that assertion is also how §3.11's "do both engines spell the 22
variables identically" question was answered (they do). Non-vacuity was proved by
mutation on `IndMach012.m1` in a scratch probe (excluded variable corrupted →
green; clean variable corrupted → red; scope deleted → red; renamed index → red;
count mismatch under a skip-everything mask → red), and the field is covered
without new test code by the existing synthetic drive
`every_exclusion_field_is_honoured_by_the_runtime`. Zero product-crate bytes.

**Exposure, measured live per deck × channel** (`DSS_GATE_ONLY=windgen` with
`DSS_LEDGER_MEASURE=1`, the real post-fix engine against the r4133 DLL — not a
proxy; every "moves" verdict was read off the harness comparators themselves,
element by element, by excluding everything else and seeing which channel reds).
All five cases are `engines: "r4133"` (dss_capi 0.14.5 has no WindGen class at
all), `n_steps=1`, `selected_elements=["*"]`.

| channel | `windgen_snap` | `windgen_snap_delta` | `windgen_daily` | `windgen_dyn` | `windgen_dyn_fault` |
|---|---|---|---|---|---|
| `iterations` | 2 → 2 | 2 → 2 | 2 → 2 | 2 → 2 | 2 → 2 |
| `voltages` | — | **moves** 19.71193 V at wbus (floor 8.208e-6) | **moves** 26.72902 V (8.206e-6) | **moves** 0.08399621 V (3.237e-3) | **moves** 0.1571924 V (2.864e-3) |
| `injection` | — | **moves** 0.4506544 (1.0e-6) | **moves** 0.5947577 (1.0e-6) | **moves** 10.30537 (0.6122) | **moves** 19.83223 (0.5454) |
| `element` | — | **all 3 red** (terminal Q −726.4838 vs −4.2e-05 kvar) | **all 3 red** (−986.0531 vs −2.1e-05) | **all 3 red** (−37077.425 vs −37087.759) | **all 4 red incl. `Fault.f1`** (−29209.383 vs −29216.667) |
| `yprim` (WindGen.w1) | — | **moves** 3.115e-3 S | **moves** 6.341e-3 S | — (max │Δ│ = 0) | — (0) |
| `y` / `y_fingerprint` | — | **moves** (frob 27.82947255 → 27.82882765) | **moves** (frob 27.82924898 → 27.82790332) | — (0) | — (0) |
| `variables` (22 WTG3 state vars) | n/a | n/a | n/a | **3 move**: `Pgen`, `Qgen`, `dOmg` | **4 move**: + `thetaPitch` |
| `property` (RP3.2's four cells) | — | does **not** move | does **not** move | does **not** move | does **not** move |
| goldens | — | — | — | — | — |

`y`/`yprim` stay put on the dynamics decks because the captured Y there is the
dynamics-mode one (`CalcYPrimMatrix`, `WindGen.pas:1441-1451`), which never reads
`Qnominalperphase`. The moving state variables, port against r4133 (allowance in
brackets): `windgen_dyn` `Pgen 1.0045538616795757` vs `1.002198412222774`
[1.200e-4], `Qgen -0.00768950341712867` vs `-0.006297830900882717` [1.001e-4],
`dOmg 0.012407481452484234` vs `0.010128443304934094` [1.002e-4];
`windgen_dyn_fault` `Pgen 1.0086500012360733` vs `1.0042629667839191` [1.201e-4],
`Qgen 0.0688589386657973` vs `0.07121218770486548` [1.014e-4],
`dOmg -0.08630408763613222` vs `-0.0812032858835468` [1.016e-4],
`thetaPitch 4.37634056945387` vs `4.376041805109156` [1.875e-4] — the one
variable that moves on the fault deck and not on the healthy one, where it moves
7.53e-5 against the same allowance and **stays compared**. Iterating the mask
proved each set is exactly minimal: with those three (resp. four) excluded the
case is green, so the other 19 (resp. 18) still gate.

**Ledger: four `exclusion` entries and one new cause, landed in the same commit**
(`tests/corpus/ledger.json` 53 → **57** entries over 29 → **30** causes;
TESTING.md's contents paragraph re-derived off the file). The entries are
`windgen-qmode0-constant-q-{snapdelta,daily,dyn,dynfault}-r4133`, all on the
`r4133` channel, all `cause_ref: windgen-qmode0-no-arm`. `exclusion`, not
`divergence`, is the right kind for the same reason G2.5's were: an engine fix
that declines an upstream bug leaves no envelope to re-assert, and
`y`/`y_fingerprint`/`yprim`/`variables` are exclusion-only fields anyway. Each
entry carries a `voltages` scope that exceeds the tier floor, which is what
`a_voltages_exclusion_that_masks_nothing_is_stale` needs; each `source` records
the measurement method and both engines' numbers; the deck-wide `voltages`/
`element` blankets are the *measured* set on the power-flow decks (6/6 nodes,
3/3 elements) and, on the dynamics decks, carry the `gic_midi`-style
justification for the srcbus nodes that sit at 30 % / 56 % of their floor. **No
entry for `modes:windgen/windgen_snap.dss`** — nothing moves there and an entry
that never applies fails the gate as NEVER APPLIED. Four rows of
`population.lock.json` gain a second `ledger=` id (`windgen_daily`,
`windgen_dyn`, `windgen_dyn_fault`, `windgen_snap_delta`); every count and every
rigor field is unchanged. No `TORN_DOWN_ROWS` row is registered in
`oracle_parity_cfg_gate.rs`, deliberately: that register is tied by arithmetic to
two censuses this sub-step decrements neither of.

**Five expected-value pins, each naming both engines' numbers** — four new in
`crates/dss-core/src/elements/pc/windgen/tests.rs`, unconditional in both lanes:
`qmode0_dispatches_the_base_kvar` (the four corpus token sets through the real
property engine, `assert_eq!` on the exact f64s — the arm is a copy, not
arithmetic — plus the arm-1 discriminator at the daily deck's hour-1 wind, so the
pin separates the arms instead of asserting a hardwired base),
`qmode0_dispatch_carries_the_sign_and_scales_with_genmult` (`pf=-0.9` and
`kvar=-400`; mode 0 and arm 1 agree to 1 ulp; `gen_multiplier = 0.5` halves the
mode-0 dispatch exactly while `var_base` does not move),
`qmode0_zero_only_when_the_base_is_zero` (`pf=1.0` → 0, so `windgen_snap` is
clean *by value*; above `VCutOut` → 0 in every mode; an out-of-range mode still
takes the `Else`) and `dynamics_variables_match_the_qmode0_dispatch` — the
**rewrite** of `dynamics_variables_match_capi015_reference`, which this fix
breaks: same readings, **same tolerances** (1e-3 / 1e-4 / 1e-2 /
1e-6, re-centred, never loosened), `dOmg` added because the gate excludes it too,
and each moved variable asserted through a `diverges(...)` helper that requires
**both** `|v - port| < tol` **and** `|v - oracle| > tol`, so the pin reds if the
engine regresses off the constant-Q dispatch *and* if the divergence silently
disappears. `Pg 1584.0`, `s -0.13875361782251128` and `Vmag` stay as
oracle-agreeing readings — they are what proves the divergence is confined to the
Q channel. The part-S self-check extended it to run **both** dynamics decks
(the fresh-eyes finding: the faulted deck's four moved variables carried a
ledger exclusion and a deck-header claim of this pin, but no pin actually
asserted them): `windgen_dyn_fault.dss` verbatim — the sustained `Fault.f1`,
30x1ms — with `Pgen 1.0086500012360733`, `Qgen 0.0688589386657973`,
`dOmg -0.08630408763613222` and `thetaPitch 4.37634056945387` against r4133's
`1.0042629667839191` / `0.07121218770486548` / `-0.0812032858835468` /
`4.376041805109156` (re-measured live on the r4133 DLL through `epri-worker`),
`thetaPitch` at 1e-4 rather than 1e-2 because its divergence is 2.99e-4, and
`Vmag`/`WtAct`/`Pg`/`s` kept as oracle-agreeing readings there too. The fifth is `exec::tests::force_hooks::windgen_force_inj_freezes_iterminal`, a pre-existing pin the fix
moved (its deck types `kw=2000 pf=0.95` and no `QMode=`): re-centred on this
engine's measured frozen currents with r4133's zero-var readings kept in the
comment and in the assertion message, its stale "the port matches it to a
faer-vs-KLU floor" rewritten as history, its discriminating claim re-measured by
temporarily removing the `get_currents` guard (the recompute lands ≈50 A off in
the imaginary part and energises the neutral at `45.18+91.04i`) and the guard
restored, tolerance unchanged at `1e-6`.

**Corpus prose corrected in the same commit, because three notes asserted numbers
the port no longer produces.** `windgen_snap_delta.dss:3` called this "the
QMode=0 (constant-Q via PF) reactive dispatch" — there is no such arm upstream;
it now says what is true and names the ledger entry. Its manifest note claimed
"PF at unity here because the aero cap makes kvarCalc saturate": **both clauses
were wrong** (measured — the `Else` yields 0 outright and the aero cap plays no
part) and are replaced by both engines' readings. `windgen_daily`'s note was
silent on Q and gains the pair plus the arm-1 contrast; `windgen_dyn`'s
"Pgen 1.0022, Qgen -0.0063" and `windgen_dyn_fault`'s header
"(capi015: Vmag 0.898, Qgen +0.071)" keep their oracle readings and gain the
port's post-fix set and the entry id. CRLF and UTF-8 preserved on all five files.

**Prose the fix invalidated, corrected rather than deleted** (this micro-part):
`props_r4133_pins.rs`'s "the probed terminal powers match on all five decks" and
the fault-ride-through pin's "the solved state is *not* divergent — probed
terminal Q is −29216.72 kvar on the port against −29216.67 on r4133" now read as
what they were, RP3.2-dated measurements taken *because* the port then reproduced
the missing arm, with the post-fix pairs and the ledger entry named; the
`LEDGER_ENTRY_PINS` verdict for `windgen.kvar` says the same (its census phrases,
which `the_rp32_census_decomposition_is_read_off_the_corpus` asserts, are
untouched); the `RP32_WINDGEN_CASES` / `windgen_dispatch` docs and the census
loop's own comment now say the `Else` is the path **r4133 still takes and the
port no longer does**, while the assertion they carry — no census deck types
`QMode=` — is unchanged and unweakened. In `ledger.json`, the
`windgen-kvar-renders-dispatched-q` cause and the `-dynfault` entry's `source`
lose their "the solved state agrees / stays fully compared" claims the same way
(values untouched; the entry's fingerprint change is the fourth `ledger=` row in
the lock diff).

**Dated correction to this record's own RP3.2 text (2026-09-04).** D2's probe
line "`QMode=1` → −363.54 kvar below the aero cap, −985.69 kvar on the daily
deck" mis-states the daily figure. Under the gate's replay
(`Set mode=daily stepsize=1h number=1`, `dblHour = 1.0`, `Pg = 1262.29 kW`)
`QMode=1` dispatches **−414.8954549079094 kvar** (`? kvar` renders `'414.895'`),
and the ≈ −986 family is the *snap-mode capped-`Pg`* reading — the same deck with
`Set mode=snap` gives `P = -3000.0009 kW`, `Q = -986.057422577853`, i.e. arm 1
sitting on its saturation fallback `kvarCalc := kvarBase = 986.0523155365896`,
which is also what the constant-Q arm dispatches. Any statement about the daily
deck must use **−414.895…** (arm 1) or **−986.052…** (the fix), never −985.69.
The originals stay where they are, corrected here and not rewritten.

**What did NOT move.** No committed golden byte (`rg -l -i windgen tests/golden`
finds only two structural JSON schema files; `golden_reports.rs` merely *masks*
the `[WindGen]` block of `dump3_commands`), so `golden.lock.json` is untouched.
No `capi_v0145` entry is possible — the class does not exist in dss_capi 0.14.5.
No tolerance in `tests/harness` or `TOLERANCE_NOTES.md` was consulted or moved,
and `tests/harness/lane.rs`'s `LANE_SKIP_*` lists are untouched: nothing here is
lane-conditional. RP4.1's four `r4133-windgen-kvar-dispatched-*` property entries
keep their `rust`/`oracle` values, the frozen census (`bins.tsv:303`,
`props_r4133_evidence_lock.rs`'s `("windgen", 5, 5)`) is unchanged and RP3.2's
four pins still pass unchanged — the port's `kvar` renders `kvar_base`
(`windgen/accessors.rs`), which the dispatch never writes. So RP3.10 neither
blocked nor unblocked RP4.1, exactly as the plan predicted.

**Live coverage cost, recorded not repaired.** After the exclusions the four
decks still gate on the iteration count, the discrete state, the full property
surface, 19 of 22 state variables on `windgen_dyn` and 18 of 22 on
`windgen_dyn_fault`, and — on the two dynamics decks — the assembled Y, its
fingerprint and the WindGen YPrim. What is no longer compared on them is the
solved model itself (node V, the RHS, element currents/powers/losses), plus
Y/fingerprint/YPrim on the two power-flow decks. It is **not** repaired by adding
decks in this sub-step (scope creep): a `QMode=1` sibling deck, which would gate
the solved model on an arm both engines share, is a follow-up idea.

**Open, recorded not chased** (none is this sub-step's, all found by its probe and
none reproduced): (a) r4133 raises an **access violation** when a deck declares
`QMode=2` on the `New` line without a `VV_Curve=` (minimal repro
`tmp/rp310/decks/syn5_crash.dss`); (b) `varBase` is computed *before*
`RecalcElementData` renormalises `kvarBase`, so models 4/5 can dispatch a stale
base; (c) arm 2 double-negates a negative `kvarBase`. Each is a distinct upstream
defect and would need its own report; the fix above touches none of them.

*Gate.* The micro-parts each ran their own: E1 `cargo test -p dss-core --lib`
**1 505 passed / 0 failed** in *both* lanes (the whole library suite, not a
filter — which is how the one moved pin was found and fixed in the same step),
plus `clippy -p dss-core --lib --tests --all-features` clean; E2 the ledger
structural tests and the mutation transcript above; E3 `DSS_GATE_ONLY=windgen
cargo test -p dss-core --test corpus_gate` green in both lanes (5/5 windgen cases,
the four new entries hit 13/13/13/15 times) **and** the full unfiltered
`corpus_gate` in the default lane — **138 passed** over the whole 523-case
population in 141.6 s, which is the run that executes `assert_all_hit`, so every
ledger entry including the four new ones is applied and non-stale — plus
`population_lock` (which failed correctly *before* regeneration: the anti-shrink
guard saw the new entries). E4 re-ran `cargo fmt --all --check`,
`oracle_parity_cfg_gate`, `props_r4133_pins` and `props_r4133_replay` after the
prose edits, because two of those binaries read `STATUS.md`,
`docs/phase-records/` and `ledger.json` at runtime.

*The five-command gate, both lanes, on the pre-commit tree* (2026-09-04, full
transcript `tmp/rp310/gate.md`, each command's exit code captured separately):
`cargo fmt --all --check` clean; both `clippy --workspace --all-targets
-- -D warnings` runs clean, with no `warning:` line at all and the changed crate
actually re-checked; `cargo test --workspace` and the same with
`--features dss-core/oracle-parity` both **4 452 passed / 0 failed / 5 ignored /
0 filtered out** over 74 result-reporting targets, the two lanes identical binary
for binary. The five ignored are the pre-existing set (the ckt24 `.graph`
diagnostic, the three manual WASM/WM golden generators and one doc-test) — RP3.10
added none. The whole delta against HEAD `08f91bbb`'s 4 449 is the library suite
(1 502 → **1 505**): the three new in-engine pins, the fourth being the rewrite
of an existing test. `corpus_gate` ran **unfiltered** in both lanes over the whole
523-case population (**138 passed**, 143.7 s / 139.0 s), which is the run that
executes `assert_all_hit`, so each of the four new entries is applied and none is
stale, and the ledger's own structural guards are inside those 138 and green in
both lanes.

*`lane_diff`* (`pwsh -File tools/lanes/lane_diff.ps1`, mandatory here because
product code moved): **PASS, exit 0, `max |Δ| = 0` exactly on every gated kind** —
`conv` 2 162, `iter` 2 162, `errs` 519, `v` 375 816, `cur` 1 170 100, `pow`
1 170 100, `loss` 366 476, `y` 1 738 084 values compared, 3 220 861 records per
lane, 0 iteration drifts, and `DOCUMENTED_DIVERGENCES` empty, so nothing was
exempted from the bound. That is the required outcome, not merely "within bound":
the fix is one unconditional arm, so a non-zero Δ on a windgen deck would have
proved it leaked into one lane. The five windgen deck blocks are byte-identical
between the lanes **and** carry the post-fix content — `modes:windgen/
windgen_snap_delta.dss` dumps `pow WindGen.w1 -500.0000392697567
-242.16125287584393` per phase, i.e. the pin's `q_nominal_per_phase =
242161.05241892627` VAr where the pre-RP3.10 engine dumped `0` — which is the
independent proof that both release builds compiled the changed `nominal.rs`.

No `#[ignore]`, no `should_panic`, no name filter used to claim green, no
tolerance touched, no `TODO(compat)` added.

*Two settlement calls taken at ritual step 3* (2026-09-04, i.e. after the gate
transcript above, so the binaries they touch were re-run — see the commit
paragraph). Both were flagged as optional by the exec parts and are done here so
the audit pair reviews them: **(1)** `docs/upgrade/DIVERGENCES.md` gains
**§L7 — WindGen `QMode=0` dispatches `kvarBase` (r4133 zero-var bug, not
reproduced)**, in the L5/L6 shape: the `WindGen.pas` evidence, the live
`epri-worker` per-deck measurement (each deck's `kvar_base` /
`q_nominal_per_phase` against r4133's 0), and the disposition (fixed in both
lanes, four ledger entries, the pins by name, the local upstream report). It is
the catalogue entry this divergence previously lacked. **(2)**
`crates/dss-core/tests/props_r4133_replay.rs` gains `RP310_WINDGEN_PINS` and
`every_rp310_windgen_pin_exists_and_is_cited`, mirroring `RP313_NCIM_PINS` /
`every_rp313_ncim_pin_exists_and_is_cited`: five rows, each naming a pin by its
full module path, asserted to exist as a `#[test]` in its own source file, to be
cited by name in this record, and — for the four the ledger leans on — in
`tests/corpus/ledger.json` itself (citations matched as whole identifiers, not
substrings; the `force_hooks` row is `false` because its deck is not a corpus
case). `props_r4133_replay` therefore goes 149 → **150**. The guard's two halves
were mutation-proved on the real test rather than assumed: renaming one table row
to a `#[test]` that does not exist reds with *"defines no such #[test]"*, and
flipping the `force_hooks` row's ledger flag to `true` reds with *"no entry in
tests/corpus/ledger.json names it"*; both mutations were reverted and the guard
re-run green in both lanes (`cargo test -p dss-core --test props_r4133_replay
rp310` matches exactly this one test).

*Commits.* The sub-step lands in **one** commit on `r4133-props` (the engine arm,
the `variables` exclusion field, the four ledger entries and their cause, the
lock, the five pins, the citation guard, `DIVERGENCES.md` §L7, the corpus and
manifest prose and this record); its sha and the audit settlement's are named by
the settlement's docs sync, as at §RP3.13, together with the plan's §0 and
§RP3.10 dated lines. Because the step-3 edits above land after the five-command
transcript, the binaries that read `STATUS.md`, `docs/phase-records/` and
`ledger.json` at runtime were re-run on the final tree in **both** lanes —
`props_r4133_replay` **150**, `props_r4133_pins` **54**,
`props_r4133_evidence_lock` **11**, `oracle_parity_cfg_gate` **11**, 0 failed, 0
ignored — plus `cargo fmt --all --check` and
`clippy -p dss-core --test props_r4133_replay -- -D warnings`.
