program single_prec_probe;
{ FPC 3.2.2 x86_64 probe (compile with ppcrossx64): pins the EXACT mixed
  f32/f64 rounding of the Pascal LoadShape single-precision paths the Rust
  port mirrors — GetMultAtHourSingle's interpolation expression
  (LoadShape.pas:2357) and RCDMeanAndStdDevSingle / CurveMeanAndStdDevSingle
  (Shared/mathutil.pas:336/390). Prints each result's f64 bit pattern; the
  values are asserted verbatim in
  crates/dss-core/src/elements/general/load_shape/tests.rs
  (sng_single_storage_matches_fpc_bit_exact). Probe run 2026-07-07:
    interp   = 3FD695F8134B71D8
    rcd_mean = 3FE1E06526000000   rcd_sd = 3FD9ADD3C0000000
    cms_mean = 3FE355E8807CF518   cms_sd = 3FD60240860CE49D }
{$mode objfpc}
uses SysUtils;
type TS4 = array[1..4] of Single;
var
  sH, sP: TS4;
  Hr, res, Mean, StdDev: Double;
  S: Single;
  sD: Double;
  i: Integer;
  q: QWord;

procedure PB(const tag: string; v: Double);
var qq: QWord absolute v;
begin
  writeln(tag, '=', IntToHex(qq, 16));
end;

// RCDMeanAndStdDevSingle replica (S: Single accumulator)
procedure RCDS(const D: TS4; N: Integer; var M, SD: Double);
var S: Single; i: Integer;
begin
  M := 0.0;
  for i := 1 to N do M := M + D[i];
  M := M / N;
  S := 0;
  for i := 1 to N do S := S + Sqr(M - D[i]);
  SD := Sqrt(S / (N - 1));
end;

// CurveMeanAndStdDevSingle replica (locals Double)
procedure CMS(const pY, pX: TS4; N: Integer; var M, SD: Double);
var s, dy1, dy2: Double; i: Integer;
begin
  s := 0;
  for i := 1 to N - 1 do
    s := s + 0.5 * (pY[i] + pY[i + 1]) * (pX[i + 1] - pX[i]);
  M := s / (pX[N] - pX[1]);
  s := 0;
  for i := 1 to N - 1 do
  begin
    dy1 := (pY[i] - M); dy2 := (pY[i + 1] - M);
    s := s + 0.5 * (dy1 * dy1 + dy2 * dy2) * (pX[i + 1] - pX[i]);
  end;
  SD := Sqrt(s / (pX[N] - pX[1]));
end;

begin
  sH[1] := 0.1; sH[2] := 2.3; sH[3] := 4.7; sH[4] := 8.9;
  sP[1] := 1.0/3.0; sP[2] := 0.123456789; sP[3] := 0.777777777; sP[4] := 0.999999999;
  // GetMultAtHourSingle interpolation expression, Hr between points 2 and 3
  Hr := 3.14159265358979;
  res := sP[2] + (Hr - sH[2]) / (sH[3] - sH[2]) * (sP[3] - sP[2]);
  PB('interp', res);
  RCDS(sP, 4, Mean, StdDev);
  PB('rcd_mean', Mean); PB('rcd_sd', StdDev);
  CMS(sP, sH, 4, Mean, StdDev);
  PB('cms_mean', Mean); PB('cms_sd', StdDev);
end.
