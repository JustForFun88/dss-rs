library genstub;
// WASM_USERMODELS WP-WM.0 P1 — minimal 15-export Generator user-model stub
// DLL. Proves the pinned dss-python oracle (dss_capi 0.14.5 backend, win64)
// loads a native user-model DLL through `Generator.UserModel=` and drives the
// full 15-function contract (GenUserModel.pas `TGenUserModel`).
//
// Recognizable behavior, asserted by probe_oracle_load.py:
//   - New records the GenVars/DynaData/CallBacks pointers, returns ID 1;
//   - Calc stores V^[1] and writes I^[k] = (100*k, -10*k) A for
//     k = 1..GenVars.NumConductors (a converged Model=6 power-flow solution
//     must then report exactly those terminal currents);
//   - Edit records the received UserData string length;
//   - 4 state vars exposed: StubCalcCount, StubLastVre, StubLastVim,
//     StubEditLen (visible through the element's variable surface).
//
// Records come from dss_capi_defs.inc (verbatim dss_capi 0.14.5 extractions,
// see extract_defs.py) + the real vendored Dynamics.pas unit.
{$MODE DELPHI}

uses
    SysUtils,
    ucomplex,
    Dynamics; // vendored dss_capi src/Shared/Dynamics.pas (TDynamicsRec)

type
{$I dss_capi_defs.inc}

type
    PGenVars = ^TGeneratorVars;

var
    GGenVars: PGenVars = nil;
    GDyna: ^TDynamicsRec = nil;
    GCallBacks: pDSSCallBacks = nil;
    GCalcCount: Integer = 0;
    GEditLen: Integer = 0;
    GLastV: complex;

function XNew(GenVars: Pointer; var DynaData: TDynamicsRec; var CallBacks: TDSSCallBacks): Integer; stdcall;
begin
    GGenVars := GenVars;
    GDyna := @DynaData;
    GCallBacks := @CallBacks;
    Result := 1;
end;

procedure XDelete(var x: Integer); stdcall;
begin
end;

function XSelect(var x: Integer): Integer; stdcall;
begin
    Result := x;
end;

procedure XEdit(s: PAnsiChar; Maxlen: Cardinal); stdcall;
begin
    GEditLen := Maxlen;
end;

procedure XInit(V, I: pComplexArray); stdcall;
begin
end;

procedure XCalc(V, I: pComplexArray); stdcall;
var
    k, n: Integer;
begin
    Inc(GCalcCount);
    GLastV := V^[1];
    n := 2;
    if GGenVars <> nil then
        n := GGenVars^.NumConductors;
    for k := 1 to n do
    begin
        I^[k].re := 100.0 * k;
        I^[k].im := -10.0 * k;
    end;
end;

procedure XIntegrate; stdcall;
begin
end;

procedure XSave; stdcall;
begin
end;

procedure XRestore; stdcall;
begin
end;

procedure XUpdateModel; stdcall;
begin
end;

function XNumVars: Integer; stdcall;
begin
    Result := 4;
end;

function XGetVariable(var I: Integer): Double; stdcall;
begin
    case I of
        1: Result := GCalcCount;
        2: Result := GLastV.re;
        3: Result := GLastV.im;
        4: Result := GEditLen;
    else
        Result := -1.0;
    end;
end;

procedure XGetAllVars(Vars: pDoubleArray); stdcall;
var
    i, j: Integer;
begin
    if Vars = nil then
        Exit;
    for i := 1 to 4 do
    begin
        j := i;
        Vars^[i] := XGetVariable(j);
    end;
end;

procedure XSetVariable(var i: Integer; var value: Double); stdcall;
begin
end;

procedure XGetVarName(var VarNum: Integer; VarName: PAnsiChar; maxlen: Cardinal); stdcall;
begin
    case VarNum of
        1: StrLCopy(VarName, 'StubCalcCount', maxlen);
        2: StrLCopy(VarName, 'StubLastVre', maxlen);
        3: StrLCopy(VarName, 'StubLastVim', maxlen);
        4: StrLCopy(VarName, 'StubEditLen', maxlen);
    else
        StrLCopy(VarName, 'StubUnknown', maxlen);
    end;
end;

exports
    XNew name 'New',
    XDelete name 'Delete',
    XSelect name 'Select',
    XInit name 'Init',
    XCalc name 'Calc',
    XIntegrate name 'Integrate',
    XSave name 'Save',
    XRestore name 'Restore',
    XEdit name 'Edit',
    XUpdateModel name 'UpdateModel',
    XNumVars name 'NumVars',
    XGetAllVars name 'GetAllVars',
    XGetVariable name 'GetVariable',
    XSetVariable name 'SetVariable',
    XGetVarName name 'GetVarName';

begin
end.
