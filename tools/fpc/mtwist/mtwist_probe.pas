program mtwist_probe;
{
  FPC 3.2.2 RTL RNG probe — pins the Rust port of the FPC Mersenne-Twister
  (`support/mathutil/rng.rs` `FpcRng`) + `Gauss`/`QuasiLognormal`
  (`support/mathutil` `gauss`/`quasi_log_normal`) against the REAL FPC 3.2.2
  RTL, the same compiler/RTL the pinned oracle's dss_capi backend is built with.

  The RNG output is nondeterministic in the engine (time-seeded per process,
  GAPS_PLAN.md §2.1), so it is gated ONLY by the Rust fixed-seed unit tests in
  rng.rs. This probe reproduces those exact expected sequences from a fixed
  RandSeed, so the pins are regenerable on any FPC 3.2.2 x86_64-win64 host.

  Build/run (same discipline as tools/fpc/fmt_battery):
    ppcrossx64 -O2 mtwist_probe.pas
    ./mtwist_probe.exe

  Expected output (all values consumed by rng.rs::tests):
    U32   seed=12345 : 3992670690 3823185381 1358822685 561383553
                       789925284 170765737 878579710 3549516158
    U32   seed=1     : 1791095845 (first draw)
    DBL   seed=12345 : Random x8 (hex bit patterns 0x3fedbf6a3c400000 ...)
    G01   seed=12345 : Gauss(0,1) x4
    G25   seed=12345 : Gauss(2.5,0.5) x2
    QLN   seed=12345 : QuasiLognormal(3.0) x2

  Prints each Double as its raw 64-bit little-endian pattern (hex) so the
  comparison is bit-exact, matching the `f64::to_bits()` assertions in rng.rs.
}

uses
  SysUtils, Math, Mathutil;   { Mathutil exports Gauss / QuasiLogNormal }

function DblBits(d: Double): QWord;
begin
  DblBits := QWord(Pointer(@d)^);
end;

var
  i: Integer;
  d: Double;

begin
  { NOTE: the RTL exposes Random:Double, not the raw u32. To pin the raw u32
    sequence, reconstruct it as Round(Random * 2^32) — exact because
    Random = u32 / 2^32 with u32 < 2^53. The Rust test pins u32 directly and
    the doubles below pin Random itself. }

  { --- raw u32, seed 12345 --- }
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
    Write(' 0x', HexStr(DblBits(d), 16));
  end;
  WriteLn;

  { --- Gauss(0,1) x4, seed 12345 --- }
  RandSeed := 12345;
  Write('G01 seed=12345 :');
  for i := 1 to 4 do
  begin
    d := Gauss(0.0, 1.0);
    Write(' 0x', HexStr(DblBits(d), 16));
  end;
  WriteLn;

  { --- Gauss(2.5,0.5) x2, seed 12345 --- }
  RandSeed := 12345;
  Write('G25 seed=12345 :');
  for i := 1 to 2 do
  begin
    d := Gauss(2.5, 0.5);
    Write(' 0x', HexStr(DblBits(d), 16));
  end;
  WriteLn;

  { --- QuasiLognormal(3.0) x2, seed 12345 --- }
  RandSeed := 12345;
  Write('QLN seed=12345 :');
  for i := 1 to 2 do
  begin
    d := QuasiLogNormal(3.0);
    Write(' 0x', HexStr(DblBits(d), 16));
  end;
  WriteLn;
end.
