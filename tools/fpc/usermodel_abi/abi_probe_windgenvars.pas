program abi_probe_windgenvars;
// R4133_PROPS_PLAN RP1.3 (WindGen UserModel/UserData over the WASM host) —
// layout of the r4133 boundary record the WindGen user-model interface passes
// to a DLL: PCElements/WindGenVars.pas (TWindGenVars), the record
// TWindGenUserModel.FNew receives (WindGenUserModel.pas:34).
//
// TWindGenVars is NOT TGeneratorVars: it drops the NCIM `deltaQNom` slot and
// appends a turbine tail that starts with a MANAGED `PLoss: string` reference.
// Compiles the REAL vendored r4133 unit (via -Fu) and prints the size + every
// field offset, so the ABI doc's §2.6 table is measured, never read off the
// source (probe discipline, ABI doc §7).
// Output -> docs/wasm/probes/p10_offsets_windgenvars_r4133.txt
{$MODE DELPHI}

uses
    Ucomplex,     // r4133 Shared/Ucomplex.pas (Complex)
    WindGenVars;  // r4133 PCElements/WindGenVars.pas (TWindGenVars)

var
    wv: TWindGenVars;

procedure P(const RecName, FieldName: string; Off: PtrUInt);
begin
    WriteLn(RecName, '.', FieldName, ' offset = ', Off);
end;

