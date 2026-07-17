"""Non-0.14.5-wheel help-string supplement for `gen_help_catalog.py`.

`gen_help_catalog.py` parses the pinned 0.14.5 wheel's gettext catalog
(`properties-en-US.mo`), but several ported surfaces carry help the pinned wheel
predates. This dict overrides/adds those, keyed by `<Class>.<prop-lowercase>` (or
`Command.<name>`), and is applied on top of the wheel parse. Two groups:

1. **Carried-forward pre-r4133 non-wheel help.** Earlier WPs hand-edited these
   into the previously-"GENERATED" `help_catalog.rs` (LineSpacing 0.15.x
   equivalent-spacing fields, WP-U1.4; `CNData.SemiconLayer`, the dump3 fix; the
   A-Diakoptics `tear_circuit` command note). WP-U2.5 folds them into the
   generator so `python gen_help_catalog.py` reproduces the committed file
   exactly instead of silently reverting them.

2. **r4133 protection overhaul (WP-U2.1..U2.5).** The 0.14.5 wheel carries the
   *old* Relay (50-prop), Recloser (24-prop), Fuse (10-prop) and SwtControl
   (8-prop) help; the port moved those classes to their r4133 surfaces (Relay 71,
   Recloser 46, Fuse 12, SwtControl 9). Source of truth = the official EPRI r4133
   engine's own `PropertyHelp` arrays (`.inputs/electricdss-code-r4133-trunk/
   Version8/Source/Controls/{Relay,Recloser,fuse,SwtControl}.pas`), captured
   verbatim from the r4133 binary's `Dump commands` output (oddie:r4133,
   `tools/opendss/bin/r4133`). Only the class-specific props are listed; the
   inherited `basefreq`/`enabled`/`like` tail is rev-identical and stays sourced
   from the wheel.

The `Dump commands` golden's `[Relay]`/`[Recloser]`/`[Fuse]`/`[SwtControl]` blocks
are pinned self-referentially against this text (the WP8.5 self-referential
regression-pin convention, as `tests/golden/props/relay.json`): the numeric-token
/ round-trip gates cover the property NAMES and VALUES against oddie:r4133, and no
byte-exact-vs-Delphi help gate exists (UPGRADE_PLAN.md §1.3-2).
"""

from __future__ import annotations

