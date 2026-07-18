program abi_probe;
// WASM_USERMODELS WP-WM.0 P2 — record-layout probe over the dss_capi 0.14.5
// user-model boundary records, compiled exactly like the release engine:
// FPC -Mdelphi, x86_64-win64, DSS_CAPI_NO_PACKED_RECORDS UNSET (=> packed).
//
// TDynamicsRec/TSolveMode come from the REAL vendored unit
// .inputs/dss_capi/src/Shared/Dynamics.pas (standalone, compiled via -Fu).
// TGeneratorVars/TDSSCallBacks live in units whose closure is the whole
// engine, so their declarations are extracted VERBATIM by extract_defs.py
// into dss_capi_defs.inc (provenance hashes inside).
//
// Output = the frozen offset tables in docs/wasm/USERMODEL_ABI.md; evidence
// copy under docs/wasm/probes/p2_offsets_dss_capi.txt.
{$MODE DELPHI}

uses
    ucomplex,  // FPC RTL: complex = record re, im: real(=Double) — the engine's UComplex
    Dynamics;  // the real vendored dss_capi unit (TDynamicsRec, TSolveMode)

type
{$I dss_capi_defs.inc}

var
    dyn: TDynamicsRec;
    gv: TGeneratorVars;
    cb: TDSSCallBacks;

procedure P(const RecName, FieldName: string; Off: PtrUInt);
begin
    WriteLn(RecName, '.', FieldName, ' offset = ', Off);
end;

begin
    WriteLn('abi_probe: dss_capi 0.14.5 user-model ABI record layouts (x86_64-win64, FPC ', {$I %FPCVERSION%}, ')');
    WriteLn('SizeOf(Pointer) = ', SizeOf(Pointer));
    WriteLn('SizeOf(Complex) = ', SizeOf(Complex));
    WriteLn('SizeOf(TSolveMode) = ', SizeOf(TSolveMode));
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
    // NB: in Delphi mode `@procvar` is the stored code pointer; `@@procvar`
    // is the address of the field itself — required for offsets here.
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
