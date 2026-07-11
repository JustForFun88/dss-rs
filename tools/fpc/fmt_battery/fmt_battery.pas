program fmt_battery;
{ FPC 3.2.2 x86_64 (the pinned oracle backend's compiler/RTL): renders every
  f64 in values.txt (hex u64 bit patterns) through the float->ASCII entry
  points the Rust port mirrors. Output CSV (semicolon-separated, one row per
  value):
    hex;FloatToStr;g2;g5;g8;g15;s0;s14
  g<N>  = FloatToStrF(v, ffGeneral, N, 0)  -> util::fmt_g(v, N)
  s<W>  = Str(v:W)                          -> report::format::fpc_sci_w
  FloatToStr                                -> util::float_to_str }
{$mode objfpc}
uses SysUtils;

var
  fin, fout: TextFile;
  line, s, row: string;
  q: QWord;
  d: Double absolute q;
  code: Word;

function G(prec: Integer): string;
begin
  Result := FloatToStrF(d, ffGeneral, prec, 0);
end;

function SW(w: Integer): string;
var t: string;
begin
  Str(d: w, t);
  Result := t;
end;

begin
  FormatSettings.DecimalSeparator := '.';
  FormatSettings.ThousandSeparator := ',';
  AssignFile(fin, 'values.txt');
  Reset(fin);
  AssignFile(fout, 'fmt_battery.csv');
  Rewrite(fout);
  while not Eof(fin) do
  begin
    ReadLn(fin, line);
    if line = '' then continue;
    Val('$' + line, q, code);
    if code <> 0 then begin WriteLn('bad hex: ', line); Halt(1); end;
    s := FloatToStr(d);
    row := line + ';' + s + ';' + G(2) + ';' + G(5) + ';' + G(8) + ';' + G(15)
         + ';' + SW(0) + ';' + SW(14);
    WriteLn(fout, row);
  end;
  CloseFile(fin);
  CloseFile(fout);
end.