R4133_HELP: dict[str, str] = {
    # --- Carried-forward pre-r4133 non-wheel help (earlier WPs; see module docstring) ---
    "CNData.semiconlayer": "{Yes/True | No/False}  Default is Yes. Existence of a semicon layer between the insulation layer and the concentric neutral strands. Affects calculation of shunt self admittances.",
    "Command.tear_circuit": "Estimates the buses for tearing the system in many parts as CPUs - 1 are in the local computer, is used for tearing the interconnected circuit into a balanced (same number of nodes) collection of subsystems for the A-Diakoptics algorithm. **Supported in dss-rs (A-Diakoptics ported per the official r3723 Delphi spec; the pinned dss_capi/dss-python oracle compiles this out).**",
    "LineSpacing.avgneutralheight": "Average height of neutral conductors. Used for equivalent distance modeling (detailed=no) as opposed to detailed cross-section coordinates.",
    "LineSpacing.avgphaseheight": "Average height of phase conductors. Used for equivalent distance modeling (detailed=no) as opposed to detailed cross-section coordinates.",
    "LineSpacing.detailed": "{Yes/True | No/False} Default = Yes. Determines whether the spacing uses a detailed cross-section coordinates with x and h arrays (Yes/True), or uses equivalent spacing fields (No/False). The equivalent spacing fields are EqDistPhPh, EqDistPhN, AvgPhaseHeight and AvgNeutralHeight.",
    "LineSpacing.eqdistphn": "Equivalent distance between phase and neutral conductors (geometric mean distance). Used for equivalent distance modeling (detailed=no) as opposed to detailed cross-section coordinates.",
    "LineSpacing.eqdistphph": "Equivalent distance between phase conductors (geometric mean distance). Used for equivalent distance modeling (detailed=no) as opposed to detailed cross-section coordinates.",
    # Relay r4133 props 1-71 (Version8/Source/Controls/Relay.pas)
    "Relay.monitoredobj": "Full object name of the circuit element, typically a line, transformer, load, or generator, to which the relay's PT and/or CT are connected. This is the \"monitored\" element. There is no default; must be specified.",
    "Relay.monitoredterm": "Number of the terminal of the circuit element to which the Relay is connected. 1 or 2, typically.  Default is 1.",
    "Relay.switchedobj": "Name of circuit element switch that the Relay controls. Specify the full object name.Defaults to the same as the Monitored element. This is the \"controlled\" element.",
    "Relay.switchedterm": "Number of the terminal of the controlled element in which the switch is controlled by the Relay. 1 or 2, typically.  Default is 1.",
    "Relay.type": "One of a legal relay type:\n  Current\n  Voltage\n  Reversepower\n  46 (neg seq current)\n  47 (neg seq voltage)\n  Generic (generic over/under relay)\n  Distance\n  TD21\n  DOC (directional overcurrent)\n\nDefault is overcurrent relay (Current). Specify the curve and pickup settings appropriate for each type. Generic relays monitor PC Element Control variables and trip on out of over/under range in definite time.",
    "Relay.phcurve": "Name of the TCC Curve object that determines the phase trip.  Must have been previously defined as a TCC_Curve object or specified as \"none\" (ignored). Default is \"none\". For overcurrent relay, multiplying the current values in the curve by the \"PhPickup\" value gives the actual current.",
    "Relay.oc_gndcurve": "Name of the TCC Curve object that determines the ground trip for overcurrent relay.  Must have been previously defined as a TCC_Curve object or specified as \"none\" (ignored). Default is \"none\". For overcurrent relay, multiplying the current values in the curve by the \"GndPickup\" value gives the actual current.",
    "Relay.phpickup": "Multiplier for the phase TCC curve for overcurrent relay OR actual phase amps when operating with definite time (see \"DefiniteTimeDelay\" property). Defaults to 1.0.",
    "Relay.oc_gndpickup": "Multiplier for the ground TCC curve for overcurrent relay OR actual ground amps (3I0) when operating with definite time (see \"DefiniteTimeDelay\" property). Defaults to 1.0.",
    "Relay.tdph": "Time dial for Phase trip curve. Multiplier on time axis of specified curve. Default=1.0.",
    "Relay.oc_tdgnd": "Time dial for Ground trip curve for overcurrent relay. Multiplier on time axis of specified curve. Default=1.0.",
    "Relay.phinst": "Actual  amps (Current relay) or kW (reverse power relay) for instantaneous phase trip which is assumed to happen in 0.01 sec + Mechanical Delay Time. Default is 0.0, which signifies no inst trip. Use this value for specifying the Reverse Power threshold (kW) for reverse power relays.",
    "Relay.oc_gndinst": "Actual amps for instantaneous ground trip for overcurrent relay which is assumed to happen in 0.01 sec + Mechanical Delay Time. Default is 0.0, which signifies no inst trip.",
    "Relay.resettime": "Reset time in sec for relay.  Default is 15. If this much time passes between the last pickup event, and the relay has not locked out, the operation counter resets.",
    "Relay.shots": "Number of shots to lockout. Default is 4. This is one more than the number of reclose intervals.",
    "Relay.recloseintervals": "Array of reclose intervals. If none, specify \"NONE\". Default for overcurrent relay is (0.5, 2.0, 2.0) seconds. Default for a voltage relay is (5.0). In a voltage relay, this is seconds after restoration of voltage that the reclose occurs. Reverse power relay is one shot to lockout, so this is ignored.  A locked out relay must be closed manually (set action=close).",
    "Relay.definitetimedelay": "Trip time delay (sec) for DEFINITE TIME relays. Default is 0.0 for current and DOC relays. For overcurrent relays, if>0 and specified pickups (ground and/or phase) are excedeed, definite time operation is used instead of curves. For DOC relay, if>0 definite time operation is used instead of curves. Used by Generic, RevPower, 46 and 47 relays. Defaults to 0.1 s for these relays.",
    "Relay.voltage_ovcurve": "TCC Curve object to use for overvoltage relay. Must have been previously defined as a TCC_Curve object or specified as \"none\" (ignored). Default is \"none\". Curve is assumed to be defined with per unit voltage values. Voltage base should be defined for the relay. Default is none (ignored).",
    "Relay.voltage_uvcurve": "TCC Curve object to use for undervoltage relay. Must have been previously defined as a TCC_Curve object or specified as \"none\" (ignored). Default is \"none\". Curve is assumed to be defined with per unit voltage values. Voltage base should be defined for the relay. Default is none (ignored).",
    "Relay.kvbase": "Voltage base (kV) for the relay. Specify line-line for 3 phase devices); line-neutral for 1-phase devices.  Relay assumes the number of phases of the monitored element.  Default is 0.0, which results in assuming the voltage values in the \"TCC\" curve are specified in actual line-to-neutral volts.",
    "Relay.47%pickup": "Percent voltage pickup for 47 relay (Neg seq voltage). Default is 2. Specify also base voltage (kvbase) and delay time value.   ",
    "Relay.46baseamps": "Base current, Amps, for 46 relay (neg seq current).  Used for establishing pickup and per unit I-squared-t.",
    "Relay.46%pickup": "Percent pickup current for 46 relay (neg seq current).  Default is 20.0.   When current exceeds this value * BaseAmps, I-squared-t calc starts.",
    "Relay.46isqt": "Negative Sequence I-squared-t trip value for 46 relay (neg seq current).  Default is 1 (trips in 1 sec for 1 per unit neg seq current).  Should be 1 to 99.",
    "Relay.generic_variable": "Name of variable in PC Elements being monitored. Only applies to Generic relay.",
    "Relay.generic_overtrip": "Trip setting (high value) for Generic relay variable. Relay trips in definite time if value of variable exceeds this value.",
    "Relay.generic_undertrip": "Trip setting (low value) for Generic relay variable. Relay trips in definite time if value of variable is less than this value.",
    "Relay.mechanicaldelay": "Fixed delay time (sec) added to relay time. Default is 0.0. Designed to represent breaker time or some other delay after a trip decision is made.Use Delay property for setting a fixed trip time delay.Added to trip time of current and voltage relays. Could use in combination with inst trip value to obtain a definite time overcurrent relay.",
    "Relay.action": "DEPRECATED. See \"State\" property",
    "Relay.z1mag": "Positive sequence reach impedance in primary ohms for Distance and TD21 functions. Default=0.7",
    "Relay.z1ang": "Positive sequence reach impedance angle in degrees for Distance and TD21 functions. Default=64.0",
    "Relay.z0mag": "Zero sequence reach impedance in primary ohms for Distance and TD21 functions. Default=2.1",
    "Relay.z0ang": "Zero sequence reach impedance angle in degrees for Distance and TD21 functions. Default=68.0",
    "Relay.mphase": "Phase reach multiplier in per-unit for Distance and TD21 functions. Default=0.7",
    "Relay.mground": "Ground reach multiplier in per-unit for Distance and TD21 functions. Default=0.7",
    "Relay.eventlog": "{Yes/True* | No/False} Default is Yes for Relay. Write trips, reclose and reset events to EventLog.",
    "Relay.debugtrace": "{Yes/True* | No/False} Default is No for Relay. Write extra details to Eventlog.",
    "Relay.distreverse": "{Yes/True* | No/False} Default is No; reverse direction for distance and td21 types.",
    "Relay.normal": "ARRAY of strings {Open | Closed} representing the Normal state of the relay in each phase of the controlled element. The relay reverts to this state for reset, change of mode, etc. Defaults to \"State\" if not specifically declared.  Setting this property to {Open | Closed} sets the normal state to the specified value for all phases (ganged operation).",
    "Relay.state": "ARRAY of strings {Open | Closed} representing the Actual state of the relay in each phase of the controlled element. Upon setting, immediately forces the state of the relay. Simulates manual control on the controlled relay. Defaults to Closed for all phases. Setting this property to {Open | Closed} sets the actual state to the specified value for all phases (ganged operation). \"Open\" causes the controlled element or respective phase to open and lock out. \"Closed\" causes the controlled element or respective phase to close and the relay to reset to its first operation.",
    "Relay.doc_tiltanglelow": "Tilt angle for low-current trip line. Default is 90.",
    "Relay.doc_tiltanglehigh": "Tilt angle for high-current trip line. Default is 90.",
    "Relay.doc_tripsettinglow": "Resistive trip setting for low-current line.  Default is 0.",
    "Relay.doc_tripsettinghigh": "Resistive trip setting for high-current line.  Default is -1 (deactivated). To activate, set a positive value. Must be greater than \"DOC_TripSettingLow\".",
    "Relay.doc_tripsettingmag": "Trip setting for current magnitude (defines a circle in the relay characteristics). Default is -1 (deactivated). To activate, set a positive value.",
    "Relay.doc_delayinner": "Trip time delay (sec) for operation in inner region for DOC relay, defined when \"DOC_TripSettingMag\" or \"DOC_TripSettingHigh\" are activate. Default is -1.0 (deactivated), meaning that the relay characteristic is insensitive in the inner region (no trip). Set to 0 for instantaneous trip and >0 for a definite time delay. If \"DOC_PhaseCurveInner\" is specified, time delay from curve is utilized instead.",
    "Relay.doc_phasecurveinner": "Name of the TCC Curve object that determines the phase trip for operation in inner region for DOC relay. Must have been previously defined as a TCC_Curve object or specified as \"none\" (ignored). Default is \"none\". Multiplying the current values in the curve by the \"DOC_PhaseTripInner\" value gives the actual current.",
    "Relay.doc_phasetripinner": "Multiplier for the \"DOC_PhaseCurveInner\" TCC curve.  Defaults to 1.0.",
    "Relay.doc_tdphaseinner": "Time dial for \"DOC_PhaseCurveInner\" TCC curve. Multiplier on time axis of specified curve. Default=1.0.",
    "Relay.doc_p1blocking": "{Yes/True* | No/False} Blocking element that impedes relay from tripping if balanced net three-phase active power is in the forward direction (i.e., flowing into the monitored terminal). For a delayed trip, if at any given time the reverse power flow condition stops, the tripping is reset. Default=True.",
    "Relay.singlephtrip": "{Yes/True | No*/False} Enables single-phase tripping and reclosing for multi-phase controlled elements. Previously locked out phases do not operate/reclose even considering multi-phase tripping. Applies to overcurrent relays only (type=current). Ignored for other types.",
    "Relay.singlephlockout": "{Yes/True | No*/False} Enables single-phase lockout for multi-phase controlled elements with single-phase tripping. Does not have impact if single-phase trip is not enabled.",
    "Relay.lock": "{Yes/True | No*/False} Controlled switch is locked in its present open / closed state or unlocked. When locked, the relay will not respond to either a manual state change issued by the user or a state change issued internally by OpenDSS when Reseting the control. Note this locking mechanism is different from the relay automatic lockout after specifed Shots.",
    "Relay.reset": "{Yes/True | No*/False} If Yes, forces Reset of relay to Normal state and removes Lock independently of any internal reset command for mode change, etc.",
    "Relay.ratedcurrent": "Controlled conducting element's continuous rated current in Amps. Defaults to 0. Not used internally for either power flow or reporting.",
    "Relay.interruptingrating": "Controlled conducting element's rated interrupting current in Amps. Defaults to 0. Not used internally for either power flow or reporting.",
    "Relay.breakertime": "DEPRECATED. See \"MechanicalDelay\" property.",
    "Relay.delay": "DEPRECATED. See \"DefiniteTimeDelay\" property.",
    "Relay.groundcurve": "DEPRECATED. See \"OC_GndCurve\" property.",
    "Relay.groundtrip": "DEPRECATED. See \"OC_GndPickup\" property.",
    "Relay.groundinst": "DEPRECATED. See \"OC_GndInst\" property.",
    "Relay.tdground": "DEPRECATED. See \"OC_TDGnd\" property.",
    "Relay.phasecurve": "DEPRECATED. See \"PhCurve\" property.",
    "Relay.phasetrip": "DEPRECATED. See \"PhPickup\" property.",
    "Relay.phaseinst": "DEPRECATED. See \"PhInst\" property.",
    "Relay.tdphase": "DEPRECATED. See \"TDPh\" property.",
    "Relay.overtrip": "DEPRECATED. See \"Generic_OverTrip\" property.",
    "Relay.undertrip": "DEPRECATED. See \"Generic_UnderTrip\" property.",
    "Relay.variable": "DEPRECATED. See \"Generic_Variable\" property.",
    "Relay.overvoltcurve": "DEPRECATED. See \"Voltage_OVCurve\" property.",
    "Relay.undervoltcurve": "DEPRECATED. See \"Voltage_UVCurve\" property.",
    # Recloser r4133 props 1-46 (Version8/Source/Controls/Recloser.pas)
    "Recloser.monitoredobj": "Full object name of the circuit element, typically a line, transformer, load, or generator, to which the Recloser's PT and/or CT are connected. This is the \"monitored\" element. There is no default; must be specified.",
    "Recloser.monitoredterm": "Number of the terminal of the circuit element to which the Recloser is connected. 1 or 2, typically.  Default is 1.",
    "Recloser.switchedobj": "Name of circuit element switch that the Recloser controls. Specify the full object name.Defaults to the same as the Monitored element. This is the \"controlled\" element.",
    "Recloser.switchedterm": "Number of the terminal of the controlled element in which the switch is controlled by the Recloser. 1 or 2, typically.  Default is 1.",
    "Recloser.numfast": "Number of Fast (fuse saving) operations.  Default is 1. (See \"Shots\")",
    "Recloser.phfastcurve": "Name of the TCC Curve object that determines the Phase Fast trip. Must have been previously defined as a TCC_Curve object or specified as \"none\" (ignored). Default is \"none\". Multiplying the current values in the curve by the \"PhFastPickup\" value gives the actual current.",
    "Recloser.phslowcurve": "Name of the TCC Curve object that determines the Phase Slow trip. Must have been previously defined as a TCC_Curve object or specified as \"none\" (ignored). Default is \"none\". Multiplying the current values in the curve by the \"PhSlowPickup\" value gives the actual current.",
    "Recloser.gndfastcurve": "Name of the TCC Curve object that determines the Ground Fast trip.  Must have been previously defined as a TCC_Curve object or specified as \"none\" (ignored). Default is \"none\". Multiplying the current values in the curve by the \"GndFastPickup\" value gives the actual current.",
    "Recloser.gndslowcurve": "Name of the TCC Curve object that determines the Ground Slow trip.  Must have been previously defined as a TCC_Curve object or specified as \"none\" (ignored). Default is \"none\". Multiplying the current values in the curve by the \"GndSlowPickup\" value gives the actual current.",
    "Recloser.phfastpickup": "Multiplier for the phase fast TCC curve. Defaults to 1.0.",
    "Recloser.gndfastpickup": "Multiplier for the ground fast TCC curve. Defaults to 1.0.",
    "Recloser.phinst": "Actual amps for instantaneous phase trip which is assumed to happen in 0.01 sec + Mechanical Delay Time. Default is 0.0, which signifies no inst trip.",
    "Recloser.gndinst": "Actual amps for instantaneous ground trip which is assumed to happen in 0.01 sec + Mechanical Delay Time. Default is 0.0, which signifies no inst trip.",
    "Recloser.resettime": "Reset time in sec for Recloser. Default is 15.",
    "Recloser.shots": "Total Number of fast and delayed shots to lockout.  Default is 4. This is one more than the number of reclose intervals.",
    "Recloser.recloseintervals": "Array of reclose intervals.  Default for Recloser is (0.5, 2.0, 2.0) seconds. A locked out Recloser must be closed manually (action=close).",
    "Recloser.mechanicaldelay": "Fixed delay time (sec) added to Recloser trip time. Default is 0.0. Used to represent breaker time or any other delay.",
    "Recloser.action": "DEPRECATED. See \"State\" property",
    "Recloser.tdphfast": "Time dial for Phase Fast trip curve. Multiplier on time axis of specified curve. Default=1.0.",
    "Recloser.tdgndfast": "Time dial for Ground Fast trip curve. Multiplier on time axis of specified curve. Default=1.0.",
    "Recloser.tdphslow": "Time dial for Phase Slow trip curve. Multiplier on time axis of specified curve. Default=1.0.",
    "Recloser.tdgndslow": "Time dial for Ground Slow trip curve. Multiplier on time axis of specified curve. Default=1.0.",
    "Recloser.normal": "ARRAY of strings {Open | Closed} representing the Normal state of the recloser in each phase of the controlled element. The recloser reverts to this state for reset, change of mode, etc. Defaults to \"State\" if not specifically declared.  Setting this property to {Open | Closed} sets the normal state to the specified value for all phases (ganged operation).",
    "Recloser.state": "ARRAY of strings {Open | Closed} representing the Actual state of the recloser in each phase of the controlled element. Upon setting, immediately forces the state of the recloser. Simulates manual control on Recloser. Defaults to Closed for all phases. Setting this property to {Open | Closed} sets the actual state to the specified value for all phases (ganged operation). \"Open\" causes the controlled element or respective phase to open and lock out. \"Closed\" causes the controlled element or respective phase to close and the recloser to reset to its first operation.",
    "Recloser.singlephtrip": "{Yes | No*} Enables single-phase tripping and reclosing for multi-phase controlled elements. Previously locked out phases do not operate/reclose even considering multi-phase tripping.",
    "Recloser.singlephlockout": "{Yes | No*} Enables single-phase lockout for multi-phase controlled elements with single-phase tripping. Does not have impact if single-phase trip is not enabled.",
    "Recloser.lock": "{Yes | No*} Controlled switch is locked in its present open / closed state or unlocked. When locked, the recloser will not respond to either a manual state change issued by the user or a state change issued internally by OpenDSS when reseting the control. Note this locking mechanism is different from the recloser automatic lockout after specifed number of shots.",
    "Recloser.reset": "{Yes | No} If Yes, forces Reset of recloser to Normal state and removes Lock independently of any internal reset command for mode change, etc.",
    "Recloser.eventlog": "{Yes/True* | No/False} Default is Yes for Recloser. Write trips, reclose and reset events to EventLog.",
    "Recloser.debugtrace": "{Yes/True* | No/False} Default is No for Recloser. Write extra details to Eventlog.",
    "Recloser.ratedcurrent": "Recloser continuous rated current in Amps. Defaults to 0. Not used internally for either power flow or reporting.",
    "Recloser.interruptingrating": "Recloser rated interrupting current in Amps. Defaults to 0. Not used internally for either power flow or reporting.",
    "Recloser.phasefast": "DEPRECATED. See \"PhFastCurve\" property.",
    "Recloser.phasedelayed": "DEPRECATED. See \"PhSlowCurve\" property.",
    "Recloser.groundfast": "DEPRECATED. See \"GndFastCurve\" property.",
    "Recloser.grounddelayed": "DEPRECATED. See \"GndSlowCurve\" property.",
    "Recloser.phasetrip": "DEPRECATED. Assigned value is specified to \"PhPickupFast\" and \"PhPickupSlow\" properties for backwards compatibility. See \"PhPickupFast\" and \"PhPickupSlow\" properties.",
    "Recloser.groundtrip": "DEPRECATED. Assigned value is specified to \"GndPickupFast\" and \"GndPickupSlow\" properties for backwards compatibility. See \"GndPickupFast\" and \"GndPickupSlow\" properties.",
    "Recloser.phslowpickup": "Multiplier for the phase slow TCC curve. Defaults to 1.0.",
    "Recloser.gndslowpickup": "Multiplier for the ground slow TCC curve. Defaults to 1.0.",
    "Recloser.tdphdelayed": "DEPRECATED. Assigned value is specified to \"TDPhSlow\" property for backwards compatibility. See \"TDPhSlow\" property.",
    "Recloser.tdgrdelayed": "DEPRECATED. Assigned value is specified to \"TDGndSlow\" property for backwards compatibility. See \"TDGndSlow\" property.",
    "Recloser.delay": "DEPRECATED. See \"MechanicalDelay\" property.",
    "Recloser.phaseinst": "DEPRECATED. See \"PhInst\" property.",
    "Recloser.groundinst": "DEPRECATED. See \"GndInst\" property.",
    "Recloser.tdgrfast": "DEPRECATED. See \"TDGndFast\" property.",
    # Fuse r4133 props 1-12 (Version8/Source/Controls/fuse.pas)
    "Fuse.monitoredobj": "Full object name of the circuit element, typically a line, transformer, load, or generator, to which the Fuse is connected. This is the \"monitored\" element. There is no default; must be specified.",
    "Fuse.monitoredterm": "Number of the terminal of the circuit element to which the Fuse is connected. 1 or 2, typically.  Default is 1.",
    "Fuse.switchedobj": "Name of circuit element switch that the Fuse controls. Specify the full object name.Defaults to the same as the Monitored element. This is the \"controlled\" element.",
    "Fuse.switchedterm": "Number of the terminal of the controlled element in which the switch is controlled by the Fuse. 1 or 2, typically.  Default is 1.  Assumes all phases of the element have a fuse of this type.",
    "Fuse.fusecurve": "Name of the TCC Curve object that determines the fuse blowing.  Must have been previously defined as a TCC_Curve object or specified as \"none\" (ignored). If \"none\", fuse sampling will be skipped and device will not blow for any current level. Default is \"none\". Multiplying the current values in the curve by the \"CurveMultiplier\" value gives the actual current.",
    "Fuse.ratedcurrent": "Fuse continuous rated current in Amps. Defaults to 0. Not used internally for either power flow or reporting.",
    "Fuse.delay": "Fixed delay time (sec) added to Fuse blowing time determined from the TCC curve. Default is 0.0. Used to represent fuse clearing time or any other delay.",
    "Fuse.action": "DEPRECATED. See \"State\" property.",
    "Fuse.normal": "ARRAY of strings {Open | Closed} representing the Normal state of the fuse in each phase of the controlled element. The fuse reverts to this state for reset, change of mode, etc. Defaults to \"State\" if not specifically declared.",
    "Fuse.state": "ARRAY of strings {Open | Closed} representing the Actual state of the fuse in each phase of the controlled element. Upon setting, immediately forces state of fuse(s). Simulates manual control on Fuse. Defaults to Closed for all phases.",
    "Fuse.curvemultiplier": "Current multiplier for the TCC curve. Defaults to 1.0.",
    "Fuse.interruptingrating": "Fuse rated interrupting current in Amps. Defaults to 0. Not used internally for either power flow or reporting.",
    # SwtControl r4133 props 1-9 (Version8/Source/Controls/SwtControl.pas)
    "SwtControl.switchedobj": "Name of circuit element switch that the SwtControl operates. Specify the full object class and name.",
    "SwtControl.switchedterm": "Terminal number of the controlled element switch. 1 or 2, typically.  Default is 1.",
    "SwtControl.action": "DEPRECATED. See \"State\" property.",
    "SwtControl.lock": "{Yes | No} Controlled switch is locked in its present open / closed state or unlocked. When locked, the switch will not respond to either a manual state change issued by the user or a state change issued internally by OpenDSS when reseting the control.",
    "SwtControl.delay": "DEPRECATED.",
    "SwtControl.normal": "ARRAY of strings {Open | Closed} representing the Normal state of the switch in each phase of the controlled element. The switch reverts to this state for reset, change of mode, etc. Defaults to \"State\" if not specifically declared.  Setting this property to {Open | Closed} sets the normal state to the specified value for all phases (ganged operation).",
    "SwtControl.state": "ARRAY of strings {Open | Closed} representing the Actual state of the switch in each phase of the controlled element. Upon setting, immediately forces the state of the switch(es). Simulates manual control on Switch. Defaults to Closed for all phases. Setting this property to {Open | Closed} sets the actual state to the specified value for all phases (ganged operation).",
    "SwtControl.reset": "{Yes | No} If Yes, forces Reset of switch to Normal state and removes Lock independently of any internal reset command for mode change, etc.",
    "SwtControl.ratedcurrent": "Switch continuous rated current in Amps. Defaults to 0. Not used internally for either power flow or reporting.",
}
