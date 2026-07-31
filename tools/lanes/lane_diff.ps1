<#
.SYNOPSIS
  DE_PASCALIZE Stage F parity<->default differential gate (plan Part IV.2,
  "Two validation lanes ... + the differential gate"; step F.5).

.DESCRIPTION
  The two lanes are two builds, so no `#[test]` can compare them: a test binary
  only ever contains one of them. This job builds both, runs the corpus
  checkpoint dump on each, and diffs the two record streams against the
  documented per-kernel bounds (see crates/dss-core/examples/lane_dump.rs for
  what is compared and against what).

  Since the parity lane is bit-identical to the pinned oracle, `default ~=
  parity` measured here IS the transitive proof `default ~= oracle` -- and it is
  sharper than the oracle comparison, whose floors are the calibrated 1e-6-class
  tiers.

  Each lane builds into its own target directory so that re-running the job does
  not force a full rebuild of the other lane (a feature change otherwise
  invalidates dss-core, dss-parser and dss-sparse every time).

.PARAMETER OutDir
  Where the dumps and the two lane target directories live. Default
  `target/lanes` -- inside the gitignored build output, never in the source
  tree.

.PARAMETER SkipDump
  Re-run only the comparison over the dumps already in `OutDir`.

.EXAMPLE
  pwsh -File tools/lanes/lane_diff.ps1

.NOTES
  Windows/pwsh, like the other build jobs under tools/. Run from the repository
  root. Exit code 0 = both lanes agree inside the bounds.
#>
[CmdletBinding()]
param(
    [string]$OutDir = "target/lanes",
    [switch]$SkipDump
)

$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
Set-Location $root

$out = Join-Path $root $OutDir
New-Item -ItemType Directory -Force -Path $out | Out-Null
$defaultDump = Join-Path $out "default.dump"
$parityDump = Join-Path $out "parity.dump"

function Invoke-Lane {
    param(
        [string]$Lane,
        [string[]]$Features,
        [string]$DumpPath
    )
    $targetDir = Join-Path $out "$Lane-target"
    $cargo = @(
        "run", "--release", "-q",
        "-p", "dss-core",
        "--example", "lane_dump",
        "--target-dir", $targetDir
    )
    foreach ($f in $Features) { $cargo += @("--features", $f) }
    $cargo += @("--", "dump", $DumpPath)

    Write-Host "== $Lane lane =============================================="
    Write-Host "cargo $($cargo -join ' ')"
    $sw = [Diagnostics.Stopwatch]::StartNew()
    & cargo @cargo
    if ($LASTEXITCODE -ne 0) { throw "$Lane lane dump failed (exit $LASTEXITCODE)" }
    $sw.Stop()
    $size = [math]::Round((Get-Item $DumpPath).Length / 1MB, 1)
    Write-Host ("{0} lane: {1} MB in {2:n0} s" -f $Lane, $size, $sw.Elapsed.TotalSeconds)
}

if (-not $SkipDump) {
    Invoke-Lane -Lane "default" -Features @() -DumpPath $defaultDump
    Invoke-Lane -Lane "parity" -Features @("dss-core/oracle-parity") -DumpPath $parityDump
}
foreach ($p in @($defaultDump, $parityDump)) {
    if (-not (Test-Path $p)) { throw "missing dump $p (run without -SkipDump)" }
}

Write-Host "== diff ===================================================="
& cargo run --release -q -p dss-core --example lane_dump `
    --target-dir (Join-Path $out "default-target") -- `
    diff $defaultDump $parityDump
$verdict = $LASTEXITCODE

# Corpus hygiene. The decks write their reports next to themselves, so a corpus
# run leaves artifacts behind exactly as `cargo test` does, and `CLAUDE.md`
# requires `git status --short tests/corpus` to be empty afterwards. Two shapes,
# both handled by exact path (never a wide `git clean`):
#
#   ?? untracked  -- a report or a whole `DI_yr_*` directory the deck created
#                    (the known Test/AutoTrans set, the EnergyMeter DI files):
#                    deleted.
#   " M" tracked  -- a deck whose own output file is *vendored* under that name
#                    (`Test/LineConstantsCode.DSS` from `Show LineConstants`,
#                    the two `IEEE_519_Mon_mpcc_1.csv` monitor exports):
#                    restored from git.
#
# A recursive delete is used for the created directories, and only after
# checking the entry is not a reparse point -- `.inputs`/`.venv` junctions live
# elsewhere in a worktree, and following one deletes main's copy (CLAUDE.md,
# "Git worktrees").
$dirty = & git status --porcelain -- tests/corpus
if ($dirty) {
    Write-Host "== corpus artifacts left by the run ========================"
    $restore = @()
    foreach ($line in $dirty) {
        $state = $line.Substring(0, 2).Trim()
        $path = $line.Substring(3).Trim('"')
        $full = Join-Path $root $path
        if ($state -eq "??") {
            $item = Get-Item -LiteralPath $full -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                Write-Warning "  reparse point under tests/corpus, NOT touched: $path"
            }
            elseif ($item.PSIsContainer) {
                Write-Host "  deleting created directory $path"
                Remove-Item -LiteralPath $full -Recurse -Force
            }
            else {
                Write-Host "  deleting untracked $path"
                Remove-Item -LiteralPath $full -Force
            }
        }
        elseif ($state -eq "M") {
            Write-Host "  restoring overwritten $path"
            $restore += $path
        }
        else {
            Write-Warning "  unexpected corpus state '$state' on $path (NOT touched)"
        }
    }
    if ($restore) { & git restore -- $restore }
}

exit $verdict
