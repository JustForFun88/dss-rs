"""Harvest A-Diakoptics matrix references from an official EPRI OpenDSS revision
(default r3723) for the committed-fixture gate (plan D9 b/c, WP-AD.3).

For a named deck + explicit manual link-branch cut, drives the official DLL via
the Oddie bridge (wait after every solve) and exports `ZLL`/`ZCC`/`Y4` plus the
post-AD SOLVED node voltages (`voltages.csv` — actor-1 NodeV after the AD stitch),
copying them into a committed reference directory with a `PROVENANCE.txt`.
Regenerate manually (the goldens discipline) — never in `cargo test`.

Why fresh harvest, not the trunk's own `References/SolveDirect/ADiakoptics_matrixes/`
CSVs: those committed CSVs were generated from an OLDER IEEE-13 deck revision and
no longer match the vendored deck (their reactance is ~20% off; the live engine on
the CURRENT deck agrees with the Rust port bit-for-bit — verified 2026-07-12).

Usage:
  python tools/opendss/gen_ad_reference.py \
     --rev r3723 --deck <IEEE_13_Bus dir> --linecodes <IEEETestCases/IEEELineCodes.DSS> \
     --link Line.670671 --out <committed ref dir>
"""
from __future__ import annotations
import argparse, hashlib, shutil, tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--rev", default="r3723")
    ap.add_argument("--deck", required=True, help="dir with the deck")
    ap.add_argument("--master", default="IEEE13Nodeckt.dss")
    ap.add_argument("--busxy", default="IEEE13Node_BusXY.csv")
    ap.add_argument("--linecodes", default=None, help="real IEEELineCodes.DSS content (flat mode)")
    ap.add_argument("--link", required=True,
                    help="comma-separated manual link branches, e.g. 'Line.l10, Line.l73'")
    ap.add_argument("--tree", action="store_true",
                    help="copy the WHOLE deck dir (decks with redirects, e.g. IEEE_123_Bus-G)")
    ap.add_argument("--out", required=True, help="committed reference dir")
    args = ap.parse_args()

    import json
    revs = json.loads((HERE / "revisions.json").read_text())
    dll = str((REPO / revs[args.rev]["dll"]).resolve())

    links = [x.strip() for x in args.link.split(",") if x.strip()]
    deck = Path(args.deck).resolve()
    out = Path(args.out).resolve()  # BEFORE the engine chdir's during compile/AD
    tmp = Path(tempfile.mkdtemp(prefix="gen_ad_ref_"))
    # Trim the master's own AD/solve tail so we drive the cut ourselves
    # (deterministic manual links, identical partition on both engines).
    if args.tree:
        # Deck with subdirectory redirects (IEEE_123_Bus-G): copy the whole tree.
        shutil.copytree(deck, tmp, dirs_exist_ok=True)
        master = tmp / args.master
    else:
        shutil.copy(deck / args.master, tmp / "master.dss")
        shutil.copy(deck / args.busxy, tmp / args.busxy)
        if args.linecodes:
            shutil.copy(Path(args.linecodes).resolve(), tmp / "IEEELineCodes.DSS")
        master = tmp / "master.dss"
    # Trim the master at its "A-Diakoptics part" marker so we drive the cut
    # ourselves (deterministic manual links). Both example masters carry the
    # marker; everything after it (Num_SubCircuits/ADiakoptics + the multi-actor
    # ActiveActor/CPU/yearly demo tail) is the deck's own scripted cases — dropping
    # it keeps only the clean base network + base solve on BOTH engines.
    kept = []
    for ln in master.read_text(errors="replace").splitlines():
        if "A-Diakoptics part" in ln:
            break
        kept.append(ln)
    master.write_text("\n".join(kept) + "\n")

    from dss import IOddieDSS
    d = IOddieDSS(library_path=dll)
    d.AllowForms = False
    d.Text.Command = "Set Editor=rundll32.exe"
    ver = str(d.Version)

    def C(s):
        d.Text.Command = s

    C("ClearAll")
    C(f'compile "{master}"')
    C(f'set datapath="{tmp}"')
    C("solve")
    C("wait")
    C(f"set LinkBranches = [{', '.join(links)}]")
    C("set UseMyLinkBranches=True")
    C("set ADiakoptics=True")
    C("wait")
    C("get ADiakoptics")
    if "yes" not in d.Text.Result.lower():
        raise SystemExit(f"AD init failed on {args.rev}: {d.Text.Result}")
    for m in ("ZLL", "ZCC", "Y4"):
        C(f"export {m}")

    # Post-AD SOLVED node voltages: solve the interconnected coordinator through
    # the AD stitch, then read the actor-1 NodeV (AllNodeNames/AllBusVolts). This
    # is the trusted baseline for the AD *solve* (not just the setup matrices).
    C("solve")
    C("wait")
    names = list(d.ActiveCircuit.AllNodeNames)
    volts = list(d.ActiveCircuit.AllBusVolts)  # interleaved re, im per node

    out.mkdir(parents=True, exist_ok=True)
    torn = tmp / "Torn_Circuit"
    prov = [f"engine: {ver}", f"rev: {args.rev}", f"deck: {deck}",
            f"link: {args.link}", "harvested by tools/opendss/gen_ad_reference.py"]
    for m in ("ZLL", "ZCC", "Y4"):
        src = next(torn.glob(f"*{m}*.csv"), None) or next(torn.glob(f"*{m}*.CSV"), None)
        if src is None:
            raise SystemExit(f"{m} not exported")
        dst = out / f"{m.lower()}.csv"
        shutil.copy(src, dst)
        digest = hashlib.sha256(dst.read_bytes()).hexdigest()[:16]
        prov.append(f"{m.lower()}.csv sha256[:16]={digest}")
        print(f"  {m} -> {dst}")

    vdst = out / "voltages.csv"
    vlines = ["Node,Re,Im"]
    for i, nm in enumerate(names):
        # full-precision f64 repr, plain float (Oddie hands back numpy scalars)
        vlines.append(f"{nm.lower()},{float(volts[2 * i])!r},{float(volts[2 * i + 1])!r}")
    vdst.write_text("\n".join(vlines) + "\n")
    vdigest = hashlib.sha256(vdst.read_bytes()).hexdigest()[:16]
    prov.append(f"voltages.csv nodes={len(names)} sha256[:16]={vdigest}")
    print(f"  voltages ({len(names)} nodes) -> {vdst}")

    (out / "PROVENANCE.txt").write_text("\n".join(prov) + "\n")
    print("provenance:", out / "PROVENANCE.txt")


if __name__ == "__main__":
    main()
