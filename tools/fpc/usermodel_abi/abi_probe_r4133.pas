program abi_probe_r4133;
// WASM_USERMODELS WP-WM.3 pre-round (ABI re-freeze to r4133, user decision
// 2026-07-19) — layouts of the SAME boundary records as the r4133 example DLL
// compiles against
// (.inputs/electricdss-code-r4133-trunk/Version8/Source/IndMach012a/), i.e. the
// r4133 headers:
//   Shared/Dynamics.pas (TDynamicsRec), PCElements/GeneratorVars.pas
//   (TGeneratorVars, now with deltaQNom), Common/DSSCallBackStructDef.pas
//   (TDSSCallBacks incl.).
// Compiles those REAL vendored r4133 units/includes (via -Fu/-Fi) and prints
// the same size/offset tables as abi_probe_r3723.pas — proving EMPIRICALLY the
// r4088+/NCIM `deltaQNom: array of Double` slot and the +8 tail shift the ABI
// re-freeze pins (never from reading the source — probe discipline). Output →
// docs/wasm/probes/p8_offsets_r4133.txt.
{$MODE DELPHI}

uses
    Ucomplex,      // r4133 Shared/Ucomplex.pas (complex, pComplexArray)
    Arraydef,      // r4133 Shared/Arraydef.pas (pIntegerArray, pDoubleArray)
    Dynamics,      // r4133 Shared/Dynamics.pas (TDynamicsRec) — identical to r3723
    GeneratorVars; // r4133 PCElements/GeneratorVars.pas (TGeneratorVars + deltaQNom)

type
    // r4133's TDSSCallBacks include uses pUTF8Char, declared for the engine in
    // Common/DSSCallBackRoutines.pas as exactly:
    pUTF8Char = ^AnsiChar;
    // (repeated here because that unit's closure is the whole engine; the
    // include below is the verbatim vendored file — identical to r3723)
{$I DSSCallBackStructDef.pas}

var
    dyn: TDynamicsRec;
    gv: TGeneratorVars;
    cb: TDSSCallBacks;

procedure P(const RecName, FieldName: string; Off: PtrUInt);
begin
    WriteLn(RecName, '.', FieldName, ' offset = ', Off);
end;

