program mtwist_probe;
{$mode objfpc}{$H+}
{
  FPC 3.2.2 RTL RNG probe — pins the Rust port of the FPC Mersenne-Twister
  (`support/mathutil/rng.rs` `FpcRng`) + `Gauss`/`QuasiLognormal`
  (`support/mathutil` `gauss`/`quasi_log_normal`) against the REAL FPC 3.2.2
  RTL, the same compiler/RTL the pinned oracle's dss_capi backend is built with.

  Self-contained: the MT core uses only the RTL `Random` (identical to dss_capi),
  and `Gauss`/`QuasiLogNormal` are copied VERBATIM from
  `.inputs/dss_capi/src/Shared/mathutil.pas:292` and `:304` so the probe compiles
  with no dss_capi unit dependency (the verbatim copy was confirmed to produce
  bit-identical output to the real `Mathutil` unit).

  The RNG output is nondeterministic in the engine (time-seeded per process,
  GAPS_PLAN.md 2.1), so it is gated ONLY by the Rust fixed-seed unit tests in
  rng.rs. This probe reproduces those exact expected sequences from a fixed
  RandSeed.

  Build/run — MUST target x86_64-win64 (SSE2), matching the oracle's backend;
  i386-win32 (x87, 80-bit intermediates) diverges on Gauss/QuasiLognormal:
    ppcrossx64 -O2 mtwist_probe.pas
    ./mtwist_probe.exe          # compare against fpc_output_x86_64_win64.txt

  Prints each Double as its raw 64-bit little-endian pattern (hex) so the
  comparison is bit-exact, matching the `f64::to_bits()` assertions in rng.rs.
  Last captured & confirmed: 2026-07-08 (all 20 pins matched rng.rs).
}

uses
  SysUtils, Math;

function DblBits(d: Double): QWord;
begin
  DblBits := QWord(Pointer(@d)^);
end;

{ mathutil.pas:292 — verbatim }
function Gauss(Mean, StdDev: Double): Double;
var
  i: Integer;
  A: Double;
begin
  A := 0.0;
  for i := 1 to 12 do
    A := A + Random;
  Result := (A - 6.0) * StdDev + Mean;
end;

{ mathutil.pas:304 — verbatim }
function QuasiLogNormal(Mean: Double): Double;
begin
  Result := exp(Gauss(0.0, 1.0)) * Mean;
end;

var
  i: Integer;
  d: Double;

begin
  { --- raw u32, seed 12345 (Round(Random*2^32) is exact: Random = u32/2^32) --- }
  RandSeed := 12345;
  Write('U32 seed=12345 :');
  for i := 1 to 8 do
    Write(' ', LongWord(Round(Random * 4294967296.0)));
  WriteLn;

  { --- raw u32, seed 1 (first draw only) --- }
  RandSeed := 1;
  WriteLn('U32 seed=1 : ', LongWord(Round(Random * 4294967296.0)));

  { --- Random:Double bit patterns, seed 12345 --- }
  RandSeed := 12345;
  Write('DBL seed=12345 :');
  for i := 1 to 8 do
  begin
    d := Random;
    Write(' 0x', LowerCase(HexStr(DblBits(d), 16)));
  end;
  WriteLn;

  { --- Gauss(0,1) x4, seed 12345 --- }
  RandSeed := 12345;
  Write('G01 seed=12345 :');
  for i := 1 to 4 do
  begin
    d := Gauss(0.0, 1.0);
    Write(' 0x', LowerCase(HexStr(DblBits(d), 16)));
  end;
  WriteLn;

  { --- Gauss(2.5,0.5) x2, seed 12345 --- }
  RandSeed := 12345;
  Write('G25 seed=12345 :');
  for i := 1 to 2 do
  begin
    d := Gauss(2.5, 0.5);
    Write(' 0x', LowerCase(HexStr(DblBits(d), 16)));
  end;
  WriteLn;

  { --- QuasiLognormal(3.0) x2, seed 12345 --- }
  RandSeed := 12345;
  Write('QLN seed=12345 :');
  for i := 1 to 2 do
  begin
    d := QuasiLogNormal(3.0);
    Write(' 0x', LowerCase(HexStr(DblBits(d), 16)));
  end;
  WriteLn;
end.
