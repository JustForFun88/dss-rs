"""Generate the AltDSS JSON-export byte goldens from dss-python (oracle).

For each micro deck (and IEEE13 element samples) we run the deck on the pinned
oracle, then capture the *exact bytes* of `DSSElement_ToJSON` (single object) and
`IActiveClass.ToJSON` (class batch) for a matrix of option combos. The Rust
engine (`golden_json.rs`) replays the identical deck and must reproduce the bytes
verbatim — a byte-exact gate, justified because JSON export is a model dump of
parsed input properties (no faer-vs-KLU last-ULP exposure). Captured PRE-SOLVE.

Writes one file per deck under tests/golden/json/<name>.json holding the deck +
every capture (kind/target/opts/bits/expected-bytes). Regeneration is manual and
must use the exact versions in tools/golden/PIN.txt.

Usage:
    python tools/golden/gen_json.py                 # regenerate every deck
    python tools/golden/gen_json.py autotrans_micro # only the named deck(s)
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = REPO_ROOT / "tests" / "golden" / "json"
SCHEMA = 1

# Option combos: (name, bits). Bits match Pascal DSSJSONOptions (0-10).
FULL = 1
SKIP_REDUNDANT = 2
ENUM_AS_INT = 4
FULL_NAMES = 8
PRETTY = 16
EXCLUDE_DISABLED = 32
INCLUDE_DSS_CLASS = 64
LOWERCASE_KEYS = 128
INCLUDE_DEFAULT_OBJS = 256
SKIP_TIMESTAMP = 512
SKIP_BUSES = 1024

COMBOS = [
    ("default", 0),
    ("full", FULL),
    ("full_pretty", FULL | PRETTY),
    # default-mode pretty and the bare IncludeDSSClass header (bit 6 alone),
    # not just their Full-mode pairings, so the compact-vs-pretty layout and the
    # header logic are each pinned in the default sweep too.
    ("pretty", PRETTY),
    ("include_class", INCLUDE_DSS_CLASS),
    ("enum_as_int", ENUM_AS_INT),
    ("full_names", FULL_NAMES),
    ("full_include_class", FULL | INCLUDE_DSS_CLASS),
    # SkipRedundant only branches in the Full sweep; vsource exercises it (its
    # R1/X1/R0/X0 defer to Z1/Z0 and drop out).
    ("full_skip_redundant", FULL | SKIP_REDUNDANT),
    ("lowercase_keys", LOWERCASE_KEYS),
]

# Whole-circuit (`ActiveCircuit.ToJSON`) combos — always captured with
# SkipTimestamp (the `! Last saved by …` stamp embeds the build revision + wall
# clock, so it can never be a deterministic golden). The circuit is ALWAYS pretty
# regardless of the Pretty bit, so `full_pretty` is the redundancy check.
CIRCUIT_COMBOS = [
    ("default", 0),
    ("full", FULL),
    ("full_pretty", FULL | PRETTY),
    ("skip_buses", SKIP_BUSES),
    ("include_default", INCLUDE_DEFAULT_OBJS),
    ("enum_as_int", ENUM_AS_INT),
]

# Each deck: a name, the command script (run after `clear`), and a list of
# captures. A capture is ("obj", "class.name") or ("batch", "ClassName"). For a
# transformer, the on-struct-array + WdgCurrents Full render is out of the Rust
# port's pre-solve reach, so Full-family combos are excluded there (see the
# `full_ok` per-deck flag / per-capture skip).
DECKS = [
    {
        "name": "load_micro",
        "commands": [
            "new circuit.probe basekv=12.47",
            "new load.l1 bus1=probe.1.2.3 kV=12.47 kW=10.5 pf=0.9 "
            "zipv=(1 0 0 1 0 0 0.95)",
            "new load.l2 bus1=probe kW=5 kvar=2 model=2 conn=delta",
        ],
        "captures": [("obj", "load.l1"), ("obj", "load.l2"), ("batch", "Load")],
    },
    {
        "name": "line_micro",
        # l1 exercises the LineCode ObjectRef (Name/FullName) + the ratings the
        # fetch marks set; l2 is a sym-component line so Full mode renders its
        # R1/X1 and its *computed* R/X/C matrices (nested-array rendering). An
        # explicit-matrix (matrix-model) line is a separate default-only deck,
        # since its Full render turns the sym scalars into JSON `null` (the
        # oracle's NaN getter), a Line-internal behavior deferred here.
        "commands": [
            "new circuit.probe basekv=12.47",
            "new linecode.lc1 nphases=3 r1=0.05 x1=0.1 c1=3 r0=0.15 x0=0.4",
            "new line.l1 bus1=a bus2=b linecode=lc1 length=1.2",
            "new line.l2 bus1=b bus2=c phases=3 "
            "r1=0.05 x1=0.1 r0=0.15 x0=0.4 c1=3 c0=1.5",
        ],
        "captures": [
            ("obj", "line.l1"),
            ("obj", "line.l2"),
            ("obj", "linecode.lc1"),
            ("batch", "Line"),
            ("batch", "LineCode"),
        ],
    },
    {
        "name": "line_matrix",
        # Explicit R/X/C matrices → matrix-model line (SymComponentsModel=false).
        # Default-family combos only: Full renders the sym scalars R1/X1/… as the
        # oracle's `null` (NaN getter under !SymComponentsModel), a Line-internal
        # detail not reproduced by the pre-solve Rust dump (recorded deferral).
        # DISTINCT diagonal entries (0.1 vs 0.11, 0.2 vs 0.22, 3 vs 3.3) so the
        # DoubleSymMatrix row/column indexing is pinned positionally — an equal-
        # diagonal fixture cannot catch a diagonal-index bug.
        "commands": [
            "new circuit.probe basekv=12.47",
            "new line.lm bus1=a bus2=b phases=2 "
            "rmatrix=(0.1 | 0.05 0.11) xmatrix=(0.2 | 0.1 0.22) "
            "cmatrix=(3 | -1 3.3)",
        ],
        "captures": [("obj", "line.lm"), ("batch", "Line")],
        "skip_full": True,
    },
    {
        "name": "vsource_micro",
        "commands": [
            "new circuit.probe basekv=115 bus1=sourcebus",
            "new vsource.v2 bus1=probe basekv=12.47 R1=1 X1=2 R0=3 X0=4",
        ],
        "captures": [
            ("obj", "vsource.source"),
            ("obj", "vsource.v2"),
            ("batch", "Vsource"),
        ],
    },
    {
        "name": "transformer_micro",
        # The redundancy deferral (kvs->kV, conns->Conn, buses->Bus, %rs->%R)
        # is exercised in default-family combos. The explicit per-winding
        # RDCOhms/MaxTap/MinTap/NumTaps/RNeut are the `ON_ARRAY` scalars with no
        # plural alternative: set here so the default sweep renders each as a
        # per-winding array — this pins the DoubleOnStructArray (`RDCOhms`…) and
        # IntegerOnStructArray (`NumTaps`) JSON arms against the oracle without
        # needing Full mode. Full-family combos are INCLUDED: the Rust JSON path
        # now refreshes Vterminal (`obj_to_json_mut`/`class_batch_to_json_mut`),
        # so the `WdgCurrents` result string renders exactly as the oracle's
        # self-refreshing getter does. This deck pins the PRE-SOLVE case (the
        # all-zero phasor list); the `transformer_solved` deck pins the NONZERO
        # post-solve case that actually exercises the refresh. Captured pre-solve,
        # so `voltagebases`/`calcv` give a well-formed (zeroed) vector to refresh from.
        "commands": [
            "new circuit.probe basekv=12.47",
            "new transformer.t1 windings=2 buses=(probe, b2) "
            "conns=(delta, wye) kvs=(12.47, 0.48) kvas=(1000, 1000) "
            "xhl=6 %rs=(0.5, 0.5) "
            "wdg=1 rdcohms=0.11 maxtap=1.1 mintap=0.9 numtaps=32 rneut=0.5 "
            "wdg=2 rdcohms=0.22 maxtap=1.2 mintap=0.8 numtaps=16 rneut=1.5",
            "makebuslist",
            "set voltagebases=[12.47,0.48]",
            "calcv",
        ],
        "captures": [("obj", "transformer.t1"), ("batch", "Transformer")],
    },
    {
        "name": "transformer_solved",
        # POST-SOLVE companion to `transformer_micro`: the headline §1.3 deliverable
        # is that the JSON-Full refresh route (`obj_to_json_mut`/`class_batch_to_json_mut`
        # → `refresh_vterminal_if_marked`) transfers the *current solution's* Vterminal
        # into the `WdgCurrents` string. Pre-solve the refresh is a proven no-op
        # (NodeV=0 → all-zeros, same string with or without the refresh call), so
        # `transformer_micro` alone cannot catch a refresh regression. Here the
        # transformer primary is on the energized `sourcebus` feeding a 500 kW load
        # and the deck `solve`s, so `WdgCurrents` is a NONZERO phasor list — a
        # regression that dropped the refresh (or mis-indexed Y_term·Vterminal) now
        # fails the gate. Byte-exact is valid despite the solve: the getter formats
        # at Pascal `%.7g`/`%.5g` (Transformer.pas:362), far coarser than the
        # faer-vs-KLU last-ULP, and Rust==oracle was verified bit-for-bit on this
        # deck. Full-family combos only (WdgCurrents renders only under Full).
        "commands": [
            "new circuit.probe basekv=12.47 bus1=sourcebus",
            "new transformer.t1 windings=2 buses=(sourcebus, b2) "
            "conns=(delta, wye) kvs=(12.47, 0.48) kvas=(1000, 1000) "
            "xhl=6 %rs=(0.5, 0.5) "
            "wdg=1 rdcohms=0.11 maxtap=1.1 mintap=0.9 numtaps=32 rneut=0.5 "
            "wdg=2 rdcohms=0.22 maxtap=1.2 mintap=0.8 numtaps=16 rneut=1.5",
            "new load.ld bus1=b2 kv=0.48 kw=500 pf=0.95",
            "set voltagebases=[12.47,0.48]",
            "solve",
        ],
        "captures": [("obj", "transformer.t1"), ("batch", "Transformer")],
        "combos": [
            ("full", FULL),
            ("full_pretty", FULL | PRETTY),
            ("full_include_class", FULL | INCLUDE_DSS_CLASS),
            ("full_skip_redundant", FULL | SKIP_REDUNDANT),
        ],
    },
    {
        "name": "autotrans_micro",
        # The AutoTrans twin of `transformer_micro` (og(json-a) item 1). AutoTrans
        # carries its own copy of the JSON array-alternative metadata
        # (`AutoTrans.pas:439-557`: bus/buses, conn/conns, kV/kVs, kVA/kVAs,
        # tap/taps, pctR/pctRs), so the DEFAULT sweep must render the SINGULAR
        # keys `Bus/Conn/kV/kVA/pctR` as per-winding arrays — a class that lost
        # the metadata would emit `Buses/Conns/kVs/kVAs` instead and fail here.
        # RDCOhms/MaxTap/MinTap/NumTaps are the `ON_ARRAY` scalars with no plural
        # alternative, set per winding with DISTINCT values so their positional
        # DoubleOnStructArray / IntegerOnStructArray indexing is pinned.
        # Three windings (series/wye/delta) so the AutoTrans-specific `series`
        # connection ordinal is in the sweep. Full-family combos INCLUDED: this
        # pins the PRE-SOLVE `WdgCurrents` (all-zero-ish phasor list); the
        # `autotrans_solved` deck pins the NONZERO post-solve case.
        "commands": [
            "new circuit.probe basekv=345 bus1=sourcebus",
            "New AutoTrans.a7 phases=3 windings=3 xhx=7.23 xht=24.45 xxt=28.45 "
            "%imag=0.0329 %noloadloss=0.02402",
            "~ wdg=1 bus=sourcebus conn=s kV=345.0 kVA=330000 %r=0.0493 "
            "rdcohms=0.11 maxtap=1.1 mintap=0.9 numtaps=32",
            "~ wdg=2 bus=low  conn=w kV=161.0 kVA=330000 %r=0.0556 "
            "rdcohms=0.22 maxtap=1.2 mintap=0.8 numtaps=16",
            "~ wdg=3 bus=tert conn=d kV=13.8  kVA=72000  %r=1.4503 "
            "rdcohms=0.33 maxtap=1.3 mintap=0.7 numtaps=8",
            "makebuslist",
            "set voltagebases=[345.0 161.0 13.8]",
            "calcv",
        ],
        "captures": [("obj", "AutoTrans.a7"), ("batch", "AutoTrans")],
    },
    {
        "name": "autotrans_solved",
        # POST-SOLVE AutoTrans companion (og(json-a) item 1). `transformer_solved`
        # proves the class-agnostic JSON Vterminal-refresh route, but AutoTrans has
        # its OWN series/common/delta winding-current getter
        # (`TAutoTransObj.GetAllWindingCurrents`, `auto_trans/yterminal.rs`), which
        # no JSON golden pinned before: every AutoTrans WdgCurrents capture was
        # pre-solve all-zeros, indistinguishable from a broken getter. Here the
        # series winding is fed from the energized `sourcebus` with loads on the
        # 161 kV common and the 13.8 kV tertiary, so Full `WdgCurrents` is a
        # NONZERO phasor list pinned byte-exact. Byte-exact is valid post-solve:
        # the getter formats at Pascal `%.7g`/`%.5g`, far coarser than the
        # faer-vs-KLU last-ULP. Full-family combos only (WdgCurrents renders only
        # under Full). NOTE: with `rdcohms`/`maxtap`/`mintap`/`numtaps` left at
        # their defaults, `RecalcElementData` derives RDCOhms from the winding
        # ratings, so this deck ALSO pins the computed-RDCOhms render.
        "commands": [
            "new circuit.probe basekv=345 bus1=sourcebus",
            "Edit Vsource.source basekv=345 pu=1.0 mvasc3=30000 25000",
            "New AutoTrans.a7 phases=3 windings=3 xhx=7.23 xht=24.45 xxt=28.45 "
            "%imag=0.0329 %noloadloss=0.02402",
            "~ wdg=1 bus=sourcebus conn=s kV=345.0 kVA=330000 %r=0.0493",
            "~ wdg=2 bus=low  conn=w kV=161.0 kVA=330000 %r=0.0556",
            "~ wdg=3 bus=tert conn=d kV=13.8  kVA=72000  %r=1.4503",
            "New Load.l161 bus1=low phases=3 kv=161 kw=250000 pf=0.88 model=1",
            "New Load.l138 bus1=tert phases=3 kv=13.8 kw=30000 pf=0.95 model=1",
            "set voltagebases=[345.0 161.0 13.8]",
            "calcvoltagebases",
            "set tolerance=1e-8",
            "solve",
        ],
        "captures": [("obj", "AutoTrans.a7"), ("batch", "AutoTrans")],
        "combos": [
            ("full", FULL),
            ("full_pretty", FULL | PRETTY),
            ("full_include_class", FULL | INCLUDE_DSS_CLASS),
            ("full_skip_redundant", FULL | SKIP_REDUNDANT),
        ],
    },
    {
        "name": "der_usermodel_full",
        # og(json-a) item 2: the Generator/PVSystem/Storage user-model string
        # properties under FULL mode. They used to carry `PropFlags::NOT_PORTED`
        # and were skipped from the Full sweep while the 0.14.5 oracle emits them
        # as `""`; WASM_USERMODELS WM.3/WM.4 made them fully ported properties and
        # `hidden_from_full_enum()` no longer consults NOT_PORTED at all, so they
        # must now render. This deck pins all six surfaces against the oracle:
        # Generator `UserModel`/`UserData`/`ShaftModel`/`ShaftData`, Storage
        # `UserModel`/`UserData` + `DynaDLL`/`DynaData`, PVSystem
        # `UserModel`/`UserData`. Left UNSET (the empty-string default) on purpose
        # — a non-empty value would name a `.wasm`/DLL and drag the loader into a
        # byte golden; the gap being closed is the *render*, not the load. Full
        # combos only for those (the default sweep omits unset strings, so it
        # cannot see them at all).
        #
        # The deck ALSO pins `Spectrum=` under FullNames, in BOTH sweeps. The
        # port resolves `Spectrum` outside the property machinery, so its PropDef
        # carries no resolving class and the FullNames render used to emit a bare
        # `mycustom` where Pascal (`PropertyOffset2 = SpectrumClass`,
        # `PCClass.pas:96-98`) emits `Spectrum.mycustom`. `MyCustom` is declared
        # in mixed case and assigned as `MYCUSTOM` to pin the lowercased stored
        # name at the same time.
        "commands": [
            "new circuit.der basekv=24 bus1=sourcebus",
            "new Spectrum.MyCustom numharm=1 harmonic=[1] %mag=[100] angle=[0]",
            "new Generator.g1 Bus1=sourcebus kV=24 kW=100 kvar=50 Model=1 "
            "spectrum=MYCUSTOM",
            "new Storage.st1 phases=3 bus1=sourcebus kv=24 kwrated=100 "
            "kwhrated=200",
            "new PVSystem.pv1 phases=3 bus1=sourcebus kv=24 kva=100 pmpp=100 "
            "irradiance=1",
            "set voltagebases=[24]",
            "calcv",
        ],
        "captures": [
            ("obj", "Generator.g1"),
            ("obj", "Storage.st1"),
            ("obj", "PVSystem.pv1"),
            ("batch", "Generator"),
            ("batch", "Storage"),
            ("batch", "PVSystem"),
        ],
        "combos": [
            ("full", FULL),
            ("full_pretty", FULL | PRETTY),
            ("full_include_class", FULL | INCLUDE_DSS_CLASS),
            ("full_skip_redundant", FULL | SKIP_REDUNDANT),
            ("full_fullnames", FULL | FULL_NAMES),
            ("full_lowercase_keys", FULL | LOWERCASE_KEYS),
            # Bit 3 alone — the DEFAULT sweep under FullNames, where the
            # `Spectrum.mycustom` render is also observable (an assigned,
            # non-default spectrum survives the default-sweep filter).
            ("fullnames", FULL_NAMES),
        ],
    },
    {
        "name": "dyneq_full",
        # FULL-mode companion to `dyneq_micro` (og(json-a) item 2 tail).
        # `dyneq_micro` is `skip_full` because the Full sweep used to drop the
        # Generator/Storage `ShaftModel`/`ShaftData` properties (the NOT_PORTED
        # gap closed by `der_usermodel_full`), so the `TDynEqPCE` "DynInit" tail
        # was never gated under Full. The tail is appended by the same code after
        # either sweep, but "same code" was the argument for leaving it unpinned
        # — this deck pins it directly instead. Same assignment mix as
        # `dyneq_micro` (plain constant → JSON number, calc-value operand and RPN
        # constant → JSON string, `Damp` written twice to pin the
        # `UserDynInit.Delete`+`Add` reorder), so a Full-only regression in the
        # tail now fails the gate.
        "commands": [
            "new circuit.dyn basekv=24 bus1=sourcebus",
            "new DynamicExp.gde nvariables=6 "
            "varnames=[Speed Mass PShaft Pterm Damp theta] "
            "expression=[Speed dt = -1 Mass / ( Pterm Damp Speed * + Pshaft - ) *;"
            " theta dt = Speed]",
            "new Generator.g1 Bus1=sourcebus kV=24 kW=100 kvar=50 Model=1 "
            "DynamicEq=gde",
            "~ Damp = 0 PShaft = P0 Pterm = P Speed = 0 theta = Edp "
            "Mass = (3.5 2 * 2220000000 376.99112 / *)",
            "~ Damp = (1 2 +)",
            "~ DynOut = [Speed theta]",
            "new DynamicExp.stde nvariables=2 varnames=[wp wq] "
            "expression=[wp dt = 0; wq dt = 0]",
            "new Storage.st1 phases=3 bus1=sourcebus kv=24 kwrated=100 "
            "kwhrated=200 DynamicEq=stde",
            "~ wp = 7 wq = (4 5 +)",
            "set voltagebases=[24]",
            "calcv",
        ],
        "captures": [
            ("obj", "Generator.g1"),
            ("obj", "Storage.st1"),
            ("batch", "Generator"),
            ("batch", "Storage"),
        ],
        "combos": [
            ("full", FULL),
            ("full_pretty", FULL | PRETTY),
            ("full_include_class", FULL | INCLUDE_DSS_CLASS),
            ("full_skip_redundant", FULL | SKIP_REDUNDANT),
            ("full_lowercase_keys", FULL | LOWERCASE_KEYS),
        ],
    },
    {
        "name": "dyneq_micro",
        # The `TDynEqPCE` "DynInit" tail (CAPI_Obj.pas:752-759): a
        # Generator/PVSystem/Storage carrying a DynamicExp with UserDynInit
        # assignments appends a literal "DynInit" object after all properties.
        # The assignment mix pins all three value renderings: a plain constant
        # (Speed=0 / wp=7) → JSON number; a calc-value operand (PShaft=P0,
        # Pterm=P, theta=Edp) → JSON string (case preserved); an RPN constant
        # (Mass=(...), wq=(4 5 +)) → JSON string. Generator exercises the
        # `self.dyneq` host, Storage the InvBasedPCE `base.dyneq` host. The
        # literal "DynInit" key must survive LowercaseKeys unchanged, and the
        # calc-value/RPN strings must be untouched by FullNames/EnumAsInt — so
        # the full combo sweep is captured (obj + batch, both classes).
        # `Damp` is assigned TWICE (first `= 0` → JSON number, then `= (1 2 +)`
        # → RPN string): this pins the dedup rewrite (Pascal `UserDynInit.Delete`
        # then `Add`, DynEqPCE.pas:161) — the second write both changes the value
        # type (number→string) AND moves `damp` to the END of the tail, past the
        # variables assigned once. (Speed=0 keeps a plain-number arm in the deck.)
        "commands": [
            "new circuit.dyn basekv=24 bus1=sourcebus",
            "new DynamicExp.gde nvariables=6 "
            "varnames=[Speed Mass PShaft Pterm Damp theta] "
            "expression=[Speed dt = -1 Mass / ( Pterm Damp Speed * + Pshaft - ) *;"
            " theta dt = Speed]",
            "new Generator.g1 Bus1=sourcebus kV=24 kW=100 kvar=50 Model=1 "
            "DynamicEq=gde",
            "~ Damp = 0 PShaft = P0 Pterm = P Speed = 0 theta = Edp "
            "Mass = (3.5 2 * 2220000000 376.99112 / *)",
            "~ Damp = (1 2 +)",
            "~ DynOut = [Speed theta]",
            "new DynamicExp.stde nvariables=2 varnames=[wp wq] "
            "expression=[wp dt = 0; wq dt = 0]",
            "new Storage.st1 phases=3 bus1=sourcebus kv=24 kwrated=100 "
            "kwhrated=200 DynamicEq=stde",
            "~ wp = 7 wq = (4 5 +)",
            "set voltagebases=[24]",
            "calcv",
        ],
        "captures": [
            ("obj", "Generator.g1"),
            ("obj", "Storage.st1"),
            ("batch", "Generator"),
            ("batch", "Storage"),
        ],
        # Default-family combos only: the DynInit tail is appended after both the
        # default and Full sweeps by the same code, so the default sweep fully
        # exercises it (number/string value split, the literal `"DynInit"` key
        # surviving LowercaseKeys, calc-strings untouched by FullNames/EnumAsInt).
        # Full mode is skipped here to avoid an unrelated Generator Full-render
        # gap (ShaftModel/ShaftData hidden by NOT_PORTED) — a recorded follow-up.
        "skip_full": True,
    },
    {
        "name": "ieee13_samples",
        # Sample real elements from the unmodified IEEE13 feeder. Compiled (the
        # master ends without a Solve; values are parsed inputs regardless).
        # Default-family combos only (`skip_full`): the transformer sample's Full
        # render exposes the on-struct-array + WdgCurrents solve-state path and a
        # matrix-model LineCode's Full render turns sym scalars to `null` — both
        # deferred (see the micro decks). Default/EnumAsInt/FullNames/Lowercase
        # exercise the set-order sweep, naming and ObjectRef paths on real data.
        "master": "Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss",
        "captures": [
            ("obj", "Line.650632"),
            ("obj", "Transformer.XFM1"),
            ("obj", "Load.671"),
            ("obj", "LineCode.mtx601"),
            ("batch", "Load"),
        ],
        "skip_full": True,
    },
    {
        "name": "batch_micro",
        # Pins two batch-only behaviors the single-object captures cannot reach:
        #  * ExcludeDisabled — the disabled `load.b` is dropped from the batch
        #    (Batch_ToJSON's ckt-element Enabled branch, CAPI_Obj.pas:1229-1240).
        #  * the empty-class batch — `Capacitor` has no objects, so the batch is
        #    `[]` (compact) / `[\r\n]` (pretty). This is the actual
        #    `IActiveClass.ToJSON` surface; it does NOT take the `batchSize=0 ->
        #    '[]'` literal shortcut, so pretty is genuinely `[\r\n]`.
        "commands": [
            "new circuit.probe basekv=12.47",
            "new load.a bus1=probe kV=12.47 kW=1",
            "new load.b bus1=probe kV=12.47 kW=2 enabled=no",
        ],
        "captures": [("batch", "Load"), ("batch", "Capacitor")],
        "combos": [
            ("default", 0),
            ("pretty", PRETTY),
            ("exclude_disabled", EXCLUDE_DISABLED),
            ("exclude_disabled_pretty", EXCLUDE_DISABLED | PRETTY),
        ],
    },
    {
        "name": "circuit_micro",
        # Whole-circuit dump: bus coordinates (X/Y), Keep, kVLN (post
        # CalcVoltageBases), open terminals (saveOpenTerminalsJSON: a whole
        # terminal + a single conductor), and class-ordering across LineCode /
        # Line / Vsource / Load. Every class here is Full-safe (sym-component
        # line, no transformer / matrix line / capacitor — the Stage-A + Stage-B
        # Full-render deferrals: transformer WdgCurrents, matrix-line sym scalars
        # → null, capacitor CMatrix computed under Full). Capacitor's default-mode
        # dump is covered by circuit_ieee13 instead.
        "commands": [
            "new circuit.probe basekv=12.47 bus1=sourcebus",
            "new linecode.lc1 nphases=3 r1=0.05 x1=0.1 c1=3 r0=0.15 x0=0.4",
            "new line.ln1 bus1=sourcebus bus2=b2 linecode=lc1 length=1.2",
            "new line.ln2 bus1=b2 bus2=b3 phases=3 r1=0.05 x1=0.1 r0=0.15 x0=0.4 c1=3 c0=1.5 length=0.8",
            "new load.l1 bus1=b3.1.2.3 kV=12.47 kW=10 pf=0.95",
            "makebuslist",
            "setbusxy bus=sourcebus x=100.5 y=-200.25",
            "set voltagebases=[12.47]",
            "calcvoltagebases",
            "set keep=[b2]",
            # A whole terminal open + a single conductor open.
            "open line.ln1 term=2",
            "open line.ln2 term=1 cond=2",
        ],
        "captures": [("circuit", "")],
        "combos": CIRCUIT_COMBOS,
    },
    {
        "name": "circuit_edited_default",
        # DefaultAndUnedited: editing one default object (spectrum.defaultload)
        # clears its flag, so it — and only it — rejoins the default dump, while
        # the other 6 spectra + the default LoadShape/GrowthShape/TCC_Curves stay
        # hidden. IncludeDefaultObjs then brings them all back.
        "commands": [
            "new circuit.probe basekv=12.47",
            "edit spectrum.defaultload %mag=(100 1.5 20 14 1 9 7)",
            "makebuslist",
        ],
        "captures": [("circuit", "")],
        "combos": [("default", 0), ("include_default", INCLUDE_DEFAULT_OBJS)],
    },
    {
        "name": "circuit_ieee13",
        # The unmodified IEEE13 feeder, whole-circuit. Full is excluded
        # (`skip_full`): IEEE13 has a transformer (WdgCurrents Full solve-state
        # deferral) and matrix-model LineCodes (sym scalars → `null` under Full),
        # both Stage-A deferrals. Default / SkipBuses / IncludeDefaultObjs /
        # EnumAsInt exercise the bus array, the class sweep and the naming on real
        # data.
        "master": "Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss",
        "captures": [("circuit", "")],
        "combos": [
            ("default", 0),
            ("skip_buses", SKIP_BUSES),
            ("include_default", INCLUDE_DEFAULT_OBJS),
            ("enum_as_int", ENUM_AS_INT),
        ],
    },
    {
        "name": "circuit_positive_seq",
        # Pins the three PreCommands conditionals that no other circuit deck
        # reaches: the `CktModel=` empty-value TODO(compat) quirk (positive
        # sequence → Pascal `OrdinalToString(Integer(True)=-1)` = ''),
        # `AllowDuplicates=True` and `LongLineCorrection=True`. Also sets fractional
        # ueweight/lossweight at `%8.2f` rounding boundaries (0.125→0.13,
        # 2.675→2.68) to pin the FPC-faithful fixed formatter (15-sig intermediate
        # + ties-away rounding) that the default weight 1.0 elsewhere cannot catch.
        # SkipBuses keeps the golden focused on the Pre/PostCommands.
        "commands": [
            "new circuit.probe basekv=12.47",
            "set cktmodel=positive",
            "set allowduplicates=yes",
            "set longlinecorrection=yes",
            "set ueweight=0.125",
            "set lossweight=2.675",
            "makebuslist",
        ],
        "captures": [("circuit", "")],
        "combos": [("skip_buses", SKIP_BUSES)],
    },
    {
        "name": "escape_micro",
        # A bus value carrying a quote, a slash and a backslash — pins fpjson
        # StringToJSON escaping end-to-end (`"`->\", `\`->\\, `/` unescaped).
        "commands": [
            "new circuit.probe basekv=12.47",
            r"""new load.esc bus1='a"b/c\d' kV=12.47 kW=1""",
        ],
        "captures": [("obj", "load.esc")],
    },
]


def check_pin() -> str:
    import dss

    pins = {}
    for line in (REPO_ROOT / "tools" / "golden" / "PIN.txt").read_text().splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            k, v = line.split("==")
            pins[k] = v
    if dss.__version__ != pins["dss-python"]:
        sys.exit(
            f"dss-python {dss.__version__} != pinned {pins['dss-python']}; "
            f"install the pinned version or update PIN.txt deliberately"
        )
    return dss.__version__


def capture_obj(d, au, lib, target: str, bits: int) -> str:
    # `SetActiveElement` only activates *circuit* elements; a general DSS object
    # (LineCode, LoadShape, …) needs the class-then-name activation, which also
    # works for circuit elements. So use it uniformly.
    cls, name = target.split(".", 1)
    d.ActiveCircuit.SetActiveClass(cls)
    d.ActiveCircuit.ActiveClass.Name = name
    return au.get_string(lib.DSSElement_ToJSON(bits))


def capture_batch(d, cls: str, bits: int) -> str:
    d.ActiveCircuit.SetActiveClass(cls)
    return d.ActiveCircuit.ActiveClass.ToJSON(bits)


def capture_circuit(d, bits: int) -> str:
    # Always SkipTimestamp (the `! Last saved by …` stamp is non-deterministic).
    return d.ActiveCircuit.ToJSON(bits | SKIP_TIMESTAMP)


def run_deck(d, au, lib, deck: dict) -> dict:
    d.Text.Command = "clear"
    master = deck.get("master")
    if master is not None:
        master_abs = (
            REPO_ROOT / "tests" / "corpus" / "electricdss-tst" / master
        ).resolve()
        d.Text.Command = f'compile "{master_abs}"'
    for cmd in deck.get("commands", []):
        d.Text.Command = cmd

    skip_full = deck.get("skip_full", False)
    combos = deck.get("combos", COMBOS)
    captures = []
    for kind, target in deck["captures"]:
        for combo_name, bits in combos:
            if skip_full and (bits & FULL):
                continue
            if kind == "obj":
                text = capture_obj(d, au, lib, target, bits)
            elif kind == "circuit":
                text = capture_circuit(d, bits)
            else:
                text = capture_batch(d, target, bits)
            # The circuit capture always sets SkipTimestamp; record the effective
            # bits so the Rust driver replays with the identical option set.
            eff_bits = bits | SKIP_TIMESTAMP if kind == "circuit" else bits
            captures.append(
                {
                    "kind": kind,
                    "target": target,
                    "opts": combo_name,
                    "bits": eff_bits,
                    "expected": text,
                }
            )
    # Declare the combos this deck actually generates, so the Rust driver can
    # assert every (kind,target) carries the full set — a silently dropped combo
    # (a generator regression) then fails the gate instead of shrinking coverage.
    combo_names = [
        name for name, bits in combos if not (skip_full and (bits & FULL))
    ]
    out = {
        "name": deck["name"],
        "commands": deck.get("commands", []),
        "combo_names": combo_names,
        "captures": captures,
    }
    if master is not None:
        out["master"] = master
    return out


def main() -> None:
    oracle_version = check_pin()
    from dss import dss as d

    au = d._api_util
    lib = au.lib

    # Optional deck-name filter: `python gen_json.py autotrans_micro …` writes
    # ONLY those goldens. Adding a deck must not rewrite the existing byte
    # goldens (an unrelated oracle/toolchain drift would silently ride along),
    # so a targeted regen is the default workflow for new decks; pass no
    # arguments for the full deliberate regeneration.
    only = set(sys.argv[1:])
    if only:
        known = {deck["name"] for deck in DECKS}
        unknown = sorted(only - known)
        if unknown:
            sys.exit(f"unknown deck(s): {', '.join(unknown)}")

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for deck in DECKS:
        if only and deck["name"] not in only:
            continue
        out = run_deck(d, au, lib, deck)
        out["schema"] = SCHEMA
        out["oracle"] = {"dss_python": oracle_version, "engine": d.Version}
        path = OUT_DIR / f"{deck['name']}.json"
        path.write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8")
        print(f"wrote {path} ({len(out['captures'])} captures)")


if __name__ == "__main__":
    main()