begin
    WriteLn('abi_probe_r4133: OpenDSS r4133 Version8 user-model ABI record layouts (x86_64-win64, FPC ', {$I %FPCVERSION%}, ')');
    WriteLn('SizeOf(Pointer) = ', SizeOf(Pointer));
    WriteLn('SizeOf(Complex) = ', SizeOf(Complex));
    WriteLn('SizeOf(Boolean) = ', SizeOf(Boolean));
    WriteLn;

    WriteLn('SizeOf(TDynamicsRec) = ', SizeOf(TDynamicsRec));
    P('TDynamicsRec', 'h', PtrUInt(@dyn.h) - PtrUInt(@dyn));
    P('TDynamicsRec', 't', PtrUInt(@dyn.t) - PtrUInt(@dyn));
    P('TDynamicsRec', 'tstart', PtrUInt(@dyn.tstart) - PtrUInt(@dyn));
    P('TDynamicsRec', 'tstop', PtrUInt(@dyn.tstop) - PtrUInt(@dyn));
    P('TDynamicsRec', 'IterationFlag', PtrUInt(@dyn.IterationFlag) - PtrUInt(@dyn));
    P('TDynamicsRec', 'SolutionMode', PtrUInt(@dyn.SolutionMode) - PtrUInt(@dyn));
    P('TDynamicsRec', 'intHour', PtrUInt(@dyn.intHour) - PtrUInt(@dyn));
    P('TDynamicsRec', 'dblHour', PtrUInt(@dyn.dblHour) - PtrUInt(@dyn));
    WriteLn;

    WriteLn('SizeOf(TGeneratorVars) = ', SizeOf(TGeneratorVars));
    P('TGeneratorVars', 'Theta', PtrUInt(@gv.Theta) - PtrUInt(@gv));
    P('TGeneratorVars', 'Pshaft', PtrUInt(@gv.Pshaft) - PtrUInt(@gv));
    P('TGeneratorVars', 'Speed', PtrUInt(@gv.Speed) - PtrUInt(@gv));
    P('TGeneratorVars', 'w0', PtrUInt(@gv.w0) - PtrUInt(@gv));
    P('TGeneratorVars', 'Hmass', PtrUInt(@gv.Hmass) - PtrUInt(@gv));
    P('TGeneratorVars', 'Mmass', PtrUInt(@gv.Mmass) - PtrUInt(@gv));
    P('TGeneratorVars', 'D', PtrUInt(@gv.D) - PtrUInt(@gv));
    P('TGeneratorVars', 'Dpu', PtrUInt(@gv.Dpu) - PtrUInt(@gv));
    P('TGeneratorVars', 'kVArating', PtrUInt(@gv.kVArating) - PtrUInt(@gv));
    P('TGeneratorVars', 'kVGeneratorBase', PtrUInt(@gv.kVGeneratorBase) - PtrUInt(@gv));
    P('TGeneratorVars', 'Xd', PtrUInt(@gv.Xd) - PtrUInt(@gv));
    P('TGeneratorVars', 'Xdp', PtrUInt(@gv.Xdp) - PtrUInt(@gv));
    P('TGeneratorVars', 'Xdpp', PtrUInt(@gv.Xdpp) - PtrUInt(@gv));
    P('TGeneratorVars', 'puXd', PtrUInt(@gv.puXd) - PtrUInt(@gv));
    P('TGeneratorVars', 'puXdp', PtrUInt(@gv.puXdp) - PtrUInt(@gv));
    P('TGeneratorVars', 'puXdpp', PtrUInt(@gv.puXdpp) - PtrUInt(@gv));
    P('TGeneratorVars', 'dTheta', PtrUInt(@gv.dTheta) - PtrUInt(@gv));
    P('TGeneratorVars', 'dSpeed', PtrUInt(@gv.dSpeed) - PtrUInt(@gv));
    P('TGeneratorVars', 'ThetaHistory', PtrUInt(@gv.ThetaHistory) - PtrUInt(@gv));
    P('TGeneratorVars', 'SpeedHistory', PtrUInt(@gv.SpeedHistory) - PtrUInt(@gv));
    P('TGeneratorVars', 'Pnominalperphase', PtrUInt(@gv.Pnominalperphase) - PtrUInt(@gv));
    P('TGeneratorVars', 'Qnominalperphase', PtrUInt(@gv.Qnominalperphase) - PtrUInt(@gv));
    // The r4088+/NCIM insertion: a managed (8-byte reference) dynamic array
    // between Qnominalperphase and NumPhases — shifts the whole tail by +8.
    P('TGeneratorVars', 'deltaQNom', PtrUInt(@gv.deltaQNom) - PtrUInt(@gv));
    P('TGeneratorVars', 'NumPhases', PtrUInt(@gv.NumPhases) - PtrUInt(@gv));
    P('TGeneratorVars', 'NumConductors', PtrUInt(@gv.NumConductors) - PtrUInt(@gv));
    P('TGeneratorVars', 'Conn', PtrUInt(@gv.Conn) - PtrUInt(@gv));
    P('TGeneratorVars', 'VthevMag', PtrUInt(@gv.VthevMag) - PtrUInt(@gv));
    P('TGeneratorVars', 'VThevHarm', PtrUInt(@gv.VThevHarm) - PtrUInt(@gv));
    P('TGeneratorVars', 'ThetaHarm', PtrUInt(@gv.ThetaHarm) - PtrUInt(@gv));
    P('TGeneratorVars', 'VTarget', PtrUInt(@gv.VTarget) - PtrUInt(@gv));
    P('TGeneratorVars', 'Zthev', PtrUInt(@gv.Zthev) - PtrUInt(@gv));
    P('TGeneratorVars', 'Zthev.re', PtrUInt(@gv.Zthev.re) - PtrUInt(@gv));
    P('TGeneratorVars', 'Zthev.im', PtrUInt(@gv.Zthev.im) - PtrUInt(@gv));
    P('TGeneratorVars', 'XRdp', PtrUInt(@gv.XRdp) - PtrUInt(@gv));
    WriteLn;

    WriteLn('SizeOf(TDSSCallBacks) = ', SizeOf(TDSSCallBacks));
    P('TDSSCallBacks', 'MsgCallBack', PtrUInt(@@cb.MsgCallBack) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetIntValue', PtrUInt(@@cb.GetIntValue) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetDblValue', PtrUInt(@@cb.GetDblValue) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetStrValue', PtrUInt(@@cb.GetStrValue) - PtrUInt(@cb));
    P('TDSSCallBacks', 'LoadParser', PtrUInt(@@cb.LoadParser) - PtrUInt(@cb));
    P('TDSSCallBacks', 'NextParam', PtrUInt(@@cb.NextParam) - PtrUInt(@cb));
    P('TDSSCallBacks', 'DoDSSCommand', PtrUInt(@@cb.DoDSSCommand) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetActiveElementBusNames', PtrUInt(@@cb.GetActiveElementBusNames) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetActiveElementVoltages', PtrUInt(@@cb.GetActiveElementVoltages) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetActiveElementCurrents', PtrUInt(@@cb.GetActiveElementCurrents) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetActiveElementLosses', PtrUInt(@@cb.GetActiveElementLosses) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetActiveElementPower', PtrUInt(@@cb.GetActiveElementPower) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetActiveElementNumCust', PtrUInt(@@cb.GetActiveElementNumCust) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetActiveElementNodeRef', PtrUInt(@@cb.GetActiveElementNodeRef) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetActiveElementBusRef', PtrUInt(@@cb.GetActiveElementBusRef) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetActiveElementTerminalInfo', PtrUInt(@@cb.GetActiveElementTerminalInfo) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetPtrToSystemVarray', PtrUInt(@@cb.GetPtrToSystemVarray) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetActiveElementIndex', PtrUInt(@@cb.GetActiveElementIndex) - PtrUInt(@cb));
    P('TDSSCallBacks', 'IsActiveElementEnabled', PtrUInt(@@cb.IsActiveElementEnabled) - PtrUInt(@cb));
    P('TDSSCallBacks', 'IsBusCoordinateDefined', PtrUInt(@@cb.IsBusCoordinateDefined) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetBusCoordinate', PtrUInt(@@cb.GetBusCoordinate) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetBuskVBase', PtrUInt(@@cb.GetBuskVBase) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetBusDistFromMeter', PtrUInt(@@cb.GetBusDistFromMeter) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetDynamicsStruct', PtrUInt(@@cb.GetDynamicsStruct) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetStepSize', PtrUInt(@@cb.GetStepSize) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetTimeSec', PtrUInt(@@cb.GetTimeSec) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetTimeHr', PtrUInt(@@cb.GetTimeHr) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetPublicDataPtr', PtrUInt(@@cb.GetPublicDataPtr) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetActiveElementName', PtrUInt(@@cb.GetActiveElementName) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetActiveElementPtr', PtrUInt(@@cb.GetActiveElementPtr) - PtrUInt(@cb));
    P('TDSSCallBacks', 'ControlQueuePush', PtrUInt(@@cb.ControlQueuePush) - PtrUInt(@cb));
    P('TDSSCallBacks', 'GetResultStr', PtrUInt(@@cb.GetResultStr) - PtrUInt(@cb));
end.