begin
    WriteLn('abi_probe_windgenvars: OpenDSS r4133 Version8 TWindGenVars layout (x86_64-win64, FPC ', {$I %FPCVERSION%}, ')');
    WriteLn('SizeOf(Pointer) = ', SizeOf(Pointer));
    WriteLn('SizeOf(Complex) = ', SizeOf(Complex));
    WriteLn('SizeOf(string) = ', SizeOf(string));
    WriteLn;

    WriteLn('SizeOf(TWindGenVars) = ', SizeOf(TWindGenVars));
    P('TWindGenVars', 'Theta', PtrUInt(@wv.Theta) - PtrUInt(@wv));
    P('TWindGenVars', 'Pshaft', PtrUInt(@wv.Pshaft) - PtrUInt(@wv));
    P('TWindGenVars', 'Speed', PtrUInt(@wv.Speed) - PtrUInt(@wv));
    P('TWindGenVars', 'w0', PtrUInt(@wv.w0) - PtrUInt(@wv));
    P('TWindGenVars', 'Hmass', PtrUInt(@wv.Hmass) - PtrUInt(@wv));
    P('TWindGenVars', 'Mmass', PtrUInt(@wv.Mmass) - PtrUInt(@wv));
    P('TWindGenVars', 'D', PtrUInt(@wv.D) - PtrUInt(@wv));
    P('TWindGenVars', 'Dpu', PtrUInt(@wv.Dpu) - PtrUInt(@wv));
    P('TWindGenVars', 'kVArating', PtrUInt(@wv.kVArating) - PtrUInt(@wv));
    P('TWindGenVars', 'kVWindGenBase', PtrUInt(@wv.kVWindGenBase) - PtrUInt(@wv));
    P('TWindGenVars', 'Xd', PtrUInt(@wv.Xd) - PtrUInt(@wv));
    P('TWindGenVars', 'Xdp', PtrUInt(@wv.Xdp) - PtrUInt(@wv));
    P('TWindGenVars', 'Xdpp', PtrUInt(@wv.Xdpp) - PtrUInt(@wv));
    P('TWindGenVars', 'puXd', PtrUInt(@wv.puXd) - PtrUInt(@wv));
    P('TWindGenVars', 'puXdp', PtrUInt(@wv.puXdp) - PtrUInt(@wv));
    P('TWindGenVars', 'puXdpp', PtrUInt(@wv.puXdpp) - PtrUInt(@wv));
    P('TWindGenVars', 'dTheta', PtrUInt(@wv.dTheta) - PtrUInt(@wv));
    P('TWindGenVars', 'dSpeed', PtrUInt(@wv.dSpeed) - PtrUInt(@wv));
    P('TWindGenVars', 'ThetaHistory', PtrUInt(@wv.ThetaHistory) - PtrUInt(@wv));
    P('TWindGenVars', 'SpeedHistory', PtrUInt(@wv.SpeedHistory) - PtrUInt(@wv));
    P('TWindGenVars', 'Pnominalperphase', PtrUInt(@wv.Pnominalperphase) - PtrUInt(@wv));
    P('TWindGenVars', 'Qnominalperphase', PtrUInt(@wv.Qnominalperphase) - PtrUInt(@wv));
    // NOTE: no `deltaQNom` here — the NCIM slot is TGeneratorVars-only, so the
    // three integers keep the historical (Appendix A) offsets 176/180/184.
    P('TWindGenVars', 'NumPhases', PtrUInt(@wv.NumPhases) - PtrUInt(@wv));
    P('TWindGenVars', 'NumConductors', PtrUInt(@wv.NumConductors) - PtrUInt(@wv));
    P('TWindGenVars', 'Conn', PtrUInt(@wv.Conn) - PtrUInt(@wv));
    P('TWindGenVars', 'VthevMag', PtrUInt(@wv.VthevMag) - PtrUInt(@wv));
    P('TWindGenVars', 'VThevHarm', PtrUInt(@wv.VThevHarm) - PtrUInt(@wv));
    P('TWindGenVars', 'ThetaHarm', PtrUInt(@wv.ThetaHarm) - PtrUInt(@wv));
    P('TWindGenVars', 'VTarget', PtrUInt(@wv.VTarget) - PtrUInt(@wv));
    P('TWindGenVars', 'Zthev', PtrUInt(@wv.Zthev) - PtrUInt(@wv));
    P('TWindGenVars', 'Zthev.re', PtrUInt(@wv.Zthev.re) - PtrUInt(@wv));
    P('TWindGenVars', 'Zthev.im', PtrUInt(@wv.Zthev.im) - PtrUInt(@wv));
    P('TWindGenVars', 'XRdp', PtrUInt(@wv.XRdp) - PtrUInt(@wv));
    // The turbine tail. `PLoss` is a MANAGED AnsiString reference (pointer):
    // it cannot cross the wasm boundary as data, exactly like TGeneratorVars'
    // `deltaQNom` (ABI doc §2.2b).
    P('TWindGenVars', 'PLoss', PtrUInt(@wv.PLoss) - PtrUInt(@wv));
    P('TWindGenVars', 'ag', PtrUInt(@wv.ag) - PtrUInt(@wv));
    P('TWindGenVars', 'Cp', PtrUInt(@wv.Cp) - PtrUInt(@wv));
    P('TWindGenVars', 'Lamda', PtrUInt(@wv.Lamda) - PtrUInt(@wv));
    P('TWindGenVars', 'Poles', PtrUInt(@wv.Poles) - PtrUInt(@wv));
    P('TWindGenVars', 'pd', PtrUInt(@wv.pd) - PtrUInt(@wv));
    P('TWindGenVars', 'Rad', PtrUInt(@wv.Rad) - PtrUInt(@wv));
    P('TWindGenVars', 'VCutin', PtrUInt(@wv.VCutin) - PtrUInt(@wv));
    P('TWindGenVars', 'VCutout', PtrUInt(@wv.VCutout) - PtrUInt(@wv));
    P('TWindGenVars', 'Pm', PtrUInt(@wv.Pm) - PtrUInt(@wv));
    P('TWindGenVars', 'Ps', PtrUInt(@wv.Ps) - PtrUInt(@wv));
    P('TWindGenVars', 'Pr', PtrUInt(@wv.Pr) - PtrUInt(@wv));
    P('TWindGenVars', 'Pg', PtrUInt(@wv.Pg) - PtrUInt(@wv));
    P('TWindGenVars', 's', PtrUInt(@wv.s) - PtrUInt(@wv));
end.
