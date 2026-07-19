program abi_probe_capcontrolvars;
// WASM_USERMODELS WP-WM.5 — layout of the r4133 engine's `TCapControlVars`
// (the CapControl `PublicDataStruct := @ControlVars` record a user-written
// CapControl model reads/writes via `GetPublicDataPtr`,
// CapControl.pas:518-519). Compiles the REAL vendored r4133 unit
// `Version8/Source/Controls/CapControlVars.pas` in its `USER_DLL` variant
// (`{$IFDEF USER_DLL}` → `uses ucomplex` + `{$INCLUDE ControlActionDefs.txt}`,
// no ControlElem/engine closure) — the byte-identical record the engine's
// non-USER_DLL path also assembles (same ControlActionDefs.txt enum, same
// packing). This is the authoritative offset source for the WM.5 native twin
// (which reads r4133's @ControlVars) and the ABI-doc §2.4 image — never from
// reading the source (probe discipline). Output → docs/wasm/probes/
// p9_offsets_capcontrolvars_r4133.txt.
//
// NOTE the r4133-vs-0.14.5 record-LAYOUT divergences this probe pins:
//   * r4133 `Voverride: Boolean` (1 B) vs 0.14.5 `Voverride: LongBool` (4 B);
//   * r4133 `EControlAction` has NO `{$Z4}` (Delphi-mode default size) vs
//     0.14.5 `{$Z4}` (int32) — so `FPendingChange`/`PresentState`/
//     `InitialState` differ in width.
// BOTH engines set `PublicDataStruct := @ControlVars` ("So User-written models
// can access" — r4133 CapControl.pas:518, 0.14.5 CapControl.pas:535); only the
// record layout differs. r4133 is frozen because it is the WM.5 gate oracle.
{$MODE DELPHI}

uses
    ucomplex,        // r4133 Shared/ucomplex.pas (Complex)
    CapControlVars;  // r4133 Controls/CapControlVars.pas (-dUSER_DLL variant)

var
    cv: TCapControlVars;

procedure P(const FieldName: string; Off: PtrUInt);
begin
    WriteLn('TCapControlVars.', FieldName, ' offset = ', Off);
end;

begin
    WriteLn('abi_probe_capcontrolvars: OpenDSS r4133 Version8 TCapControlVars layout (x86_64-win64, USER_DLL variant, FPC ', {$I %FPCVERSION%}, ')');
    WriteLn('SizeOf(Complex) = ', SizeOf(Complex));
    WriteLn('SizeOf(Boolean) = ', SizeOf(Boolean));
    WriteLn('SizeOf(EControlAction) = ', SizeOf(EControlAction));
    WriteLn('SizeOf(TCapControlVars) = ', SizeOf(TCapControlVars));
    WriteLn;
    P('FCTPhase', PtrUInt(@cv.FCTPhase) - PtrUInt(@cv));
    P('FPTPhase', PtrUInt(@cv.FPTPhase) - PtrUInt(@cv));
    P('ON_Value', PtrUInt(@cv.ON_Value) - PtrUInt(@cv));
    P('OFF_Value', PtrUInt(@cv.OFF_Value) - PtrUInt(@cv));
    P('PFON_Value', PtrUInt(@cv.PFON_Value) - PtrUInt(@cv));
    P('PFOFF_Value', PtrUInt(@cv.PFOFF_Value) - PtrUInt(@cv));
    P('CTRatio', PtrUInt(@cv.CTRatio) - PtrUInt(@cv));
    P('PTRatio', PtrUInt(@cv.PTRatio) - PtrUInt(@cv));
    P('ONDelay', PtrUInt(@cv.ONDelay) - PtrUInt(@cv));
    P('OFFDelay', PtrUInt(@cv.OFFDelay) - PtrUInt(@cv));
    P('DeadTime', PtrUInt(@cv.DeadTime) - PtrUInt(@cv));
    P('LastOpenTime', PtrUInt(@cv.LastOpenTime) - PtrUInt(@cv));
    P('Voverride', PtrUInt(@cv.Voverride) - PtrUInt(@cv));
    P('VoverrideEvent', PtrUInt(@cv.VoverrideEvent) - PtrUInt(@cv));
    P('VoverrideBusSpecified', PtrUInt(@cv.VoverrideBusSpecified) - PtrUInt(@cv));
    P('VOverrideBusIndex', PtrUInt(@cv.VOverrideBusIndex) - PtrUInt(@cv));
    P('Vmax', PtrUInt(@cv.Vmax) - PtrUInt(@cv));
    P('Vmin', PtrUInt(@cv.Vmin) - PtrUInt(@cv));
    P('FPendingChange', PtrUInt(@cv.FPendingChange) - PtrUInt(@cv));
    P('ShouldSwitch', PtrUInt(@cv.ShouldSwitch) - PtrUInt(@cv));
    P('Armed', PtrUInt(@cv.Armed) - PtrUInt(@cv));
    P('PresentState', PtrUInt(@cv.PresentState) - PtrUInt(@cv));
    P('InitialState', PtrUInt(@cv.InitialState) - PtrUInt(@cv));
    P('SampleP', PtrUInt(@cv.SampleP) - PtrUInt(@cv));
    P('SampleP.re', PtrUInt(@cv.SampleP.re) - PtrUInt(@cv));
    P('SampleP.im', PtrUInt(@cv.SampleP.im) - PtrUInt(@cv));
    P('SampleV', PtrUInt(@cv.SampleV) - PtrUInt(@cv));
    P('SampleCurr', PtrUInt(@cv.SampleCurr) - PtrUInt(@cv));
    P('NumCapSteps', PtrUInt(@cv.NumCapSteps) - PtrUInt(@cv));
    P('AvailableSteps', PtrUInt(@cv.AvailableSteps) - PtrUInt(@cv));
    P('LastStepInService', PtrUInt(@cv.LastStepInService) - PtrUInt(@cv));
    P('VOverrideBusName', PtrUInt(@cv.VOverrideBusName) - PtrUInt(@cv));
    P('CapacitorName', PtrUInt(@cv.CapacitorName) - PtrUInt(@cv));
    P('ControlActionHandle', PtrUInt(@cv.ControlActionHandle) - PtrUInt(@cv));
    P('CondOffset', PtrUInt(@cv.CondOffset) - PtrUInt(@cv));
end.
