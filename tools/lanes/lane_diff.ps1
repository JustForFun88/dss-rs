<#
.SYNOPSIS
  The parity<->default differential job: builds both lanes, dumps the corpus
  checkpoints on each and diffs the two dumps.

.DESCRIPTION
  The two lanes are two builds, so no `#[test]` can compare them: a test binary
  only ever contains one of them. This job builds both, runs the corpus
  checkpoint dump on each, and diffs the two record streams against the
  documented per-kernel bounds (see crates/dss-core/examples/lane_dump.rs for
  what is compared and against what).

  The parity lane is compared with the OpenDSS engines at the calibrated
  tiers, so `default ~= parity` measured here carries that standing over to the
  default lane. Its bound is sharper than those 1e-6-class tiers.

  Run it after a change to a compat kernel, dss-sparse or the solution module.
  It is not part of the gate's test run: it costs two release builds and writes
  a few hundred MB under `OutDir`.

  Both cargo calls go through the build queue (tools/gate/gatelock.py, label
  `lane_diff`). The dumps hold the build slot while they solve the corpus.

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

# Paths under `tests/corpus` that were ALREADY dirty before this run — an
# in-flight edit to `ledger.json`, a manifest, `population.lock.json`. The
# cleanup below must never touch them: it exists to undo what the *dumps* wrote,
# and a blind `git restore` over everything dirty silently discards the working
# tree the job was invoked to validate.
$preDirty = @{}
foreach ($line in (& git status --porcelain -- tests/corpus)) {
    $preDirty[$line.Substring(3).Trim('"')] = $line.Substring(0, 2).Trim()
}

# Every cargo call goes through the build/test queue of the shared machine.
$gatelock = Join-Path $root "tools/gate/gatelock.py"

# The fetched OpenDSS test suite (gitignored) is brought to its pin before the
# dumps, and again after them to undo what the decks wrote into it.
$vendor = Join-Path $root "tools/corpus/vendor.py"
$null | & python $vendor
if ($LASTEXITCODE -ne 0) { throw "tools/corpus/vendor.py failed (exit $LASTEXITCODE)" }

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
    & python $gatelock --label lane_diff -- cargo @cargo
    if ($LASTEXITCODE -ne 0) { throw "$Lane lane dump failed (exit $LASTEXITCODE)" }
    $sw.Stop()
    $size = [math]::Round((Get-Item $DumpPath).Length / 1MB, 1)
    Write-Host ("{0} lane: {1} MB in {2:n0} s" -f $Lane, $size, $sw.Elapsed.TotalSeconds)
}

if (-not $SkipDump) {
    Invoke-Lane -Lane "default" -Features @() -DumpPath $defaultDump
    Invoke-Lane -Lane "parity" -Features @("dss-compat/opendss-parity") -DumpPath $parityDump
}
foreach ($p in @($defaultDump, $parityDump)) {
    if (-not (Test-Path $p)) { throw "missing dump $p (run without -SkipDump)" }
}

Write-Host "== diff ===================================================="
& python $gatelock --label lane_diff -- cargo run --release -q -p dss-core --example lane_dump `
    --target-dir (Join-Path $out "default-target") -- `
    diff $defaultDump $parityDump
$verdict = $LASTEXITCODE

# Corpus hygiene. The decks write their reports next to themselves, so a corpus
# run leaves artifacts behind. In the fetched suite `tools/corpus/vendor.py`
# undoes them (below). In the tracked deck families `git status --short
# tests/corpus` must be empty afterwards. Two shapes there,
# both handled by exact path (never a wide `git clean`):
#
#   ?? untracked  -- a report or a whole `DI_yr_*` directory the deck created:
#                    deleted.
#   " M" tracked  -- a deck whose own output file is committed under that
#                    name: restored from git.
#
# A recursive delete is used for the created directories, and only after
# checking the entry is not a reparse point -- `.inputs`/`.venv` junctions live
# elsewhere in a worktree, and following one deletes main's copy.
$dirty = & git status --porcelain -- tests/corpus
if ($dirty) {
    Write-Host "== corpus artifacts left by the run ========================"
    $restore = @()
    foreach ($line in $dirty) {
        $state = $line.Substring(0, 2).Trim()
        $path = $line.Substring(3).Trim('"')
        $full = Join-Path $root $path
        if ($preDirty.ContainsKey($path)) {
            Write-Host "  pre-existing working-tree change, NOT touched: $path"
        }
        elseif ($state -eq "??") {
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
Write-Host "== fetched corpus =========================================="
$null | & python $vendor
if ($LASTEXITCODE -ne 0) { Write-Warning "tools/corpus/vendor.py failed (exit $LASTEXITCODE)" }

exit $verdict
