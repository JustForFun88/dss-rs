//! GENERATED FILE — do not edit by hand.
//!
//! The oracle's gettext help catalog (`dss/messages/properties-en-US.mo` in
//! the pinned dss-python wheel), rendered by `Dump commands` (Pascal
//! `DumpAllDSSCommands`, `Utilities.pas:821-872`). Keys: `Command.<name>`,
//! `Executive.<option>`, `<Class>.<prop-lowercase>` (Pascal `DSSHelp` /
//! `TDSSClass.GetPropertyHelp`).
//!
//! Regenerate manually — with the exact pinned versions in
//! `tools/golden/PIN.txt` only, same rule as the goldens — via
//! `python tools/golden/gen_help_catalog.py`.

/// Sorted `(key, help)` pairs parsed from the pinned wheel's `.mo` catalog.
static HELP_CATALOG: &[(&str, &str)] = &[
    (
        "AutoTrans.%imag",
        "Percent magnetizing current. Default=0.0. Magnetizing branch is in parallel with windings in each phase. Also, see \"ppm_antifloat\".",
    ),
    (
        "AutoTrans.%loadloss",
        "Percent load loss at full load. The %R of the High and Low windings (1 and 2) are adjusted to agree at rated kVA loading.",
    ),
    (
        "AutoTrans.%noloadloss",
        "Percent no load losses at rated excitation voltage. Default is 0. Converts to a resistance in parallel with the magnetizing impedance in each winding.",
    ),
    (
        "AutoTrans.%r",
        "Percent ac resistance this winding.  This value is for the power flow model.Is derived from the full load losses in the transformer test report.",
    ),
    (
        "AutoTrans.%rs",
        "Use this property to specify all the winding ac %resistances using an array. Example:\n\nNew AutoTrans.T1 buses=[Hibus, lowbus] ~ %Rs=(0.2  0.3)",
    ),
    (
        "AutoTrans.bank",
        "Name of the bank this transformer is part of, for CIM, MultiSpeak, and other interfaces.",
    ),
    ("AutoTrans.basefreq", "Base Frequency for ratings."),
    ("AutoTrans.bus", "Bus connection spec for this winding."),
    (
        "AutoTrans.buses",
        "Use this to specify all the bus connections at once using an array. Example:\n\nNew AutoTrans.T1 buses=[Hbus, Xbus]",
    ),
    (
        "AutoTrans.conn",
        "Connection of this winding {Series, wye*, Delta, LN, LL }. Default is \"wye\" with the neutral solidly grounded. \nFor AutoTrans, Winding 1 is always Series and Winding 2 (the Common winding) is always Wye. \nIf only 2 windings, no need to specify connections.",
    ),
    (
        "AutoTrans.conns",
        "Use this to specify all the Winding connections at once using an array. Example:\n\nNew AutoTrans.T1 buses=[Hbus, Xbus] ~ conns=(series, wye)",
    ),
    (
        "AutoTrans.core",
        "{Shell*|5-leg|3-Leg|1-phase|core-1-phase|4-leg} Core Type. Used for GIC analysis in auxiliary programs. Not used inside OpenDSS.",
    ),
    ("AutoTrans.emergamps", "Maximum or emerg current."),
    (
        "AutoTrans.emerghkva",
        "Emergency (contingency)  kVA rating of H winding (winding 1+2).  Usually 140% - 150% of maximum nameplate rating, depending on load shape. Defaults to 150% of kVA rating of Winding 1.",
    ),
    (
        "AutoTrans.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    ("AutoTrans.faultrate", "Failure rate per year."),
    (
        "AutoTrans.flrise",
        "Temperature rise, deg C, for full load.  Default is 65.",
    ),
    (
        "AutoTrans.hsrise",
        "Hot spot temperature rise, deg C.  Default is 15.",
    ),
    (
        "AutoTrans.kv",
        "For 2-or 3-phase, enter phase-phase kV rating.  Otherwise, kV rating of the actual winding. Specify H terminal kV rating for Series winding.",
    ),
    (
        "AutoTrans.kva",
        "Base kVA rating of the winding. Side effect: forces change of max normal and emerg kVA ratings.If 2-winding AutoTrans, forces other winding to same value. When winding 1 is defined, all other windings are defaulted to the same rating and the first two winding resistances are defaulted to the %loadloss value.",
    ),
    (
        "AutoTrans.kvas",
        "Use this to specify the kVA ratings of all windings at once using an array.",
    ),
    (
        "AutoTrans.kvs",
        "Use this to specify the kV ratings of all windings at once using an array. Example:\n\nNew AutoTrans.T1 buses=[Hbus, Xbus] \n~ conns=(series, wye)\n~ kvs=(115, 12.47)\n\nSee kV= property for voltage rules.",
    ),
    (
        "AutoTrans.leadlag",
        "{Lead | Lag (default) | ANSI (default) | Euro } Designation in mixed Delta-wye connections the relationship between HV to LV winding. Default is ANSI 30 deg lag, e.g., Dy1 of Yd1 vector group. To get typical European Dy11 connection, specify either \"lead\" or \"Euro\"",
    ),
    (
        "AutoTrans.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "AutoTrans.m",
        "m Exponent for thermal properties in IEEE C57.  Typically 0.9 - 1.0",
    ),
    (
        "AutoTrans.maxtap",
        "Max per unit tap for the active winding.  Default is 1.10",
    ),
    (
        "AutoTrans.mintap",
        "Min per unit tap for the active winding.  Default is 0.90",
    ),
    (
        "AutoTrans.n",
        "n Exponent for thermal properties in IEEE C57.  Typically 0.8.",
    ),
    ("AutoTrans.normamps", "Normal rated current."),
    (
        "AutoTrans.normhkva",
        "Normal maximum kVA rating of H winding (winding 1+2).  Usually 100% - 110% of maximum nameplate rating, depending on load shape. Defaults to 110% of kVA rating of Winding 1.",
    ),
    (
        "AutoTrans.numtaps",
        "Total number of taps between min and max tap.  Default is 32 (16 raise and 16 lower taps about the neutral position). The neutral position is not counted.",
    ),
    (
        "AutoTrans.pctperm",
        "Percent of failures that become permanent.",
    ),
    (
        "AutoTrans.phases",
        "Number of phases this AutoTrans. Default is 3.",
    ),
    (
        "AutoTrans.ppm_antifloat",
        "Default=1 ppm.  Parts per million of AutoTrans winding VA rating connected to ground to protect against accidentally floating a winding without a reference. If positive then the effect is adding a very large reactance to ground.  If negative, then a capacitor.",
    ),
    (
        "AutoTrans.rdcohms",
        "Winding dc resistance in OHMS. Specify this for GIC analysis. From transformer test report (divide by number of phases). Defaults to 85% of %R property (the ac value that includes stray losses).",
    ),
    ("AutoTrans.repair", "Hours to repair."),
    (
        "AutoTrans.sub",
        "={Yes|No}  Designates whether this AutoTrans is to be considered a substation.Default is No.",
    ),
    (
        "AutoTrans.subname",
        "Substation Name. Optional. Default is null. If specified, printed on plots",
    ),
    ("AutoTrans.tap", "Per unit tap that this winding is on."),
    (
        "AutoTrans.taps",
        "Use this to specify the p.u. tap of all windings at once using an array.",
    ),
    (
        "AutoTrans.thermal",
        "Thermal time constant of the AutoTrans in hours.  Typically about 2.",
    ),
    (
        "AutoTrans.wdg",
        "Set this = to the number of the winding you wish to define.  Then set the values for this winding.  Winding 1 is always the Series winding. Winding 2 is always Common winding (wye connected). Repeat for each winding.  Alternatively, use the array collections (buses, kVAs, etc.) to define the windings.  Note: reactances are BETWEEN pairs of windings; they are not the property of a single winding.",
    ),
    (
        "AutoTrans.wdgcurrents",
        "(Read only) Makes winding currents available via return on query (? AutoTrans.TX.WdgCurrents). Order: Phase 1, Wdg 1, Wdg 2, ..., Phase 2 ...\n\nWARNING: If the transformer has open terminal(s), results may be wrong, i.e. avoid using this in those situations. For more information, see https://github.com/dss-extensions/dss-extensions/issues/24",
    ),
    (
        "AutoTrans.windings",
        "Number of windings, this AutoTrans. (Also is the number of terminals) Default is 2. This property triggers memory allocation for the AutoTrans and will cause other properties to revert to default values.",
    ),
    (
        "AutoTrans.xfmrcode",
        "Name of a library entry for transformer properties. The named XfmrCode must already be defined.",
    ),
    (
        "AutoTrans.xht",
        "Use this to specify the percent reactance, H-T (winding 1 to winding 3).  Use for 3-winding AutoTranss only. On the kVA base of winding 1(H-X). ",
    ),
    (
        "AutoTrans.xhx",
        "Use this to specify the percent reactance, H-L (winding 1 to winding 2).  Use for 2- or 3-winding AutoTranss. On the kVA base of winding 1(H-X). ",
    ),
    (
        "AutoTrans.xrconst",
        "={Yes|No} Default is NO. Signifies whether or not the X/R is assumed constant for harmonic studies.",
    ),
    (
        "AutoTrans.xscarray",
        "Use this to specify the percent reactance between all pairs of windings as an array. All values are on the kVA base of winding 1.  The order of the values is as follows:\n\n(x12 13 14... 23 24.. 34 ..)  \n\nThere will be n(n-1)/2 values, where n=number of windings.",
    ),
    (
        "AutoTrans.xxt",
        "Use this to specify the percent reactance, L-T (winding 2 to winding 3).  Use for 3-winding AutoTranss only. On the kVA base of winding 1(H-X).  ",
    ),
    (
        "CNData.capradius",
        "Equivalent conductor radius for capacitance calcs. Specify this for bundled conductors. Defaults to same value as radius. Define Diam or Radius property first.",
    ),
    (
        "CNData.diacable",
        "Diameter over cable; same units as radius; no default.",
    ),
    (
        "CNData.diains",
        "Diameter over insulation layer; same units as radius; no default. Establishes outer radius for capacitance calculation.",
    ),
    (
        "CNData.diam",
        "Diameter; Alternative method for entering radius.",
    ),
    (
        "CNData.diastrand",
        "Diameter of a concentric neutral strand; same units as core conductor radius; no default.",
    ),
    (
        "CNData.emergamps",
        "Emergency ampacity, amperes. Defaults to 1.5 * Normal Amps if not specified.",
    ),
    (
        "CNData.epsr",
        "Insulation layer relative permittivity; default is 2.3.",
    ),
    (
        "CNData.gmrac",
        "GMR at 60 Hz. Defaults to .7788*radius if not specified.",
    ),
    (
        "CNData.gmrstrand",
        "Geometric mean radius of a concentric neutral strand; same units as core conductor GMR; defaults to 0.7788 * CN strand radius.",
    ),
    (
        "CNData.gmrunits",
        "Units for GMR: {mi|kft|km|m|Ft|in|cm|mm} Default=none.",
    ),
    (
        "CNData.inslayer",
        "Insulation layer thickness; same units as radius; no default. With DiaIns, establishes inner radius for capacitance calculation.",
    ),
    (
        "CNData.k",
        "Number of concentric neutral strands; default is 2",
    ),
    (
        "CNData.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "CNData.normamps",
        "Normal ampacity, amperes. Defaults to Emergency amps/1.5 if not specified.",
    ),
    (
        "CNData.rac",
        "Resistance at 60 Hz per unit length. Defaults to 1.02*Rdc if not specified.",
    ),
    (
        "CNData.radius",
        "Outside radius of conductor. Defaults to GMR/0.7788 if not specified.",
    ),
    (
        "CNData.radunits",
        "Units for outside radius: {mi|kft|km|m|Ft|in|cm|mm} Default=none.",
    ),
    (
        "CNData.ratings",
        "An array of ratings to be used when the seasonal ratings flag is True. It can be used to insert\nmultiple ratings to change during a QSTS simulation to evaluate different ratings in lines.",
    ),
    (
        "CNData.rdc",
        "dc Resistance, ohms per unit length (see Runits). Defaults to Rac/1.02 if not specified.",
    ),
    (
        "CNData.rstrand",
        "AC resistance of a concentric neutral strand; same units as core conductor resistance; no default.",
    ),
    (
        "CNData.runits",
        "Length units for resistance: ohms per {mi|kft|km|m|Ft|in|cm|mm} Default=none.",
    ),
    (
        "CNData.seasons",
        "Defines the number of ratings to be defined for the wire, to be used only when defining seasonal ratings using the \"Ratings\" property.",
    ),
    (
        "CNData.semiconlayer",
        "{Yes/True | No/False}  Default is Yes. Existence of a semicon layer between the insulation layer and the concentric neutral strands. Affects calculation of shunt self admittances.",
    ),
    ("CapControl.basefreq", "Base Frequency for ratings."),
    (
        "CapControl.capacitor",
        "Name of Capacitor element which the CapControl controls. No Default; Must be specified.Do not specify the full object name; \"Capacitor\" is assumed for the object class.  Example:\n\nCapacitor=cap1",
    ),
    (
        "CapControl.controlsignal",
        "Load shape used for controlling the connection/disconnection of the capacitor to the grid, when the load shape is DIFFERENT than ZERO (0) the capacitor will be ON and connected to the grid. Otherwise, if the load shape value is EQUAL to ZERO (0) the capacitor bank will be OFF and disconnected from the grid.",
    ),
    (
        "CapControl.ctphase",
        "Number of the phase being monitored for CURRENT control or one of {AVG | MAX | MIN} for all phases. Default=1. If delta or L-L connection, enter the first or the two phases being monitored [1-2, 2-3, 3-1]. Must be less than the number of phases. Does not apply to kvar control which uses all phases by default.",
    ),
    (
        "CapControl.ctratio",
        "Ratio of the CT from line amps to control ampere setting for current and kvar control types. ",
    ),
    (
        "CapControl.deadtime",
        "Dead time after capacitor is turned OFF before it can be turned back ON. Default is 300 sec.",
    ),
    (
        "CapControl.delay",
        "Time delay, in seconds, from when the control is armed before it sends out the switching command to turn ON.  The control may reset before the action actually occurs. This is used to determine which capacity control will act first. Default is 15.  You may specify any floating point number to achieve a model of whatever condition is necessary.",
    ),
    (
        "CapControl.delayoff",
        "Time delay, in seconds, for control to turn OFF when present state is ON. Default is 15.",
    ),
    (
        "CapControl.element",
        "Full object name of the circuit element, typically a line or transformer, to which the capacitor control's PT and/or CT are connected.There is no default; must be specified.",
    ),
    (
        "CapControl.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "CapControl.eventlog",
        "{Yes/True | No/False*} Default is NO for CapControl. Log control actions to Eventlog.",
    ),
    (
        "CapControl.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "CapControl.offsetting",
        "Value at which the control arms to switch the capacitor OFF. (See help for ONsetting)For Time control, is OK to have Off time the next day ( < On time)",
    ),
    (
        "CapControl.onsetting",
        "Value at which the control arms to switch the capacitor ON (or ratchet up a step).  \n\nType of Control:\n\nCurrent: Line Amps / CTratio\nVoltage: Line-Neutral (or Line-Line for delta) Volts / PTratio\nkvar:    Total kvar, all phases (3-phase for pos seq model). This is directional. \nPF:      Power Factor, Total power in monitored terminal. Negative for Leading. \nTime:    Hrs from Midnight as a floating point number (decimal). 7:30am would be entered as 7.5.\nFollow:  Follows a loadshape (ControlSignal) to determine when to turn ON/OFF the capacitor. If the value is different than 0 the capacitor will connect to the grid, otherwise, it will be disconnected.",
    ),
    (
        "CapControl.pctminkvar",
        "For PF control option, min percent of total bank kvar at which control will close capacitor switch. Default = 50.",
    ),
    (
        "CapControl.ptphase",
        "Number of the phase being monitored for VOLTAGE control or one of {AVG | MAX | MIN} for all phases. Default=1. If delta or L-L connection, enter the first or the two phases being monitored [1-2, 2-3, 3-1]. Must be less than the number of phases. Does not apply to kvar control which uses all phases by default.",
    ),
    (
        "CapControl.ptratio",
        "Ratio of the PT that converts the monitored voltage to the control voltage. Default is 60.  If the capacitor is Wye, the 1st phase line-to-neutral voltage is monitored.  Else, the line-to-line voltage (1st - 2nd phase) is monitored.",
    ),
    (
        "CapControl.reset",
        "{Yes | No} If Yes, forces Reset of this CapControl.",
    ),
    (
        "CapControl.terminal",
        "Number of the terminal of the circuit element to which the CapControl is connected. 1 or 2, typically.  Default is 1.",
    ),
    (
        "CapControl.type",
        "{Current | Voltage | kvar | PF | Time | Follow} Control type.  Specify the ONsetting and OFFsetting appropriately with the type of control. (See help for ONsetting)",
    ),
    (
        "CapControl.userdata",
        "String (in quotes or parentheses if necessary) that gets passed to the user-written CapControl model Edit function for defining the data required for that model. ",
    ),
    (
        "CapControl.usermodel",
        "Name of DLL containing user-written CapControl model, overriding the default model.  Set to \"none\" to negate previous setting. ",
    ),
    (
        "CapControl.vbus",
        "Name of bus to use for voltage override function. Default is bus at monitored terminal. Sometimes it is useful to monitor a bus in another location to emulate various DMS control algorithms.",
    ),
    (
        "CapControl.vmax",
        "Maximum voltage, in volts.  If the voltage across the capacitor divided by the PTRATIO is greater than this voltage, the capacitor will switch OFF regardless of other control settings. Default is 126 (goes with a PT ratio of 60 for 12.47 kV system).",
    ),
    (
        "CapControl.vmin",
        "Minimum voltage, in volts.  If the voltage across the capacitor divided by the PTRATIO is less than this voltage, the capacitor will switch ON regardless of other control settings. Default is 115 (goes with a PT ratio of 60 for 12.47 kV system).",
    ),
    (
        "CapControl.voltoverride",
        "{Yes | No}  Default is No.  Switch to indicate whether VOLTAGE OVERRIDE is to be considered. Vmax and Vmin must be set to reasonable values if this property is Yes.",
    ),
    ("Capacitor.basefreq", "Base Frequency for ratings."),
    (
        "Capacitor.bus1",
        "Name of first bus of 2-terminal capacitor. Examples:\nbus1=busname\nbus1=busname.1.2.3\n\nIf only one bus specified, Bus2 will default to this bus, Node 0, and the capacitor will be a Yg shunt bank.",
    ),
    (
        "Capacitor.bus2",
        "Name of 2nd bus. Defaults to all phases connected to first bus, node 0, (Shunt Wye Connection) except when Bus2 explicitly specified. \n\nNot necessary to specify for delta (LL) connection.",
    ),
    (
        "Capacitor.cmatrix",
        "Nodal cap. matrix, lower triangle, microfarads, of the following form:\n\ncmatrix=\"c11 | -c21 c22 | -c31 -c32 c33\"\n\nAll steps are assumed the same if this property is used.",
    ),
    (
        "Capacitor.conn",
        "={wye | delta |LN |LL}  Default is wye, which is equivalent to LN",
    ),
    (
        "Capacitor.cuf",
        "ARRAY of Capacitance, each phase, for each step, microfarads.\nSee Rules for NumSteps.",
    ),
    (
        "Capacitor.emergamps",
        "Maximum or emerg current. Defaults to 180% of per-phase rated current.",
    ),
    (
        "Capacitor.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    ("Capacitor.faultrate", "Failure rate per year."),
    (
        "Capacitor.harm",
        "ARRAY of harmonics to which each step is tuned. Zero is interpreted as meaning zero reactance (no filter). Default is zero.",
    ),
    (
        "Capacitor.kv",
        "For 2, 3-phase, kV phase-phase. Otherwise specify actual can rating.",
    ),
    (
        "Capacitor.kvar",
        "Total kvar, if one step, or ARRAY of kvar ratings for each step.  Evenly divided among phases. See rules for NUMSTEPS.",
    ),
    (
        "Capacitor.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Capacitor.normamps",
        "Normal rated current. Defaults to 180% of per-phase rated current.",
    ),
    (
        "Capacitor.numsteps",
        "Number of steps in this capacitor bank. Default = 1. Forces reallocation of the capacitance, reactor, and states array.  Rules: If this property was previously =1, the value in the kvar property is divided equally among the steps. The kvar property does not need to be reset if that is accurate.  If the Cuf or Cmatrix property was used previously, all steps are set to the value of the first step. The states property is set to all steps on. All filter steps are set to the same harmonic. If this property was previously >1, the arrays are reallocated, but no values are altered. You must SUBSEQUENTLY assign all array properties.",
    ),
    (
        "Capacitor.pctperm",
        "Percent of failures that become permanent.",
    ),
    ("Capacitor.phases", "Number of phases."),
    (
        "Capacitor.r",
        "ARRAY of series resistance in each phase (line), ohms. Default is 0.0",
    ),
    ("Capacitor.repair", "Hours to repair."),
    (
        "Capacitor.states",
        "ARRAY of integers {1|0} states representing the state of each step (on|off). Defaults to 1 when reallocated (on). Capcontrol will modify this array as it turns steps on or off.",
    ),
    (
        "Capacitor.xl",
        "ARRAY of series inductive reactance(s) in each phase (line) for filter, ohms at base frequency. Use this OR \"h\" property to define filter. Default is 0.0.",
    ),
    ("Command.//", "Comment.  Command line is ignored."),
    (
        "Command.?",
        "Inquiry for property value.  Result is put into GlobalResult and can be seen in the Result Window. Specify the full property name.\n\nExample: ? Line.Line1.R1\n\nNote you can set this property merely by saying:\nLine.line1.r1=.058",
    ),
    (
        "Command._docontrolactions",
        "For step control of solution process: Pops control actions off the control queue according to the present control mode rules. Dispatches control actions to proper control element \"DoPendingAction\" handlers.",
    ),
    (
        "Command._initsnap",
        "For step control of solution process: Initialize iteration counters, etc. that normally occurs at the start of a snapshot solution process.",
    ),
    (
        "Command._samplecontrols",
        "For step control of solution process: Sample the control elements, which push control action requests onto the control queue.",
    ),
    (
        "Command._showcontrolqueue",
        "For step control of solution process: Show the present control queue contents.",
    ),
    (
        "Command._solvedirect",
        "For step control of solution process: Invoke direct solution function in DSS. Non-iterative solution of Y matrix and active sources only.",
    ),
    (
        "Command._solvenocontrol",
        "For step control of solution process: Solves the circuit in present state but does not check for control actions.",
    ),
    (
        "Command._solvepflow",
        "For step control of solution process: Invoke iterative power flow solution function of DSS directly.",
    ),
    ("Command.abort", "Aborts all the simulations running"),
    (
        "Command.about",
        "Display \"About Box\".  (Result string set to Version string.)",
    ),
    (
        "Command.addbusmarker",
        "Add a marker to a bus in a circuit plot. Markers must be added before issuing the Plot command. Effect is persistent until circuit is cleared. See also ClearBusMarkers command. Example: \n\nClearBusMarkers    !...Clears any previous bus markers\nAddBusMarker Bus=Mybusname code=5 color=Red size=3\n\nYou can use any of the standard color names  or RGB numbers. See Help on C1 property in Plot command.",
    ),
    (
        "Command.aggregateprofiles",
        "Aggregates the load shapes in the model using the number of zones given in the argument.\nUse this command when the number of load shapes is considerably big, this algorithm will simplify\nthe amount of load shapes in order to make the memory consumption lower for the model.\nThe output of this algorithm is a script describing the new load shapes and their application into loads across the model.\nThe argument on this command can be: Actual/pu to define the units in which the load profiles are.\nCheck the OpenDSS user manual for details\n**Currently not supported on DSS-Extensions, tracked in https://github.com/dss-extensions/dss_capi/issues/46**",
    ),
    (
        "Command.alignfile",
        "Alignfile [file=]filename.  Aligns DSS script files in columns for easier reading.",
    ),
    (
        "Command.allocateloads",
        "Estimates the allocation factors for loads that are defined using the XFKVA property. Requires that energymeter objects be defined with the PEAKCURRENT property set. Loads that are not in the zone of an energymeter cannot be allocated.",
    ),
    (
        "Command.allpceatbus",
        "Brings back the names of all PCE connected to the bus specified in the argument.\nThe command goes as follows:\n\nAllPCEatBus myBus\n\nWhere \"myBus\" is the name of the bus of interest",
    ),
    (
        "Command.allpdeatbus",
        "Brings back the names of all PDE connected to the bus specified in the argument.\nThe command goes as follows:\n\nAllPDEatBus myBus\n\nWhere \"myBus\" is the name of the bus of interest",
    ),
    (
        "Command.batchedit",
        "Batch edit objects in the same class. Example: BatchEdit Load..* duty=duty_shape\nIn place of the object name, supply a PERL regular expression. .* matches all names.\nThe subsequent parameter string is applied to each object selected.",
    ),
    (
        "Command.buildy",
        "Forces rebuild of Y matrix upon next Solve command regardless of need. The usual reason for doing this would be to reset the matrix for another load level when using LoadModel=PowerFlow (the default) when the system is difficult to solve when the load is far from its base value.  Works by invalidating the Y primitive matrices for all the Power Conversion elements.",
    ),
    (
        "Command.buscoords",
        "Define x,y coordinates for buses.  Execute after Solve or MakeBusList command is executed so that bus lists are defined.Reads coordinates from a CSV file with records of the form: busname, x, y.\n\nExample: BusCoords [file=]xxxx.csv",
    ),
    (
        "Command.calcincmatrix",
        "Calculates the incidence matrix of the Active Circuit",
    ),
    (
        "Command.calcincmatrix_o",
        "Calculates the incidence matrix of the Active Circuit. However, in this case the matrix will be calculated considering its hierarchical order,listing the buses starting from the substation to the farthest load in the model",
    ),
    (
        "Command.calclaplacian",
        "Calculate the laplacian matrix using the incidence matrix previously calculated. Before calling this command the incidence matrix needs to be calculated using calcincmatrix/calcincmatrix_o.",
    ),
    (
        "Command.calcvoltagebases",
        "Calculates voltage base for buses based on voltage bases defined with Set voltagebases=... command.",
    ),
    (
        "Command.capacity",
        "Find the maximum load the active circuit can serve in the PRESENT YEAR. Uses the EnergyMeter objects with the registers set with the SET UEREGS= (..) command for the AutoAdd functions.  Syntax (defaults shown):\n\ncapacity [start=]0.9 [increment=]0.005\n\nReturns the metered kW (load + losses - generation) and per unit load multiplier for the loading level at which something in the system reports an overload or undervoltage. If no violations, then it returns the metered kW for peak load for the year (1.0 multiplier). Aborts and returns 0 if no energymeters.",
    ),
    (
        "Command.cd",
        "Change default directory to specified directory\n\nCD dirname\nOn OpenDSS, this actually changes the current working directory of the whole hosting process.\nFor DSS-Extensions, there is an option to track the directory internally, while avoiding process-wide changes. This is required to allow running multiple instances of the DSS engine in the same process while keeping a consistent state. This option is currently disabled, but it will be enabled in a future version.",
    ),
    (
        "Command.cktlosses",
        "Returns the total losses for the active circuit in the Result string in kW, kvar.",
    ),
    (
        "Command.classes",
        "List of intrinsic DSS Classes. Returns comma-separated list in Result variable.",
    ),
    (
        "Command.cleanup",
        "Force execution of the end-of-time-step cleanup functions that samples/saves meters and updates selected state variables such as storage level",
    ),
    ("Command.clear", "Clear all circuits currently in memory."),
    (
        "Command.clearall",
        "Clears all the circuits and all the actors, after this instruction there will be only 1 actor (actor 1) and will be the active actor",
    ),
    (
        "Command.clearbusmarkers",
        "Clear all bus markers created with the AddBusMarker command.",
    ),
    (
        "Command.clone",
        "Clones the active circuit. This command creates as many copies of the active circuit as indicated in the argument if the number of requested clones does not overpasses the number of local CPUs. The form of this command is clone X whereX is the number of clones to be created",
    ),
    ("Command.close", "Opposite of the Open command."),
    (
        "Command.closedi",
        "Close all DI files ... useful at end of yearly solution where DI files are left open. (Reset and Set Year=nnn will also close the DI files)",
    ),
    (
        "Command.comhelp",
        "Shows the documentation file for the COM interface.This file provides guidance on the properties and methods included in the COM interface as well as examples and tips. Use this file to learn more about the COM interface and its different interfaces or just as a reference guide.",
    ),
    (
        "Command.comparecases",
        "[Case1=]casename [case2=]casename [register=](register number) [meter=]{Totals* | SystemMeter | metername}. \nCompares yearly simulations of two specified cases with respect to the quantity in the designated register from the designated meter file. Defaults: Register=9 meter=Totals.  Example:\n\nComparecases base pvgens 10",
    ),
    (
        "Command.compile",
        "Reads the designated file name containing DSS commands and processes them as if they were entered directly into the command line. The file is said to be \"compiled.\" Similar to \"redirect\" except changes the default directory to the path of the specified file.\n\nSyntax:\nCompile filename\n\n**Note**: on DSS-Extensions, we recommend using relative paths for `Redirect` and `Compile`, when possible. This is especially useful when running scripts from ZIP archives.",
    ),
    (
        "Command.connect",
        "Request to create a TCP/IP socket to communicate data with external modules. This function requires the host address and TCP port to connect. **Not supported on DSS-Extensions**",
    ),
    (
        "Command.currents",
        "Returns the currents for each conductor of ALL terminals of the active circuit element in the Result string. (See Select command.)Returned as comma-separated magnitude and angle.",
    ),
    (
        "Command.cvrtloadshapes",
        "Convert all Loadshapes presently loaded into either files of single or files of double. Usually files of singles are adequate precision for loadshapes.  Syntax:\n\ncvrtloadshapes type=sng  (this is the default)\ncvrtloadshapes type=dbl\n\nA DSS script for loading the loadshapes from the created files is produced and displayed in the default editor. ",
    ),
    (
        "Command.di_plot",
        "[case=]casename [year=]yr [registers=](reg1, reg2,...)  [peak=]y/n  [meter=]metername\nPlots demand interval (DI) results from yearly simulation cases.  Plots selected registers from selected meter file (default = DI_Totals.csv).  Peak defaults to NO.  If YES, only daily peak of specified registers is plotted. Example:\n\n DI_Plot basecase year=5 registers=(9,11) no",
    ),
    (
        "Command.disable",
        "Disables a circuit element or entire class.  Example:\nDisable load.loadxxx\nDisable generator.*  (Disables all generators)\n\nThe item remains defined, but is not included in the solution.",
    ),
    (
        "Command.disconnect",
        "Request to terminate a TCP/IP socket. This function requires the host address and TCP port to disconnect. **Not supported on DSS-Extensions**",
    ),
    (
        "Command.distribute",
        "kw=nn how={Proportional* | Uniform |Random | Skip} skip=nn PF=nn file=filename MW=nn What=[Generator*|Load]\n\nCreates a DSS script file to distribute Generator or Load objects on the system in the manner specified by \"how\".\nkW = total generation to be distributed (default=1000) \nhow= process name as indicated (default=proportional to load)\nskip = no. of buses to skip for \"How=Skip\" (default=1)\nPF = power factor for new generators (default=1.0)\nfile = name of file to save (default=distgenerators.dss or distloads.dss)\nMW = alternate way to specify kW (default = 1)\nWhat = what type of device to add, Generator (default) or Load",
    ),
    (
        "Command.doscmd",
        "Do a DOS command. Sends the command \"cmd ... \" to Windows. Execute the \"cmd /?\" command in a DOS window to see the options. To do a DOS command and automatically exit, do \n\nDOScmd /c ...command string ...\n\nTo keep the DOS window open, use /k switch.\n**Note**: DOScmd is deprecated and disabled by default on DSS-Extensions. Remember to enable it through the `AllowDOSCmd` API before using on scripts.",
    ),
    (
        "Command.dump",
        "Display the properties of either a specific DSS object or a complete dump on all variables in the problem (Warning! Could be very large!). Brings up the default text editor with the text file written by this command.\n Syntax: dump [class.obj] [debug]\n Examples:\n\n Dump line.line1 \n Dump solution  (dumps all solution vars) \n Dump commands  (dumps all commands to a text file) \n Dump transformer.*  (dumps all transformers)\n Dump ALLOCationfactors  (load allocation factors)\n Dump Buslist    (bus name hash list)\n Dump Devicelist    (Device name hash list)\n Dump      (dumps all objects in circuit) ",
    ),
    (
        "Command.edit",
        "Edit an object. The object is selected and it then becomes the active object.\n\nNote that Edit is the default command.  You many change a property value simply by giving the full property name and the new value, for example:\n\nline.line1.r1=.04\nvsource.source.kvll=230",
    ),
    (
        "Command.enable",
        "Enables a circuit element or entire class.  Example:\nEnable load.loadxxx\nEnable generator.*  (enables all generators)",
    ),
    (
        "Command.estimate",
        "Execute state estimator on present circuit given present sensor values.",
    ),
    (
        "Command.export",
        "Export various solution values to CSV (or XML) files for import into other programs. Creates a new file except for Energymeter and Generator objects, for which the results for each device of this class are APPENDED to the CSV File. You may export to a specific file by specifying the file name as the LAST parameter on the line. For example:\n\n  Export Voltage Myvoltagefile.csv\n\nOtherwise, the default file names shown in the Export help are used. For Energymeter and Generator, specifying the switch \"/multiple\" (or /m) for the file name will cause a separate file to be written for each meter or generator. The default is for a single file containing all elements.\n\nMay be abreviated Export V, Export C, etc.  Default is \"V\" for voltages. If Set ShowExport=Yes, the output file will be automatically displayed in the default editor. Otherwise, you must open the file separately. The name appears in the Result window.",
    ),
    (
        "Command.exportoverloads",
        "Exports the overloads report with the content available at the moment of the call. It only affects the overloads report for the active actor.",
    ),
    (
        "Command.exportvviolations",
        "Exports the voltage violations1 report with the content available at the moment of the call. It only affects the voltage violations report for the active actor.",
    ),
    (
        "Command.fileedit",
        "Edit specified file in default text file editor (see Set Editor= option).\n\nFileedit EXP_METERS.csv (brings up the meters export file)\n\n\"FileEdit\" may be abbreviated to a unique character string.",
    ),
    (
        "Command.finishtimestep",
        "Do Cleanup, sample monitors, and increment time.",
    ),
    (
        "Command.fncspublish",
        "Read FNCS publication topics from a JSON file",
    ),
    (
        "Command.formedit",
        "FormEdit [class.object].  Brings up form editor on active DSS object.",
    ),
    (
        "Command.get",
        "Returns DSS property values set using the Set command. Result is returned in Result property of the Text interface. \n\nVBA Example:\n\nDSSText.Command = \"Get mode\"\nAnswer = DSSText.Result\n\nMultiple properties may be requested on one get.  The results are appended and the individual values separated by commas.\n\nSee help on Set command for property names.",
    ),
    (
        "Command.gis",
        "Executes GIS options working with OpenDSS-GIS. See GIS command help. **Not supported on DSS-Extensions**",
    ),
    (
        "Command.giscoords",
        "Define x,y coordinates for buses using real GIS Latitude and Longitude values (decimal numbers).  Similar to BusCoords command. Execute after Solve command or MakeBusList command is executed so that bus lists are defined.Reads coordinates from a CSV file with records of the form: busname, Latitude, Longitude.\n\nExample: GISCoords [file=]xxxx.csv\n\nNote: For using only if OpenDSS-GIS is locally installed. **Not supported (no-op) on DSS-Extensions**",
    ),
    (
        "Command.helicspublish",
        "Read HELICS publication topics from a JSON file. **Currently not supported directly as a command on DSS-Extensions; see also https://helics.org/tools/**",
    ),
    ("Command.help", "Gives this display."),
    (
        "Command.init",
        "This command forces reinitialization of the solution for the next Solve command. To minimize iterations, most solutions start with the previous solution unless there has been a circuit change.  However, if the previous solution is bad, it may be necessary to re-initialize.  In most cases, a re-initiallization results in a zero-load power flow solution with only the series power delivery elements considered.",
    ),
    (
        "Command.interpolate",
        "{All | MeterName}  Default is \"All\". Interpolates coordinates for missing bus coordinates in meter zone",
    ),
    (
        "Command.latlongcoords",
        "Define x,y coordinates for buses using Latitude and Longitude values (decimal numbers).  Similar to BusCoords command. Execute after Solve command or MakeBusList command is executed so that bus lists are defined.Reads coordinates from a CSV file with records of the form: busname, Latitude, Longitude.\n\nExample: LatLongCoords [file=]xxxx.csv\n\nNote: Longitude is mapped to x coordinate and Latitude is mapped to y coordinate.",
    ),
    (
        "Command.losses",
        "Returns the total losses for the active circuit element (see Select command) in the Result string in kW, kvar.",
    ),
    (
        "Command.m",
        "Continuation of editing on the active object. An abbreviation for More",
    ),
    (
        "Command.makebuslist",
        "Updates the buslist, if needed, using the currently enabled circuit elements.  (This happens automatically for Solve command.) See ReprocessBuses",
    ),
    (
        "Command.makeposseq",
        "Attempts to convert present circuit model to a positive sequence equivalent. It is recommended to Save the circuit after this and edit the saved version to correct possible misinterpretations.",
    ),
    (
        "Command.more",
        "Continuation of editing on the active object.",
    ),
    (
        "Command.new",
        "Create a new object within the DSS. Object becomes the active object\nExample: New Line.line1 ...",
    ),
    (
        "Command.newactor",
        "This command creates a new actor (OpenDSS Instance) and sets the new actor as the active actor. There can be only 1 circuit per actor. The NewActor command will increment the variable NumOfActors; however, if the number of actors is the same as the number of available CPUs the new actor will not be created generating an error message. This instruction will deliver the ID of the active actor. This command does not requires a precedent command.",
    ),
    (
        "Command.next",
        "{Year | Hour | t}  Increments year, hour, or time as specified.  If \"t\" is specified, then increments time by current step size.",
    ),
    (
        "Command.nodediff",
        "Global result is set to voltage difference, volts and degrees, (Node1 - Node2) between any two nodes. Syntax:\n\n   NodeDiff Node1=MyBus.1 Node2=MyOtherBus.1",
    ),
    (
        "Command.nodelist",
        "[Circuit element name] (Optional) Returns a list of node numbers for all conductors of all terminals of the active circuit element in the Result window or interface.If the optional circuit element name is supplied, the program makes it the active element. Usage:\n\nNodeList\nNodeList Line.Myline",
    ),
    (
        "Command.obfuscate",
        "Change Bus and circuit element names to generic values to remove identifying names. Generally, you will follow this command immediately by a \"Save Circuit Dir=MyDirName\" command.",
    ),
    (
        "Command.open",
        "Opens the specified terminal and conductor of the specified circuit element. If the conductor is not specified, all phase conductors of the terminal are opened.\n\nExamples:\nOpen line.line1 2 \n(opens all phases of terminal 2)\n\nOpen line.line1 2 3\n(opens the 3rd conductor of terminal 2)\n\nAs a side effect, the bus of affected terminal is activated.",
    ),
    ("Command.panel", "Displays main control panel window."),
    (
        "Command.phaselosses",
        "Returns the losses for the active circuit element (see Select command) for each PHASE in the Result string in comma-separated kW, kvar pairs.",
    ),
    (
        "Command.plot",
        "Plots circuits and results in a variety of manners.  See separate Plot command help.",
    ),
    (
        "Command.powers",
        "Returns the powers (complex) going into each conductors of ALL terminals of the active circuit element in the Result string. (See Select command.)Returned as comma-separated kW and kvar.",
    ),
    (
        "Command.pstcalc",
        "Pst calculation. PstCalc Npts=nnn Voltages=[array] dt=nnn freq=nn lamp=120 or 230.\nSet Npts to a big enough value to hold the incoming voltage array. \ndt = time increment in seconds. default is 1\nfreq = base frequency in Hz 50 or 60. Default is default base frequency\nLamp= 120 for North America; 230 for Europe. Default is 120\n\nPSTCalc Npts=1900 V=[file=MyCSVFile.csv, Col=3, Header=y] dt=1 freq=60 lamp=120",
    ),
    (
        "Command.puvoltages",
        "Just like the Voltages command, except the voltages are in per unit if the kVbase at the bus is defined.",
    ),
    (
        "Command.quit",
        "Shuts down DSS unless this is the DLL version.  Then it does nothing;  DLL parent is responsible for shutting down the DLL.",
    ),
    (
        "Command.reconductor",
        "Reconductor a line section. Must be in an EnergyMeter zone. \nSyntax: Reconductor Line1=... Line2=... {LineCode= | Geometry = } EditString=\"...\" NPhases=#\nLine1 and Line2 may be given in any order. All lines in the path between the two are redefined with either the LineCode or Geometry (not both). You may also add an optional string the alter any other line properties. The edit string should be enclosed in quotes or parentheses or brackets.\nNphases is an optional filter on the number of phases in line segments to change.",
    ),
    (
        "Command.redirect",
        "Reads the designated file name containing DSS commands and processes them as if they were entered directly into the command line. Similar to \"Compile\", but leaves current directory where it was when Redirect command is invoked.Can temporarily change to subdirectories if nested Redirect commands require.\n\nex:  redirect filename\n\n**Note**: on DSS-Extensions, we recommend using relative paths for `Redirect` and `Compile`, when possible. This is especially useful when running scripts from ZIP archives.",
    ),
    (
        "Command.reduce",
        "{All | MeterName}  Default is \"All\".  Reduce the circuit according to reduction options. See \"Set ReduceOptions\" and \"Set Keeplist\" options.Energymeter objects actually perform the reduction.  \"All\" causes all meters to reduce their zones.",
    ),
    (
        "Command.refine_buslevels",
        "This function takes the bus levels array and traces all the possible paths considering the longest paths from the substation to the farthest branches within the circuit. Then, the new paths are filled with 0 to complement the original levels proposed by the calcincmatrix_o command.",
    ),
    (
        "Command.relcalc",
        "[restore=Y/N]Perform reliability calcs: Failure rates and number of interruptions. \n\nOptional parameter:\n\nIf restore=y automatic restoration of unfaulted section is assumed.",
    ),
    (
        "Command.remove",
        "{ElementName=} [KeepLoad=Y*/N] [EditString=\"...\"] Remove (disable) all branches downline from the PDelement named by \"ElementName\" property. Circuit must have an Energymeter on this branch. If KeepLoad=Y (default) a new Load element is defined and kW, kvar set to present power flow solution for the first element eliminated. The EditString is applied to each new Load element defined. \nIf KeepLoad=N, all downline elements are disabled. Examples: \n\nRemove Line.Lin3021\nRemove Line.L22 Editstring=\"Daily=Dailycurve Duty=SolarShape\nRemove Line.L333 KeepLoad=No",
    ),
    (
        "Command.rephase",
        "Generates a script to change the phase designation of all lines downstream from a start in line. Useful for such things as moving a single-phase lateral from one phase to another and keep the phase designation consistent for reporting functions that need it to be (not required for simply solving). \n\nStartLine=... PhaseDesignation=\"...\"  EditString=\"...\" ScriptFileName=... StopAtTransformers=Y/N/T/F\n\nEnclose the PhaseDesignation in quotes since it contains periods (dots).\nYou may add and optional EditString to edit any other line properties.\n\nRephase StartLine=Line.L100  PhaseDesignation=\".2\"  EditString=\"phases=1\" ScriptFile=Myphasechangefile.dss  Stop=No",
    ),
    (
        "Command.reprocessbuses",
        "Forces reprocessing of bus definitions whether there has been a change or not. Use for rebuilding meter zone lists when a line length changes, for example or some other event that would not normally trigger an update to the bus list.",
    ),
    (
        "Command.reset",
        "{MOnitors | MEters | Faults | Controls | Eventlog | Keeplist |(no argument) } Resets all Monitors, Energymeters, etc. If no argument specified, resets all options listed.",
    ),
    (
        "Command.rotate",
        "Usage: Rotate [angle=]nnn.  Rotate circuit plotting coordinates by specified angle (degrees). ",
    ),
    (
        "Command.sample",
        "Force all monitors and meters to take a sample for the most recent solution. Keep in mind that meters will perform integration.",
    ),
    (
        "Command.save",
        "{Save [class=]{Meters | Circuit | Voltages | (classname)} [file=]filename [dir=]directory \n\nDefault class = Meters, which saves the present values in both monitors and energy meters in the active circuit. \n\n\"Save Circuit\" saves the present enabled circuit elements to the specified subdirectory in standard DSS form with a Master.dss file and separate files for each class of data. \n\nIf Dir= not specified a unique name based on the circuit name is created automatically. \n\nIf Dir= is specified, any existing files are overwritten. \n\n\"Save Voltages\" saves the present solution in a simple CSV format in a file called DSS_SavedVoltages. Used for VDIFF command.\n\nAny class can be saved to a file.  If no filename specified, the classname is used.",
    ),
    (
        "Command.select",
        "Selects an element and makes it the active element.  You can also specify the active terminal (default = 1).\n\nSyntax:\nSelect [element=]elementname  [terminal=]terminalnumber \n\nExample:\nSelect Line.Line1 \n~ R1=.1\n(continue editing)\n\nSelect Line.Line1 2 \nVoltages  (returns voltages at terminal 2 in Result)",
    ),
    (
        "Command.seqcurrents",
        "Returns the sequence currents into all terminals of the active circuit element (see Select command) in Result string.  Returned as comma-separated magnitude only values.Order of returned values: 0, 1, 2  (for each terminal).",
    ),
    (
        "Command.seqpowers",
        "Returns the sequence powers into all terminals of the active circuit element (see Select command) in Result string.  Returned as comma-separated kw, kvar pairs.Order of returned values: 0, 1, 2  (for each terminal).",
    ),
    (
        "Command.seqvoltages",
        "Returns the sequence voltages at all terminals of the active circuit element (see Select command) in Result string.  Returned as comma-separated magnitude only values.Order of returned values: 0, 1, 2  (for each terminal).",
    ),
    (
        "Command.set",
        "Used to set various DSS solution modes and options.  You may also set the options with the Solve command. See \"Options\" for help.",
    ),
    (
        "Command.setbusxy",
        "Bus=...  X=...  Y=... Set the X, Y coordinates for a single bus. Prerequisite: Bus must exist as a result of a Solve, CalcVoltageBases, or MakeBusList command.",
    ),
    (
        "Command.setkvbase",
        "Command to explicitly set the base voltage for a bus. Bus must be previously defined. Parameters in order are:\nBus = {bus name}\nkVLL = (line-to-line base kV)\nkVLN = (line-to-neutral base kV)\n\nkV base is normally given in line-to-line kV (phase-phase). However, it may also be specified by line-to-neutral kV.\nThe following examples are equivalent:\n\nsetkvbase Bus=B9654 kVLL=13.2\nsetkvbase B9654 13.2\nsetkvbase B9654 kvln=7.62",
    ),
    (
        "Command.setloadandgenkv",
        "Set load and generator object kv to agree with the bus they are connected to using the bus voltage base and connection type.",
    ),
    (
        "Command.show",
        "Writes selected results to a text file and brings up the default text editor (see Set Editor=....) with the file for you to browse.\n\nSee separate help on Show command. \n\nDefault is \"show voltages LN Seq\".  ",
    ),
    (
        "Command.solve",
        "Perform the solution of the present solution mode. You can set any option that you can set with the Set command (see Set). The Solve command is virtually synonymous with the Set command except that a solution is performed after the options are processed.",
    ),
    (
        "Command.solveall",
        "Solves all the circuits (Actors) loaded into memory by the user",
    ),
    (
        "Command.summary",
        "Returns a power flow summary of the most recent solution in the global result string.",
    ),
    (
        "Command.tear_circuit",
        "Estimates the buses for tearing the system in many parts as CPUs - 1 are in the local computer, is used for tearing the interconnected circuit into a balanced (same number of nodes) collection of subsystems for the A-Diakoptics algorithm. **Supported in dss-rs (A-Diakoptics ported per the official r3723 Delphi spec; the pinned dss_capi/dss-python oracle compiles this out).**",
    ),
    (
        "Command.top",
        "[class=]{Loadshape | Tshape | Monitor  } [object=]{ALL (Loadshapes only) | objectname}. Send specified object to TOP.  Loadshapes and TShapes must be hourly fixed interval. ",
    ),
    (
        "Command.totalpowers",
        "Returns the total powers (complex) at ALL terminals of the active circuit element in the Result string. (See Select command.)Returned as comma-separated kW and kvar.",
    ),
    (
        "Command.totals",
        "Totals all EnergyMeter objects in the circuit and reports register totals in the result string.",
    ),
    (
        "Command.updatestorage",
        "Update Storage elements based on present solution and time interval. ",
    ),
    (
        "Command.userclasses",
        "List of user-defined DSS Classes. Returns comma-separated list in Result variable.",
    ),
    (
        "Command.uuids",
        "Read UUIDs (v4) for class names and other CIM objects. Tab or comma-delimited file with full object name (or key) and UUID. Side effect is to start a new UUID list for the Export CIM100 command; the UUID list is freed after the Export UUIDs command.",
    ),
    (
        "Command.var",
        "Define and view script variables.  Variable names begin with \"@\"\n\nUsage:\n\nvar @varname1=values  @varname2=value2    ...\nvar @varname1  (shows the value of @varname1)\nvar            (displays all variables and values)\n\nExample of using a variable:\n\nFileEdit @LastFile",
    ),
    (
        "Command.variable",
        "[name=] MyVariableName  [Index=] IndexofMyVariable \n\nReturns the value of the specified state variable of the active circuit element, if a PCelement. Returns the value as a string in the Result window or the Text.Result interface if using one of the APIs. \n\nYou may specify the variable by name or by its index. You can determine the index using the VarNames command. If any part of the request is invalid, the Result is null.",
    ),
    (
        "Command.varnames",
        "Returns variable names for active element if PC element. Otherwise, returns null.",
    ),
    (
        "Command.varvalues",
        "Returns variable values for active element if PC element. Otherwise, returns null.",
    ),
    (
        "Command.vdiff",
        "Displays the difference between the present solution and the last on saved using the SAVE VOLTAGES command.",
    ),
    (
        "Command.visualize",
        "[What=] one of {Currents* | Voltages | Powers} [element=]full_element_name  (class.name). Shows the selected quantity for selected element on a multiphase line drawing in phasor values.",
    ),
    (
        "Command.voltages",
        "Returns the voltages for the ACTIVE BUS in the Result string. For setting the active Bus, use the Select command or the Set Bus= option. Returned as magnitude and angle quantities, comma separated, one set per conductor of the terminal.",
    ),
    (
        "Command.wait",
        "Pauses the scripting thread until all the active actors are Ready to receive new commands (have finished all their tasks and are ready to receive new simulation orders).",
    ),
    (
        "Command.yearlycurves",
        "[cases=](case1, case2, ...) [registers=](reg1, reg2, ...)  [meter=]{Totals* | SystemMeter | metername}Plots yearly curves for specified cases and registers. \nDefault: meter=Totals. Example: \n\nyearlycurves cases=(basecase, pvgens) registers=9",
    ),
    (
        "Command.ysc",
        "Returns full Ysc matrix for the ACTIVE BUS in comma-separated complex number form G + jB.",
    ),
    (
        "Command.zsc",
        "Returns full Zsc matrix for the ACTIVE BUS in comma-separated complex number form.",
    ),
    (
        "Command.zsc012",
        "Returns symmetrical component short circuit impedances Z0, Z1, and Z2 for the ACTIVE 3-PHASE BUS. Determined from Zsc matrix.",
    ),
    (
        "Command.zsc10",
        "Returns symmetrical component impedances, Z1, Z0 for the ACTIVE BUS in comma-separated R+jX form.",
    ),
    (
        "Command.zscrefresh",
        "Refreshes Zsc matrix for the ACTIVE BUS.",
    ),
    (
        "Command.~",
        "Continuation of editing on the active object. An abbreviation.\n\nExample:\nNew Line.Line1 Bus1=aaa  bus2=bbb\n~ R1=.058\n~ X1=.1121",
    ),
    (
        "DynamicExp.domain",
        "It is the domain for which the equation is defined, it can be one of [time*, dq]. By deafult, dynamic epxressions are defined in the time domain.",
    ),
    (
        "DynamicExp.expression",
        "It is the differential expression using OpenDSS RPN syntax. The expression must be contained within brackets in case of having multiple equations, for example:\n\nexpression=\"[w dt = 1 M / (P_m D*w - P_e -) *]\"",
    ),
    (
        "DynamicExp.nvariables",
        "(Int) Number of state variables to be considered in the differential equation.",
    ),
    (
        "DynamicExp.var",
        "(String) Activates the state variable using the given name.",
    ),
    (
        "DynamicExp.varidx",
        "(Int) read-only, returns the index of the active state variable.",
    ),
    (
        "DynamicExp.varnames",
        "([String]) Array of strings with the names of the state variables.",
    ),
    ("ESPVLControl.basefreq", "Base Frequency for ratings."),
    (
        "ESPVLControl.element",
        "Full object name of the circuit element, typically a line or transformer, which the control is monitoring. There is no default; must be specified.",
    ),
    (
        "ESPVLControl.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "ESPVLControl.forecast",
        "Loadshape object containing daily forecast.",
    ),
    (
        "ESPVLControl.kvarlimit",
        "Max kvar to be delivered through the element.  Uses same dead band as kW.",
    ),
    (
        "ESPVLControl.kwband",
        "Bandwidth (kW) of the dead band around the target limit.No dispatch changes are attempted if the power in the monitored terminal stays within this band.",
    ),
    (
        "ESPVLControl.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "ESPVLControl.localcontrollist",
        "Array list of ESPVLControl local controller objects to be dispatched by System Controller. If not specified, all ESPVLControl devices with type=local in the circuit not attached to another controller are assumed to be part of this controller's fleet.",
    ),
    (
        "ESPVLControl.localcontrolweights",
        "Array of proportional weights corresponding to each ESPVLControl local controller in the LocalControlList.",
    ),
    (
        "ESPVLControl.pvsystemlist",
        "Array list of PVSystem objects to be dispatched by a Local Controller. ",
    ),
    (
        "ESPVLControl.pvsystemweights",
        "Array of proportional weights corresponding to each PVSystem in the PVSystemList.",
    ),
    (
        "ESPVLControl.storagelist",
        "Array list of Storage objects to be dispatched by Local Controller. ",
    ),
    (
        "ESPVLControl.storageweights",
        "Array of proportional weights corresponding to each Storage object in the StorageControlList.",
    ),
    (
        "ESPVLControl.terminal",
        "Number of the terminal of the circuit element to which the ESPVLControl control is connected. 1 or 2, typically.  Default is 1. Make sure you have the direction on the power matching the sign of kWLimit.",
    ),
    (
        "ESPVLControl.type",
        "Type of controller.  1= System Controller; 2= Local controller. ",
    ),
    (
        "EnergyMeter.3phaselosses",
        "{Yes | No}  Default is YES. Compute Line losses and segregate by 3-phase and other (1- and 2-phase) line losses. ",
    ),
    (
        "EnergyMeter.action",
        "{Clear (reset) | Save | Take | Zonedump | Allocate | Reduce} \n\n(A)llocate = Allocate loads on the meter zone to match PeakCurrent.\n(C)lear = reset all registers to zero\n(R)educe = reduces zone by merging lines (see Set Keeplist & ReduceOption)\n(S)ave = saves the current register values to a file.\n   File name is \"MTR_metername.csv\".\n(T)ake = Takes a sample at present solution\n(Z)onedump = Dump names of elements in meter zone to a file\n   File name is \"Zone_metername.csv\".",
    ),
    ("EnergyMeter.basefreq", "Base Frequency for ratings."),
    (
        "EnergyMeter.caidi",
        "(Read only) Makes CAIDI result available via return on query (? energymeter.myMeter.CAIDI.",
    ),
    (
        "EnergyMeter.custinterrupts",
        "(Read only) Makes Total Customer Interrupts value result available via return on query (? energymeter.myMeter.CustInterrupts.",
    ),
    (
        "EnergyMeter.element",
        "Name (Full Object name) of element to which the monitor is connected.",
    ),
    (
        "EnergyMeter.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "EnergyMeter.int_duration",
        "Average annual duration, in hr, of interruptions for head of the meter zone (source side of zone or feeder).",
    ),
    (
        "EnergyMeter.int_rate",
        "Average number of annual interruptions for head of the meter zone (source side of zone or feeder).",
    ),
    (
        "EnergyMeter.kvaemerg",
        "Upper limit on kVA load in the zone, Emergency configuration. Default is 0.0 (ignored). Overrides limits on individual lines for overload UE. With \"LocalOnly=Yes\" option, uses only load in metered branch.",
    ),
    (
        "EnergyMeter.kvanormal",
        "Upper limit on kVA load in the zone, Normal configuration. Default is 0.0 (ignored). Overrides limits on individual lines for overload EEN. With \"LocalOnly=Yes\" option, uses only load in metered branch.",
    ),
    (
        "EnergyMeter.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "EnergyMeter.linelosses",
        "{Yes | No}  Default is YES. Compute Line losses. If NO, then none of the losses are computed.",
    ),
    (
        "EnergyMeter.localonly",
        "{Yes | No}  Default is NO.  If Yes, meter considers only the monitored element for EEN and UE calcs.  Uses whole zone for losses.",
    ),
    (
        "EnergyMeter.losses",
        "{Yes | No}  Default is YES. Compute Zone losses. If NO, then no losses at all are computed.",
    ),
    (
        "EnergyMeter.mask",
        "Mask for adding registers whenever all meters are totalized.  Array of floating point numbers representing the multiplier to be used for summing each register from this meter. Default = (1, 1, 1, 1, ... ).  You only have to enter as many as are changed (positional). Useful when two meters monitor same energy, etc.",
    ),
    (
        "EnergyMeter.option",
        "Enter a string ARRAY of any combination of the following. Options processed left-to-right:\n\n(E)xcess : (default) UE/EEN is estimate of energy over capacity \n(T)otal : UE/EEN is total energy after capacity exceeded\n(R)adial : (default) Treats zone as a radial circuit\n(M)esh : Treats zone as meshed network (not radial).\n(C)ombined : (default) Load UE/EEN computed from combination of overload and undervoltage.\n(V)oltage : Load UE/EEN computed based on voltage only.\n\nExample: option=(E, R)",
    ),
    (
        "EnergyMeter.peakcurrent",
        "ARRAY of current magnitudes representing the peak currents measured at this location for the load allocation function.  Default is (400, 400, 400). Enter one current for each phase",
    ),
    (
        "EnergyMeter.phasevoltagereport",
        "{Yes | No}  Default is NO.  Report min, max, and average phase voltages for the zone and tabulate by voltage base. Demand Intervals must be turned on (Set Demand=true) and voltage bases must be defined for this property to take effect. Result is in a separate report file.",
    ),
    (
        "EnergyMeter.saidi",
        "(Read only) Makes SAIDI result available via return on query (? energymeter.myMeter.SAIDI.",
    ),
    (
        "EnergyMeter.saifi",
        "(Read only) Makes SAIFI result available via return on query (? energymeter.myMeter.SAIFI.",
    ),
    (
        "EnergyMeter.saifikw",
        "(Read only) Makes SAIFIkW result available via return on query (? energymeter.myMeter.SAIFIkW.",
    ),
    (
        "EnergyMeter.seqlosses",
        "{Yes | No}  Default is YES. Compute Sequence losses in lines and segregate by line mode losses and zero mode losses.",
    ),
    (
        "EnergyMeter.terminal",
        "Number of the terminal of the circuit element to which the monitor is connected. 1 or 2, typically.",
    ),
    (
        "EnergyMeter.vbaselosses",
        "{Yes | No}  Default is YES. Compute losses and segregate by voltage base. If NO, then voltage-based tabulation is not reported.",
    ),
    (
        "EnergyMeter.xfmrlosses",
        "{Yes | No}  Default is YES. Compute Transformer losses. If NO, transformers are ignored in loss calculations.",
    ),
    (
        "EnergyMeter.zonelist",
        "ARRAY of full element names for this meter's zone.  Default is for meter to find it's own zone. If specified, DSS uses this list instead.  Can access the names in a single-column text file.  Examples: \n\nzonelist=[line.L1, transformer.T1, Line.L3] \nzonelist=(file=branchlist.txt)",
    ),
    (
        "Executive.%growth",
        "Set default annual growth rate, percent, for loads with no growth curve specified. Default is 2.5.",
    ),
    (
        "Executive.%mean",
        "Percent mean to use for global load multiplier. Default is 65%.",
    ),
    (
        "Executive.%normal",
        "Sets the Normal rating of all lines to a specified percent of the emergency rating.  Note: This action takes place immediately. Only the in-memory value is changed for the duration of the run.",
    ),
    (
        "Executive.%stddev",
        "Percent Standard deviation to use for global load multiplier. Default is 9%.",
    ),
    (
        "Executive.activeactor",
        "Gets/Sets the number of the active actor, if the value is * (set active actor=*), the commands send after this instruction will be applied to all the actors.",
    ),
    (
        "Executive.actorprogress",
        "Gets progress (%) for all the actors when performing a task",
    ),
    (
        "Executive.addtype",
        "{Generator | Capacitor} Default is Generator. Type of device for AutoAdd Mode.",
    ),
    (
        "Executive.adiakoptics",
        "{YES/TRUE | NO/FALSE} Activates the A-Diakoptics solution algorithm for using spatial parallelization on the feeder.\nThis parameter only affects Actor 1, no matter from which actor is called. When activated (True), OpenDSS will start the \ninitialization routine for the A-Diakoptics solution mode",
    ),
    (
        "Executive.algorithm",
        "{Normal | Newton}  Solution algorithm type.  Normal is a fixed point iteration that is a little quicker than the Newton iteration.  Normal is adequate for most radial distribution circuits.  Newton is more robust for circuits that are difficult to solve.Diakoptics is used for accelerating the simulation using multicore computers",
    ),
    (
        "Executive.allocationfactors",
        "Sets the connected kVA allocation factors for all loads in the active circuit to the value given.",
    ),
    (
        "Executive.allowduplicates",
        "{YES/TRUE | NO/FALSE}   Default is No. Flag to indicate if it is OK to have devices of same name in the same class. If No, then a New command is treated as an Edit command. If Yes, then a New command will always result in a device being added.",
    ),
    (
        "Executive.autobuslist",
        "Array of bus names to include in AutoAdd searches. Or, you can specify a text file holding the names, one to a line, by using the syntax (file=filename) instead of the actual array elements. Default is null, which results in the program using either the buses in the EnergyMeter object zones or, if no EnergyMeters, all the buses, which can make for lengthy solution times. \n\nExamples:\n\nSet autobuslist=(bus1, bus2, bus3, ... )\nSet autobuslist=(file=buslist.txt)",
    ),
    (
        "Executive.basefrequency",
        "Default = 60. Set the fundamental frequency for harmonic solution and the default base frequency for all impedance quantities. Side effect: also changes the value of the solution frequency. Saved as default for next circuit.",
    ),
    (
        "Executive.bus",
        "Set Active Bus by name.  Can also be done with Select and SetkVBase commands and the \"Set Terminal=\"  option. The bus connected to the active terminal becomes the active bus. See Zsc and Zsc012 commands.",
    ),
    (
        "Executive.capkvar",
        "Size of capacitor, kVAR, to automatically add to system.  Default is 600.0.",
    ),
    (
        "Executive.capmarkercode",
        "Numeric marker code (0..47 -- see Users Manual) for Capacitors. Default is 38.",
    ),
    (
        "Executive.capmarkersize",
        "Size of Capacitor marker. Default is 3.",
    ),
    (
        "Executive.casename",
        "Name of case for yearly simulations with demand interval data. Becomes the name of the subdirectory under which all the year data are stored. Default = circuit name \n\nSide Effect: Sets the prefix for output files",
    ),
    (
        "Executive.cfactors",
        "Sets the CFactors for for all loads in the active circuit to the value given.",
    ),
    ("Executive.circuit", "Set the active circuit by name."),
    (
        "Executive.cktmodel",
        "{Multiphase | Positive}  Default = Multiphase.  Designates whether circuit model is to interpreted as a normal multi-phase model or a positive-sequence only model",
    ),
    ("Executive.class", "Synonym for Type=. (See above)"),
    (
        "Executive.concatenatereports",
        "Activates/Deactivates the option for concatenate the reports generated by the existing actors, if Yes, every time the user calls a show/export monitor command the report will include the data generated by all the actors, otherwise the report will containThe data generated by the active actor",
    ),
    (
        "Executive.controlmode",
        "{OFF | STATIC |EVENT | TIME}  Default is \"STATIC\".  Control mode for the solution. Set to OFF to prevent controls from changing.\nSTATIC = Time does not advance.  Control actions are executed in order of shortest time to act until all actions are cleared from the control queue.  Use this mode for power flow solutions which may require several regulator tap changes per solution.\n\nEVENT = solution is event driven.  Only the control actions nearest in time are executed and the time is advanced automatically to the time of the event. \n\nTIME = solution is time driven.  Control actions are executed when the time for the pending action is reached or surpassed.\n\nControls may reset and may choose not to act when it comes their time. \nUse TIME mode when modeling a control externally to the DSS and a solution mode such as DAILY or DUTYCYCLE that advances time, or set the time (hour and sec) explicitly from the external program. ",
    ),
    (
        "Executive.coverage",
        "Percentage of coverage expected when estimating the longest paths on the circuit for tearing, the default coverage\nis the 90% (0.9), this value cannot exceed 1.0. When used with the \"Set\" command is used for the algorithm for estimating the paths within the circuit\nbut when the \"get\" command is used after executing the tear_circuit command it will deliver the actual coverage after running the algorithm",
    ),
    (
        "Executive.cpu",
        "(default -1)Gets/Sets the CPU to be used by the active actor. If negative (-1) means that the actor affinity is to all the CPUs and will be executed in the\nfirst available CPU and will be reallocated into another CPU dynamically if the operating system requires it. By setting a CPU number for an actor will force\nthe actor to be executed only on the specific CPU.",
    ),
    (
        "Executive.daisysize",
        "Default is 1.0. Relative size (a multiplier applied to default size) of daisy circles on daisy plot.",
    ),
    (
        "Executive.datapath",
        "Set the data path for files written or read by the DSS.\nDefaults to the user documents folder.\nIf the DataPath is not writable, output files will be written to the user application data folder.\nMay be Null.  Executes a CHDIR to this path if non-null.\nDoes not require a circuit defined.",
    ),
    (
        "Executive.defaultbasefrequency",
        "Set Default Base Frequency, Hz. Side effect: Sets solution Frequency and default Circuit Base Frequency. This value is saved when the DSS closes down.",
    ),
    (
        "Executive.defaultdaily",
        "Default daily load shape name. Default value is \"default\", which is a 24-hour curve defined when the DSS is started.",
    ),
    (
        "Executive.defaultyearly",
        "Default yearly load shape name. Default value is \"default\", which is a 24-hour curve defined when the DSS is started.",
    ),
    (
        "Executive.demandinterval",
        "{YES/TRUE | NO/FALSE} Default = no. Set for keeping demand interval data for daily, yearly, etc, simulations. Side Effect:  Resets all meters!!!",
    ),
    (
        "Executive.diverbose",
        "{YES/TRUE | NO/FALSE} Default = FALSE.  Set to Yes/True if you wish a separate demand interval (DI) file written for each meter.  Otherwise, only the totalizing meters are written.",
    ),
    (
        "Executive.dssvinstalled",
        "Returns Yes/No if the OpenDSS Viewer installation is detected in the local machine (Read Only)",
    ),
    (
        "Executive.earthmodel",
        "One of {Carson | FullCarson | Deri*}.  Default is Deri, which isa  fit to the Full Carson that works well into high frequencies. \"Carson\" is the simplified Carson method that is typically used for 50/60 Hz power flow programs. Applies only to Line objects that use LineGeometry objects to compute impedances.",
    ),
    (
        "Executive.editor",
        "Set the command string required to start up the editor preferred by the user. Does not require a circuit defined.",
    ),
    (
        "Executive.element",
        "Sets the active DSS element by name. You can use the complete object spec (class.name) or just the name.  if full name is specifed, class becomes the active class, also.",
    ),
    (
        "Executive.emergvmaxpu",
        "Maximum permissible per unit voltage for emergency (contingency) conditions. Default is 1.08.",
    ),
    (
        "Executive.emergvminpu",
        "Minimum permissible per unit voltage for emergency (contingency) conditions. Default is 0.90.",
    ),
    (
        "Executive.eventlogdefault",
        "{YES/TRUE | NO/FALSE*} Sets/gets the default for the eventlog. After changing this flags the model needs to be recompiled to take effect.",
    ),
    (
        "Executive.frequency",
        "Sets the frequency for the solution of the active circuit.",
    ),
    (
        "Executive.fusemarkercode",
        "Numeric marker code (0..47 see Users Manual) for Fuse elements. Default is 25.",
    ),
    (
        "Executive.fusemarkersize",
        "Size of Fuse marker. Default is 1.",
    ),
    (
        "Executive.genkw",
        "Size of generator, kW, to automatically add to system. Default is 1000.0",
    ),
    (
        "Executive.genmult",
        "Global multiplier for the kW output of every generator in the circuit. Default is 1.0. Applies to all but Autoadd solution modes. Ignored for generators designated as Status=Fixed.",
    ),
    (
        "Executive.genpf",
        "Power factor of generator to assume for automatic addition. Default is 1.0.",
    ),
    (
        "Executive.giscolor",
        "Color    : A Hex string defining 24 bit color in RGB format, e.g. , red = FF0000",
    ),
    (
        "Executive.giscoords",
        "[Coords] : An array of doubles defining the longitude and latitude for an area to be used as reference for the OpenDSS-GIS related commands, long1, lat1, long2, lat2",
    ),
    (
        "Executive.gisinstalled",
        "Returns Yes/No if the OpenDSS GIS installation is detected in the local machine (Read Only)",
    ),
    (
        "Executive.gisthickness",
        "Thickness: An integer defining the thickness (default = 3)",
    ),
    ("Executive.h", "Alternate name for time step size."),
    (
        "Executive.harmonics",
        "{ALL | (list of harmonics) }  Default = ALL. Array of harmonics for which to perform a solution in Harmonics mode. If ALL, then solution is performed for all harmonics defined in spectra currently being used. Otherwise, specify a more limited list such as: \n\n   Set Harmonics=(1 5 7 11 13)",
    ),
    (
        "Executive.hour",
        "Sets the hour used for the start time of the solution.",
    ),
    (
        "Executive.keeplist",
        "Array of bus names to keep when performing circuit reductions. You can specify a text file holding the names, one to a line, by using the syntax (file=filename) instead of the actual array elements. Command is cumulative (reset keeplist first). Reduction algorithm may keep other buses automatically. \n\nExamples:\n\nReset Keeplist (sets all buses to FALSE (no keep))\nSet KeepList=(bus1, bus2, bus3, ... )\nSet KeepList=(file=buslist.txt)",
    ),
    (
        "Executive.keepload",
        "Keeploads = Y/N option for ReduceOption Laterals option",
    ),
    (
        "Executive.ldcurve",
        "Set Load-Duration Curve. Global load multiplier is defined by this curve for LD1 and LD2 solution modes. Default is Nil.",
    ),
    (
        "Executive.linkbranches",
        "Get/set the names of the link branches used for tearing the circuit after initializing using set ADiakoptics = True. Using this instruction will set the Active Actor = 1\nIf ADiakoptics is not initialized, this instruction will return an error message",
    ),
    (
        "Executive.loadmodel",
        "{Powerflow | Admittance} depending on the type of solution you wish to perform. If admittance, a non-iterative, direct solution is done with all loads and generators modeled by their equivalent admittance.",
    ),
    (
        "Executive.loadmult",
        "Global load multiplier for this circuit.  Does not affect loads designated to be \"fixed\".  All other base kW values are multiplied by this number. Defaults to 1.0 when the circuit is created. As with other values, it always stays at the last value to which it was set until changed again.",
    ),
    (
        "Executive.loadshapeclass",
        "={Daily | Yearly | Duty | None*} Default loadshape class to use for mode=time and mode=dynamic simulations. Loads and generators, etc., will follow this shape as time is advanced. Default value is None. That is, Load will not vary with time.",
    ),
    (
        "Executive.log",
        "{YES/TRUE | NO/FALSE} Default = FALSE.  Significant solution events are added to the Event Log, primarily for debugging.",
    ),
    (
        "Executive.lossregs",
        "Which EnergyMeter register(s) to use for Losses in AutoAdd Mode. May be one or more registers.  if more than one, register values are summed together. Array of integer values > 0.  Defaults to 13 (for Zone kWh Losses). \n\nfor a list of EnergyMeter register numbers, do the \"Show Meters\" command after defining a circuit.",
    ),
    (
        "Executive.lossweight",
        "Weighting factor for Losses in AutoAdd functions.  Defaults to 1.0.\n\nAutoadd mode minimizes\n\n(Lossweight * Losses + UEweight * UE). \n\nIf you wish to ignore Losses, set to 0. This applies only when there are EnergyMeter objects. Otherwise, AutoAdd mode minimizes total system losses.",
    ),
    (
        "Executive.markcapacitors",
        "{YES/TRUE | NO/FALSE}  Default is NO. Mark Capacitor locations with a symbol. See CapMarkerCode. ",
    ),
    (
        "Executive.markercode",
        "Number code for node marker on circuit plots. Number from 0 to 47. Default is 16 (open circle). 24 is solid circle. Try other values for other symbols. See also Nodewidth",
    ),
    (
        "Executive.markfuses",
        "{YES/TRUE | NO/FALSE}  Default is NO. Mark Fuse locations with a symbol. See FuseMarkerCode and FuseMarkerSize. ",
    ),
    (
        "Executive.markpvsystems",
        "{YES/TRUE | NO/FALSE}  Default is NO. Mark PVSystem locations with a symbol. See PVMarkerCode and PVMarkerSize. ",
    ),
    (
        "Executive.markreclosers",
        "{YES/TRUE | NO/FALSE}  Default is NO. Mark Recloser locations with a symbol. See RecloserMarkerCode and RecloserMarkerSize. ",
    ),
    (
        "Executive.markregulators",
        "{YES/TRUE | NO/FALSE}  Default is NO. Mark Regulator locations with a symbol. See RegMarkerCode. ",
    ),
    (
        "Executive.markrelays",
        "{YES/TRUE | NO/FALSE}  Default is NO. Mark Relay locations with a symbol. See RelayMarkerCode and RelayMarkerSize. ",
    ),
    (
        "Executive.markstorage",
        "{YES/TRUE | NO/FALSE}  Default is NO. Mark Storage locations with a symbol. See StoreMarkerCode and StoreMarkerSize. ",
    ),
    (
        "Executive.markswitches",
        "{YES/TRUE | NO/FALSE}  Default is NO. Mark lines that are switches or are isolated with a symbol. See SwitchMarkerCode.",
    ),
    (
        "Executive.marktransformers",
        "{YES/TRUE | NO/FALSE}  Default is NO. Mark transformer locations with a symbol. See TransMarkerCode. The coordinate of one of the buses for winding 1 or 2 must be defined for the symbol to show",
    ),
    (
        "Executive.maxcontroliter",
        "Max control iterations per solution.  Default is 10.",
    ),
    (
        "Executive.maxiterations",
        "Sets the maximum allowable iterations for power flow solutions. Default is 15.",
    ),
    (
        "Executive.miniterations",
        "Minimum number of iterations required for a solution. Default is 2.",
    ),
    (
        "Executive.mode",
        "Set the solution Mode: One of\n  Snapshot,\n  Daily,\n  Yearly (follow Yearly curve),\n  DIrect,\n  DUtycycle,\n  Time, ( see LoadShapeClass option)\n  DYnamic,  ( see LoadShapeClass option)\n  Harmonic,\n  HarmonicT,  (sequential Harmonic Mode)\n  M1 (Monte Carlo 1),\n  M2 (Monte Carlo 2),\n  M3 (Monte Carlo 3),\n  Faultstudy,\n  MF (monte carlo fault study)\n  Peakday,\n  LD1 (load-duration 1)\n  LD2 (load-duration 2)\n  AutoAdd (see AddType)\n  YearlyVQ (Yearly Vector Quantization)\n  DutyVQ (Duty Vector Quantization)\n\nSide effect: setting the Mode property resets all monitors and energy meters. It also resets the time step, etc. to defaults for each mode.  After the initial reset, the user must explicitly reset the monitors and/or meters until another Set Mode= command.",
    ),
    (
        "Executive.neglectloady",
        "{YES/TRUE | NO/FALSE}  Default is NO. For Harmonic solution, neglect the Load shunt admittance branch that can siphon off some of the Load injection current. \n\nIf YES, the current injected from the LOAD at harmonic frequencies will be nearly ideal.",
    ),
    (
        "Executive.nodewidth",
        "Width of node marker. Default=1. See MarkerCode",
    ),
    (
        "Executive.normvmaxpu",
        "Maximum permissible per unit voltage for normal conditions. Default is 1.05.",
    ),
    (
        "Executive.normvminpu",
        "Minimum permissible per unit voltage for normal conditions. Default is 0.95.",
    ),
    (
        "Executive.num_subcircuits",
        "This is the number of subcircuits in which the circuit will be torn when executing the tear_circuit command, by default is the number of local CPUs - 1",
    ),
    (
        "Executive.numactors",
        "Delivers the number of Actors created by the user, 1 is the default",
    ),
    (
        "Executive.numallociterations",
        "Default is 2. Maximum number of iterations for load allocations for each time the AllocateLoads or Estimate command is given.",
    ),
    (
        "Executive.numanodes",
        "Delivers the number of Non-uniform memory access nodes (NUMA Nodes) available on the machine (read Only). This information is vital when working with processor clusters (HPC). It will help you know the number of processors in the cluster",
    ),
    (
        "Executive.number",
        "Number of solutions or time steps to perform for each Solve command. Defaults for selected modes: \n\nDaily = 24\nYearly = 8760\nDuty = 100",
    ),
    (
        "Executive.numcores",
        "Delivers the number of physical processors (Cores) available on the computer. If your computers processor has less than 64 cores, this number should be equal to the half of the available CPUs, otherwise the number should  be the same (Read Only)",
    ),
    (
        "Executive.numcpus",
        "Delivers the number of threads (CPUs) available on the machine (read Only)",
    ),
    ("Executive.object", "Synonym for Element=. (See above)"),
    (
        "Executive.opendssviewer",
        "Activates/Deactivates the extended version of the plot command for figures with the OpenDSS Viewer.",
    ),
    (
        "Executive.overloadreport",
        "{YES/TRUE | NO/FALSE} Default = FALSE. For yearly solution mode, sets overload reporting on/off. DemandInterval must be set to true for this to have effect.",
    ),
    (
        "Executive.parallel",
        "Activates/Deactivates the parallel machine in OpenDSS, if deactivated OpenDSS will behave sequentially",
    ),
    (
        "Executive.pricecurve",
        "Sets the PRICESHAPE object to use to obtain for price signal. Default is none (null string). If none, price signal either remains constant or is set by an external process using Set Price= option. Curve is defined as a PRICESHAPE  in actual values (not normalized) and should be defined to correspond to the type of analysis being performed (daily, yearly, etc.).",
    ),
    (
        "Executive.pricesignal",
        "Sets the present price signal ($/MWh) for the circuit.  Default value is 25.",
    ),
    (
        "Executive.processtime",
        "The time in microseconds to execute the solve process in the most recent time step or solution (read only)",
    ),
    (
        "Executive.pvmarkercode",
        "Numeric marker code (0..47 see Users Manual) for PVSystems and PVSystem. Default is 15.",
    ),
    (
        "Executive.pvmarkersize",
        "Size of PVsystem and PVSystem markers. Default is 1.",
    ),
    (
        "Executive.querylog",
        "{YES/TRUE | NO/FALSE} Default = FALSE. When set to TRUE/YES, clears the query log file and thereafter appends the time-stamped Result string contents to the log file after a query command, ?. ",
    ),
    (
        "Executive.random",
        "One of [Uniform | Gaussian | Lognormal | None ] for Monte Carlo Variables.",
    ),
    (
        "Executive.reclosermarkercode",
        "Numeric marker code (0..47 see Users Manual) for Recloser elements. Default is 17. (color=Lime)",
    ),
    (
        "Executive.reclosermarkersize",
        "Size of Recloser marker. Default is 5.",
    ),
    (
        "Executive.recorder",
        "{YES/TRUE | NO/FALSE} Default = FALSE. Opens DSSRecorder.dss in DSS install folder and enables recording of all commands that come through the text command interface. Closed by either setting to NO/FALSE or exiting the program. When closed by this command, the file name can be found in the Result. Does not require a circuit defined.",
    ),
    (
        "Executive.reduceoption",
        "{ Default or [null] | Shortlines [Zmag=nnn] | MergeParallel | BreakLoops | Switches | Ends | Laterals}  Strategy for reducing feeders. Default is to eliminate all dangling end buses and buses without load, caps, or taps. \n\"Shortlines [Zmag=0.02]\" merges short branches with impedance less than Zmag (default = 0.02 ohms) \n\"MergeParallel\" merges lines that have been found to be in parallel \n\"Breakloops\" disables one of the lines at the head of a loop. \n\"Ends\" eliminates dangling ends only.\n\"Switches\" merges switches with downline lines and eliminates dangling switches.\n\"Laterals [Keepload=Yes*/No]\" uses the Remove command to eliminate 1-phase laterals and optionally lump the load back to the 2- or 3-phase feeder (default behavior). \n\nMarking buses with \"Keeplist\" will prevent their elimination.",
    ),
    (
        "Executive.registryupdate",
        "{YES/TRUE | NO/FALSE}  Default is Yes. Update Windows Registry values upon exiting.  You might want to turn this off if you temporarily change fonts or DefaultBaseFrequency, for example. ",
    ),
    (
        "Executive.regmarkercode",
        "Numeric marker code (0..47 see Users Manual) for Regulators. Default is 17. (red)",
    ),
    (
        "Executive.regmarkersize",
        "Size of Regulator marker. Default is 5.",
    ),
    (
        "Executive.relaymarkercode",
        "Numeric marker code (0..47 see Users Manual) for Relay elements. Default is 17. (Color=Lime)",
    ),
    (
        "Executive.relaymarkersize",
        "Size of Relay marker. Default is 5.",
    ),
    (
        "Executive.sampleenergymeters",
        "{YES/TRUE | NO/FALSE} Overrides default value for sampling EnergyMeter objects at the end of the solution loop. Normally Time and Duty modes do not automatically sample EnergyMeters whereas Daily, Yearly, M1, M2, M3, LD1 and LD2 modes do. Use this Option to turn sampling on or off",
    ),
    (
        "Executive.seasonrating",
        "{YES/TRUE | NO/FALSE} Default = FALSE. Enables/disables the seasonal selection of the rating for determining if an element is overloaded. When enabled, the energy meter will look for the rating (NormAmps) using the SeasonSignal  to evaluate if the PDElement is overloaded",
    ),
    (
        "Executive.seasonsignal",
        "It is the name of the XY curve defining the ratings seasonal change for the PDElements in the model when performing QSTS simulations. The seasonal ratings need to be defined at the PDElement or at the general object definition such as linecodes, lineGeometry, etc.",
    ),
    (
        "Executive.sec",
        "Sets the seconds from the hour for the start time of the solution.",
    ),
    (
        "Executive.showexport",
        "{YES/TRUE | NO/FALSE} Default = FALSE. If YES/TRUE will automatically show the results of an Export Command after it is written.",
    ),
    (
        "Executive.stepsize",
        "Sets the time step size for the active circuit.  Default units are s. May also be specified in minutes or hours by appending \"m\" or \"h\" to the value. For example:\n\n   stepsize=.25h \n  stepsize=15m\n  stepsize=900s",
    ),
    (
        "Executive.steptime",
        "Process time + meter sampling time in microseconds for most recent time step - (read only)",
    ),
    (
        "Executive.storemarkercode",
        "Numeric marker code (0..47 see Users Manual) for Storage elements. Default is 9.",
    ),
    (
        "Executive.storemarkersize",
        "Size of Storage marker. Default is 1.",
    ),
    (
        "Executive.switchmarkercode",
        "Numeric marker code for lines with switches or are isolated from the circuit. Default is 4. See markswitches option.",
    ),
    (
        "Executive.terminal",
        "Set the active terminal of the active circuit element. May also be done with Select command.",
    ),
    (
        "Executive.time",
        "Specify the solution start time as an array:\ntime=(hour, secs)",
    ),
    (
        "Executive.tolerance",
        "Sets the solution tolerance.  Default is 0.0001.",
    ),
    (
        "Executive.totaltime",
        "The accumulated time in microseconds to solve the circuit since the last reset. Set this value to reset the accumulator.",
    ),
    (
        "Executive.tracecontrol",
        "{YES/TRUE | NO/FALSE}  Set to YES to trace the actions taken in the control queue.  Creates a file named TRACE_CONTROLQUEUE.csv in the default directory. The names of all circuit elements taking an action are logged.",
    ),
    (
        "Executive.transmarkercode",
        "Numeric marker code (0..47 see Users Manual) for transformers. Default is 35. See markstransformers option.",
    ),
    (
        "Executive.transmarkersize",
        "Size of transformer marker. Default is 1.",
    ),
    (
        "Executive.trapezoidal",
        "{YES/TRUE | NO/FALSE}  Default is \"No/False\". Specifies whether to use trapezoidal integration for accumulating energy meter registers. Applies to EnergyMeter and Generator objects.  Default method simply multiplies the present value of the registers times the width of the interval (Euler). Trapezoidal is more accurate when there are sharp changes in a load shape or unequal intervals. Trapezoidal is automatically used for some load-duration curve simulations where the interval size varies considerably. Keep in mind that for Trapezoidal, you have to solve one more point than the number of intervals. That is, to do a Daily simulation on a 24-hr load shape, you would set Number=25 to force a solution at the first point again to establish the last (24th) interval.\n\nNote: Set Mode= resets Trapezoidal to No/False. Set this to Yes/True AFTER setting the Mode option.",
    ),
    (
        "Executive.type",
        "Sets the active DSS class type.  Same as Class=...",
    ),
    (
        "Executive.ueregs",
        "Which EnergyMeter register(s) to use for UE in AutoAdd Mode. May be one or more registers.  if more than one, register values are summed together. Array of integer values > 0.  Defaults to 11 (for Load EEN). \n\nfor a list of EnergyMeter register numbers, do the \"Show Meters\" command after defining a circuit.",
    ),
    (
        "Executive.ueweight",
        "Weighting factor for UE/EEN in AutoAdd functions.  Defaults to 1.0.\n\nAutoadd mode minimizes\n\n(Lossweight * Losses + UEweight * UE). \n\nIf you wish to ignore UE, set to 0. This applies only when there are EnergyMeter objects. Otherwise, AutoAdd mode minimizes total system losses.",
    ),
    (
        "Executive.usemylinkbranches",
        "{YES/TRUE | NO/FALSE*} Set/get the boolean flag for indicating to the tearing algorithm the source of the link branches for tearing the model into sub-circuits. If FALSE, OpenDSS will use METIS for estimating the link branches to be used based on the number of sub-circuits given by the user through the command \"set Num_SubCircuits\".Otherwise, OpenDSS will use the list of link branches given by the user with the command \"set LinkBranches\".",
    ),
    (
        "Executive.voltagebases",
        "Define legal bus voltage bases for this circuit.  Enter an array of the legal voltage bases, in phase-to-phase voltages, for example:\n\nset voltagebases=\".208, .480, 12.47, 24.9, 34.5, 115.0, 230.0\" \n\nWhen the CalcVoltageBases command is issued, a snapshot solution is performed with no load injections and the bus base voltage is set to the nearest legal voltage base. The defaults are as shown in the example above.",
    ),
    (
        "Executive.voltexceptionreport",
        "{YES/TRUE | NO/FALSE} Default = FALSE. For yearly solution mode, sets voltage exception reporting on/off. DemandInterval must be set to true for this to have effect.",
    ),
    (
        "Executive.year",
        "Sets the Year (integer number) to be used for the solution. for certain solution types, this determines the growth multiplier.",
    ),
    (
        "Executive.zmag",
        "Sets the Zmag option (in Ohms) for ReduceOption Shortlines option. Lines have less line mode impedance are reduced.",
    ),
    (
        "Executive.zonelock",
        "{YES/TRUE | NO/FALSE}  Default is No. if No, then meter zones are recomputed each time there is a change in the circuit. If Yes, then meter zones are not recomputed unless they have not yet been computed. Meter zones are normally recomputed on Solve command following a circuit change.",
    ),
    ("ExpControl.basefreq", "Base Frequency for ratings."),
    (
        "ExpControl.deltaq_factor",
        "Convergence parameter; Defaults to 0.7. \n\nSets the maximum change (in per unit) from the prior var output level to the desired var output level during each control iteration. If numerical instability is noticed in solutions such as var sign changing from one control iteration to the next and voltages oscillating between two values with some separation, this is an indication of numerical instability (use the EventLog to diagnose). If the maximum control iterations are exceeded, and no numerical instability is seen in the EventLog of via monitors, then try increasing the value of this parameter to reduce the number of control iterations needed to achieve the control criteria, and move to the power flow solution.",
    ),
    (
        "ExpControl.derlist",
        "Alternative to PVSystemList for CIM export and import.\n\nHowever, storage is not actually implemented yet. Use fully qualified PVSystem names.",
    ),
    (
        "ExpControl.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "ExpControl.eventlog",
        "{Yes/True* | No/False} Default is No for ExpControl. Log control actions to Eventlog.",
    ),
    (
        "ExpControl.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "ExpControl.preferq",
        "{Yes/True* | No/False} Default is No for ExpControl.\n\nCurtails real power output as needed to meet the reactive power requirement. IEEE1547-2018 requires Yes, but the default is No for backward compatibility of OpenDSS models.",
    ),
    (
        "ExpControl.pvsystemlist",
        "Array list of PVSystems to be controlled.\n\nIf not specified, all PVSystems in the circuit are assumed to be controlled by this ExpControl.",
    ),
    (
        "ExpControl.qbias",
        "Equilibrium per-unit reactive power when V=Vreg; defaults to 0.\n\nEnter > 0 for lagging (capacitive) bias, < 0 for leading (inductive) bias.",
    ),
    (
        "ExpControl.qmaxlag",
        "Limit on lagging (capacitive) reactive power injection, in per-unit of base kva; defaults to 0.44.\n\nFor Category A inverters per P1547/D7, set this value to 0.25.Regardless of QmaxLag, the reactive power injection is still limited by dynamic headroom when actual real power output exceeds 0%",
    ),
    (
        "ExpControl.qmaxlead",
        "Limit on leading (inductive) reactive power injection, in per-unit of base kva; defaults to 0.44.For Category A inverters per P1547/D7, set this value to 0.25.\n\nRegardless of QmaxLead, the reactive power injection is still limited by dynamic headroom when actual real power output exceeds 0%",
    ),
    (
        "ExpControl.slope",
        "Per-unit reactive power injection / per-unit voltage deviation from Vreg; defaults to 50.\n\nUnlike InvControl, base reactive power is constant at the inverter kva rating.",
    ),
    (
        "ExpControl.tresponse",
        "Open-loop response time for changes in Q.\n\nThe value of Q reaches 90% of the target change within Tresponse, which corresponds to a low-pass filter having tau = Tresponse / 2.3026. The behavior is similar to LPFTAU in InvControl, but here the response time is input instead of the time constant. IEEE1547-2018 default is 10s for Category A and 5s for Category B, adjustable from 1s to 90s for both categories. However, the default is 0 for backward compatibility of OpenDSS models.",
    ),
    (
        "ExpControl.vreg",
        "Per-unit voltage at which reactive power is zero; defaults to 1.0.\n\nThis may dynamically self-adjust when VregTau > 0, limited by VregMin and VregMax.If input as 0, Vreg will be initialized from a snapshot solution with no inverter Q.The equilibrium point of reactive power is also affected by Qbias",
    ),
    (
        "ExpControl.vregmax",
        "Upper limit on adaptive Vreg; defaults to 1.05 per-unit",
    ),
    (
        "ExpControl.vregmin",
        "Lower limit on adaptive Vreg; defaults to 0.95 per-unit",
    ),
    (
        "ExpControl.vregtau",
        "Time constant for adaptive Vreg. Defaults to 1200 seconds.\n\nWhen the control injects or absorbs reactive power due to a voltage deviation from the Q=0 crossing of the volt-var curve, the Q=0 crossing will move toward the actual terminal voltage with this time constant. Over time, the effect is to gradually bring inverter reactive power to zero as the grid voltage changes due to non-solar effects. If zero, then Vreg stays fixed. IEEE1547-2018 requires adjustability from 300s to 5000s",
    ),
    (
        "ExportOption.allocationfactors",
        "Exports load allocation factors. File name is assigned.",
    ),
    (
        "ExportOption.branchreliability",
        "(Default file = EXP_BranchReliability.csv) Failure rate, number of interruptions and other reliability data for each PD element.",
    ),
    (
        "ExportOption.buscoords",
        "[Default file = EXP_BUSCOORDS.csv] Bus coordinates in csv form.",
    ),
    (
        "ExportOption.buslevels",
        "Exports the names and the level of each Bus inside the Circuit based on its topology information. The level value defineshow far or close is the bus from the circuits backbone (0 means that the bus is at the backbone)",
    ),
    (
        "ExportOption.busreliability",
        "(Default file = EXP_BusReliability.csv) Failure rate, number of interruptions and other reliability data at each bus.",
    ),
    (
        "ExportOption.capacity",
        "(Default file = EXP_CAPACITY.csv) Capacity report.",
    ),
    (
        "ExportOption.cdpsmasset",
        "** Deprecated ** (IEC 61968-13, CDPSM Asset profile)",
    ),
    (
        "ExportOption.cdpsmelec",
        "** Deprecated ** (IEC 61968-13, CDPSM Electrical Properties profile)",
    ),
    (
        "ExportOption.cdpsmgeo",
        "** Deprecated ** (IEC 61968-13, CDPSM Geographical profile)",
    ),
    (
        "ExportOption.cdpsmstatevar",
        "** Deprecated ** (IEC 61968-13, CDPSM State Variables profile)",
    ),
    (
        "ExportOption.cdpsmtopo",
        "** Deprecated ** (IEC 61968-13, CDPSM Topology profile)",
    ),
    (
        "ExportOption.cim100",
        "(Default file = CIM100x.XML) (IEC 61968-13, combined CIM100 for unbalanced load flow profile)\n [File=filename fid=_uuidstring Substation=subname sid=_uuidstring\n SubGeographicRegion=subgeoname sgrid=_uuidstring GeographicRegion=geoname rgnid=_uuidstring]",
    ),
    (
        "ExportOption.cim100fragments",
        "(Default file ROOT = CIM100) (IEC 61968-13, CIM100 for unbalanced load flow profile)\n produces 6 separate files ROOT_FUN.XML for Functional profile,\n ROOT_EP.XML for Electrical Properties profile,\n ROOT_TOPO.XML for Topology profile,\n ROOT_CAT.XML for Asset Catalog profile,\n ROOT_GEO.XML for Geographical profile and\n ROOT_SSH.XML for Steady State Hypothesis profile\n [File=fileroot fid=_uuidstring Substation=subname sid=_uuidstring\n SubGeographicRegion=subgeoname sgrid=_uuidstring GeographicRegion=geoname rgnid=_uuidstring]",
    ),
    (
        "ExportOption.contours",
        "Exports the Contours matrix (C) calculated after initilizing A-Diakoptics. The output format is compressed coordianted and the values are integers.  If A-Diakoptics is not initialized this command does nothing",
    ),
    (
        "ExportOption.counts",
        "[Default file = EXP_Counts.csv] (instance counts for each class)",
    ),
    (
        "ExportOption.currents",
        "(Default file = EXP_CURRENTS.csv) Currents in each conductor of each element.",
    ),
    (
        "ExportOption.elemcurrents",
        "(Default file = EXP_ElemCurrents.csv)  Exports the current into all conductors of all circuit elements",
    ),
    (
        "ExportOption.elempowers",
        "(Default file = EXP_elemPowers.csv)  Exports the powers into all conductors of all circuit elements",
    ),
    (
        "ExportOption.elemvoltages",
        "(Default file = EXP_ElemVoltages.csv)  Exports the voltages to ground at all conductors of all circuit elements",
    ),
    (
        "ExportOption.errorlog",
        "(Default file = EXP_ErrorLog.txt) All entries in the present Error log.",
    ),
    (
        "ExportOption.estimation",
        "(Default file = EXP_ESTIMATION.csv) Results of last estimation.",
    ),
    (
        "ExportOption.eventlog",
        "(Default file = EXP_EventLog.csv) All entries in the present event log.",
    ),
    (
        "ExportOption.faultstudy",
        "(Default file = EXP_FAULTS.csv) results of a fault study.",
    ),
    (
        "ExportOption.generators",
        "(Default file = EXP_GENMETERS.csv) Present values of generator meters. Adding the switch \"/multiple\" or \"/m\" will  cause a separate file to be written for each generator.",
    ),
    (
        "ExportOption.gicmvars",
        "(Default file = EXP_GIC_Mvar.csv) Mvar for each GICtransformer object by bus for export to power flow programs ",
    ),
    (
        "ExportOption.incmatrix",
        "Exports the Branch-to-Node Incidence matrix calculated for the circuit in compressed coordianted format (Row,Col,Value)",
    ),
    (
        "ExportOption.incmatrixcols",
        "Exports the names of the Cols (Buses) used for calculating the Branch-to-Node Incidence matrix for the active circuit",
    ),
    (
        "ExportOption.incmatrixrows",
        "Exports the names of the rows (PDElements) used for calculating the Branch-to-Node Incidence matrix for the active circuit",
    ),
    (
        "ExportOption.laplacian",
        "Exports the Laplacian matrix calculated using the branch-to-node Incidence matrix in compressed coordinated format (Row,Col,Value)",
    ),
    (
        "ExportOption.loads",
        "(Default file = EXP_LOADS.csv) Report on loads from most recent solution.",
    ),
    (
        "ExportOption.losses",
        "[Default file = EXP_LOSSES.csv] Losses for each element.",
    ),
    (
        "ExportOption.meters",
        "(Default file = EXP_METERS.csv) Energy meter exports. Adding the switch \"/multiple\" or \"/m\" will  cause a separate file to be written for each meter.",
    ),
    (
        "ExportOption.monitors",
        "(file name is assigned by Monitor export) Monitor values. The argument is the name of the monitor (e.g. Export Monitor XYZ, XYZ is the name of the monitor).\nThe argument can be ALL, which means that all the monitors will be exported",
    ),
    (
        "ExportOption.nodenames",
        "(Default file = EXP_NodeNames.csv) Exports Single-column file of all node names in the active circuit. Useful for making scripts.",
    ),
    (
        "ExportOption.nodeorder",
        "(Default file = EXP_NodeOrder.csv)  Exports the present node order for all conductors of all circuit elements",
    ),
    (
        "ExportOption.overloads",
        "(Default file = EXP_OVERLOADS.csv) Overloaded elements report.",
    ),
    (
        "ExportOption.p_byphase",
        "(Default file = EXP_P_BYPHASE.csv) [MVA] [Filename] Power by phase. Default is kVA.",
    ),
    (
        "ExportOption.powers",
        "(Default file = EXP_POWERS.csv) [MVA] [Filename] Powers (kVA by default) into each terminal of each element.",
    ),
    (
        "ExportOption.profile",
        "[Default file = EXP_Profile.csv] Coordinates, color of each line section in Profile plot. Same options as Plot Profile Phases property.\n\nExample:  Export Profile Phases=All [optional file name]",
    ),
    (
        "ExportOption.pvsystem_meters",
        "(Default file = EXP_PVMETERS.csv) Present values of PVSystem meters. Adding the switch \"/multiple\" or \"/m\" will  cause a separate file to be written for each PVSystem.",
    ),
    (
        "ExportOption.result",
        "(Default file = EXP_Result.csv)  Exports the result of the most recent command.",
    ),
    (
        "ExportOption.sections",
        "(Default file = EXP_SECTIONS.csv) Data for each section between overcurrent protection devices. \n\nExamples: \n  Export Sections [optional filename]\nExport Sections meter=M1 [optional filename]",
    ),
    (
        "ExportOption.seqcurrents",
        "(Default file = EXP_SEQCURRENTS.csv) Sequence currents in each terminal of 3-phase elements.",
    ),
    (
        "ExportOption.seqpowers",
        "(Default file = EXP_SEQPOWERS.csv) Sequence powers into each terminal of 3-phase elements.",
    ),
    (
        "ExportOption.seqvoltages",
        "(Default file = EXP_SEQVOLTAGES.csv) Sequence voltages.",
    ),
    (
        "ExportOption.seqz",
        "(Default file = EXP_SEQZ.csv) Equivalent sequence Z1, Z0 to each bus.",
    ),
    (
        "ExportOption.storage_meters",
        "(Default file = EXP_STORAGEMETERS.csv) Present values of Storage meters. Adding the switch \"/multiple\" or \"/m\" will  cause a separate file to be written for each Storage device.",
    ),
    (
        "ExportOption.summary",
        "[Default file = EXP_Summary.csv] Solution summary.",
    ),
    (
        "ExportOption.taps",
        "(Default file = EXP_Taps.csv)  Exports the regulator tap report similar to Show Taps.",
    ),
    (
        "ExportOption.unserved",
        "(Default file = EXP_UNSERVED.csv) [UEonly] [Filename] Report on elements that are unserved due to violation of ratings.",
    ),
    (
        "ExportOption.uuids",
        "[Default file = EXP_UUIDS.csv] Uuids for each element. This frees the UUID list after export.",
    ),
    (
        "ExportOption.voltages",
        "(Default file = EXP_VOLTAGES.csv) Voltages to ground by bus/node.",
    ),
    (
        "ExportOption.voltageselements",
        "(Default file = EXP_VOLTAGES_ELEM.csv) Voltages to ground by circuit element.",
    ),
    (
        "ExportOption.y",
        "(Default file = EXP_Y.csv) [triplets] [Filename] System Y matrix, defaults to non-sparse format.",
    ),
    (
        "ExportOption.y4",
        "Exports the inverse of Z4 (ZCC) calculated after initilizing A-Diakoptics. The output format is compressed coordianted and the values are complex conjugates.  If A-Diakoptics is not initialized this command does nothing",
    ),
    (
        "ExportOption.ycurrents",
        "(Default file = EXP_YCurrents.csv)  Exports the present solution complex Current array in same order as YNodeList. This is generally the injection current array",
    ),
    (
        "ExportOption.ynodelist",
        "(Default file = EXP_YNodeList.csv)  Exports a list of nodes in the same order as the System Y matrix.",
    ),
    (
        "ExportOption.yprims",
        "(Default file = EXP_YPRIMS.csv) All primitive Y matrices.",
    ),
    (
        "ExportOption.yvoltages",
        "(Default file = EXP_YVoltages.csv)  Exports the present solution complex Voltage array in same order as YNodeList.",
    ),
    (
        "ExportOption.zcc",
        "Exports the connectivity matrix (ZCC) calculated after initilizing A-Diakoptics. The output format is compressed coordianted and the values are complex conjugates.  If A-Diakoptics is not initialized this command does nothing",
    ),
    (
        "ExportOption.zll",
        "Exports the Link branches matrix (ZLL) calculated after initilizing A-Diakoptics. The output format is compressed coordianted and the values are complex conjugates. If A-Diakoptics is not initialized this command does nothing",
    ),
    (
        "FMonitor.action",
        "{Clear | Save | Take | Process}\n(C)lears or (S)aves current buffer.\n(T)ake action takes a sample.\n(P)rocesses the data taken so far (e.g. Pst for mode 4).\n\nNote that monitors are automatically reset (cleared) when the Set Mode= command is issued. Otherwise, the user must explicitly reset all monitors (reset monitors command) or individual monitors with the Clear action.",
    ),
    (
        "FMonitor.attack_defense",
        "Define attack and defense:  attack_defense = {atk , dfs , atk_time , atk_node_num  , d_atk0  , beta_dfs, D_beta, D_p }.\nattack_defense has to be defined after ''nodes'.\nExample: attack_defense = { true , false , 0.5 , 1 , 0.1 , 5, 1 , 1}.\nExample: (1) under attack); (2) defense is off; (3) attack starts at 0.5s; (4) attack is on node 1;\n(5) initial value of attack: d_0 = 0.1; (6) beta = 5;\n(7) D_bata is used as a multiplier on \\phi;\n(8) D_p is used as the attack on gradient control: D_p = 1, which is normal; D_p=-1, gradient control work on the opposite.",
    ),
    (
        "FMonitor.b_curt_ctrl",
        "b_Curt_Ctrl:set P curtailment on/off;\nb_Curt_Ctrl=True: P curtailment will be implemented according to the system voltage (default);\nb_Curt_Ctrl=False: P curtailment will not be implemented.",
    ),
    ("FMonitor.basefreq", "Base Frequency for ratings."),
    ("FMonitor.cluster_num", "Cluster_num"),
    (
        "FMonitor.comm_hide",
        "Comm_hide={...}. It is defined like CommVector.",
    ),
    (
        "FMonitor.comm_node_hide",
        "Comm_node_hide={...}. It is defined like CommVector.",
    ),
    (
        "FMonitor.commdelayvector",
        "CommDelayVector of this FMonitor. \nThe first entry of this vector is the number of the node.\nExample:(CommVector={2,t1,0,t2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0})\nThe example show node #2 can communicate to node #1 and #3 with time delay t1 and t2 separately",
    ),
    (
        "FMonitor.commvector",
        "CommVector of this FMonitor. \nThe first entry of this vector is the number of \nExample:(CommVector={2,1,1,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0})\nThe example show node #2 can communicate to node #1,#2,#3",
    ),
    (
        "FMonitor.egen",
        " EGen = {kVA_fm, M_fm, D_fm, Tau_fm, Ki_fm,init_time}\nwhere equations are:\n(1):delta''=omega\n(1):M_fm * omega''=puPm - puPe - D_fm*omega\n(1):Tau_fm*Pm ''=Ki_fm * omega \npuPm = Pm / kVA_fm, puPe = Pe/ kVAM_fm;\neverything is zero within init_time(default value is 0.5s);\nk_dltP is the coordinator for PV control input: u_i = k_dltP * pu_DltP + omg_fm.",
    ),
    (
        "FMonitor.element",
        "Name (Full Object name) of element to which the monitor is connected.",
    ),
    (
        "FMonitor.elemtableline",
        "ElemTableLine of the each node within this cluster. \nThe first entry of this vector is the number of node within cluster \nThe second entry of this vector is element name \nThe third entry of this vector is terminal number \nThe fourth entry of this vector is voltage sensor \nExample:(ElemTable={2,Line.1,1,1})\nThe example show node #2 Element",
    ),
    (
        "FMonitor.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "FMonitor.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "FMonitor.maxlocalmem",
        "MaxLocalMem: the max number of local memory size. No larger than 99",
    ),
    (
        "FMonitor.mode",
        "Bitmask integer designating the values the monitor is to capture: \n0 = Voltages and currents\n1 = Powers\n2 = Tap Position (Transformers only)\n3 = State Variables (PCElements only)\n4 = Flicker level and severity index (Pst) for voltages. No adders apply.\n    Flicker level at simulation time step, Pst at 10-minute time step.\n5 = Solution variables (Iterations, etc).\n\nNormally, these would be actual phasor quantities from solution.\n6 = Capacitor Switching (Capacitors only)\nCombine with adders below to achieve other results for terminal quantities:\n+16 = Sequence quantities\n+32 = Magnitude only\n+64 = Positive sequence only or avg of all phases\n\nMix adder to obtain desired results. For example:\nMode=112 will save positive sequence voltage and current magnitudes only\nMode=48 will save all sequence voltages and currents, but magnitude only.",
    ),
    (
        "FMonitor.node_num",
        "Node_num\nAssign a node number within a cluster",
    ),
    (
        "FMonitor.nodes",
        "Nodes connected to this FMonitor. Example:(Nodes=33)",
    ),
    (
        "FMonitor.p_mode",
        "0 = real Power controlled by each p_ref on each DG\n1 = real Power on MeteredElem controlled by DGs according to P_trans_ref\n2 = Not defined\n3 = Not defined",
    ),
    ("FMonitor.p_sensor", "P_Sensor\nEnable power sensor"),
    (
        "FMonitor.p_trans_ref",
        "P_trans_ref: P ref value for metered element(unit kW)",
    ),
    (
        "FMonitor.ppolar",
        "{Yes/True | No/False} Default = YES. Report power in Apparent power, S, in polar form (Mag/Angle).(default)  Otherwise, is P and Q",
    ),
    (
        "FMonitor.residual",
        "{Yes/True | No/False} Default = No.  Include Residual channel (sum of all phases) for voltage and current. Does not apply to sequence quantity modes or power modes.",
    ),
    (
        "FMonitor.t_intvl_smpl",
        "T_intvl_smpl: \nThe information of each agent will be sampled at each T_comm time. Unit is second.\nT_intvl_smpl is also the minimal communication time between neighbor nodes.\nIf T_intvl_smpl=0.0, no delay for the communication is enabled in the simulation.",
    ),
    (
        "FMonitor.terminal",
        "Number of the terminal of the circuit element to which the monitor is connected. 1 or 2, typically. For monitoring states, attach monitor to terminal 1.",
    ),
    (
        "FMonitor.total_clusters",
        "Total_Clusters.\nDefine the total number of groups in a circuit\nJust use for the first defined FMonitor",
    ),
    (
        "FMonitor.up_dly",
        "up_dly: delay time to upper level. For example: \"up_dly := 0.05\"\nIt can be used to simulate the time delay between clusters",
    ),
    ("FMonitor.v_sensor", "V_Sensor\nEnable voltage sensor"),
    (
        "FMonitor.vipolar",
        "{Yes/True | No/False} Default = YES. Report voltage and current in polar form (Mag/Angle). (default)  Otherwise, it will be real and imaginary.",
    ),
    (
        "FMonitor.volt_limits_pu",
        "Volt_limits_pu: example \"Volt_limits_pu={a0,a1, a2}\"\na0: the phase number, 0 means pos. seq; a1: upper voltage limit of this cluster, usually 1.05;\na2: upper voltage limit of this cluster, usually 0.95",
    ),
    (
        "Fault.%stddev",
        "Percent standard deviation in resistance to assume for Monte Carlo fault (MF) solution mode for GAUSSIAN distribution. Default is 0 (no variation from mean).",
    ),
    ("Fault.basefreq", "Base Frequency for ratings."),
    (
        "Fault.bus1",
        "Name of first bus. Examples:\n\nbus1=busname\nbus1=busname.1.2.3\n\nBus2 automatically defaults to busname.0,0,0 unless it was previously defined. ",
    ),
    (
        "Fault.bus2",
        "Name of 2nd bus of the 2-terminal Fault object. Defaults to all phases connected to first bus, node 0, if not specified. (Shunt Wye Connection to ground reference)\n\nThat is, the Fault defaults to a ground fault unless otherwise specified.",
    ),
    ("Fault.emergamps", "Maximum or emerg current."),
    (
        "Fault.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    ("Fault.faultrate", "Failure rate per year."),
    (
        "Fault.gmatrix",
        "Use this to specify a nodal conductance (G) matrix to represent some arbitrary resistance network. Specify in lower triangle form as usual for DSS matrices.",
    ),
    (
        "Fault.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Fault.minamps",
        "Minimum amps that can sustain a temporary fault. Default is 5.",
    ),
    ("Fault.normamps", "Normal rated current."),
    (
        "Fault.ontime",
        "Time (sec) at which the fault is established for time varying simulations. Default is 0.0 (on at the beginning of the simulation)",
    ),
    (
        "Fault.pctperm",
        "Percent of failures that become permanent.",
    ),
    ("Fault.phases", "Number of Phases. Default is 1."),
    (
        "Fault.r",
        "Resistance, each phase, ohms. Default is 0.0001. Assumed to be Mean value if gaussian random mode.Max value if uniform mode.  A Fault is actually a series resistance that defaults to a wye connection to ground on the second terminal.  You may reconnect the 2nd terminal to achieve whatever connection.  Use the Gmatrix property to specify an arbitrary conductance matrix.",
    ),
    ("Fault.repair", "Hours to repair."),
    (
        "Fault.temporary",
        "{Yes | No} Default is No.  Designate whether the fault is temporary.  For Time-varying simulations, the fault will be removed if the current through the fault drops below the MINAMPS criteria.",
    ),
    ("Fuse.action", "DEPRECATED. See \"State\" property."),
    ("Fuse.basefreq", "Base Frequency for ratings."),
    (
        "Fuse.delay",
        "Fixed delay time (sec) added to Fuse blowing time determined from the TCC curve. Default is 0.0. Used to represent fuse clearing time or any other delay.",
    ),
    (
        "Fuse.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "Fuse.fusecurve",
        "Name of the TCC Curve object that determines the fuse blowing.  Must have been previously defined as a TCC_Curve object. Default is \"Tlink\". Multiplying the current values in the curve by the \"RatedCurrent\" value gives the actual current.",
    ),
    (
        "Fuse.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Fuse.monitoredobj",
        "Full object name of the circuit element, typically a line, transformer, load, or generator, to which the Fuse is connected. This is the \"monitored\" element. There is no default; must be specified.",
    ),
    (
        "Fuse.monitoredterm",
        "Number of the terminal of the circuit element to which the Fuse is connected. 1 or 2, typically.  Default is 1.",
    ),
    (
        "Fuse.normal",
        "ARRAY of strings {Open | Closed} representing the Normal state of the fuse in each phase of the controlled element. The fuse reverts to this state for reset, change of mode, etc. Defaults to \"State\" if not specifically declared.",
    ),
    (
        "Fuse.ratedcurrent",
        "Multiplier or actual phase amps for the phase TCC curve.  Defaults to 1.0.",
    ),
    (
        "Fuse.state",
        "ARRAY of strings {Open | Closed} representing the Actual state of the fuse in each phase of the controlled element. Upon setting, immediately forces state of fuse(s). Simulates manual control on Fuse. Defaults to Closed for all phases.",
    ),
    (
        "Fuse.switchedobj",
        "Name of circuit element switch that the Fuse controls. Specify the full object name.Defaults to the same as the Monitored element. This is the \"controlled\" element.",
    ),
    (
        "Fuse.switchedterm",
        "Number of the terminal of the controlled element in which the switch is controlled by the Fuse. 1 or 2, typically.  Default is 1.  Assumes all phases of the element have a fuse of this type.",
    ),
    (
        "GICLine.angle",
        "Phase angle in degrees of first phase. Default=0.0.  See Voltage property",
    ),
    (
        "GICLine.basefreq",
        "Inherited Property for all PCElements. Base frequency for specification of reactance value.",
    ),
    (
        "GICLine.bus1",
        "Name of bus to which the main terminal (1) is connected.\nbus1=busname\nbus1=busname.1.2.3",
    ),
    (
        "GICLine.bus2",
        "Name of bus to which 2nd terminal is connected.\nbus2=busname\nbus2=busname.1.2.3\n\nNo Default; must be specified.",
    ),
    (
        "GICLine.c",
        "Value of line blocking capacitance in microfarads. Default = 0.0, implying that there is no line blocking capacitor.",
    ),
    (
        "GICLine.ee",
        "Eastward Electric field (V/km).  If specified, Voltage and Angle are computed from EN, EE, lat and lon values.",
    ),
    (
        "GICLine.en",
        "Northward Electric field (V/km). If specified, Voltage and Angle are computed from EN, EE, lat and lon values.",
    ),
    (
        "GICLine.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "GICLine.frequency",
        "Source frequency.  Defaults to 0.1 Hz.",
    ),
    ("GICLine.lat1", "Latitude of Bus1 (degrees)"),
    ("GICLine.lat2", "Latitude of Bus2 (degrees)"),
    (
        "GICLine.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    ("GICLine.lon1", "Longitude of Bus1 (degrees)"),
    ("GICLine.lon2", "Longitude of Bus2 (degrees)"),
    ("GICLine.phases", "Number of phases.  Defaults to 3."),
    (
        "GICLine.r",
        "Resistance of line, ohms of impedance in series with GIC voltage source. ",
    ),
    (
        "GICLine.spectrum",
        "Inherited Property for all PCElements. Name of harmonic spectrum for this source.  Default is \"defaultvsource\", which is defined when the DSS starts.",
    ),
    (
        "GICLine.volts",
        "Voltage magnitude, in volts, of the GIC voltage induced across this line. When specified, voltage source is assumed defined by Voltage and Angle properties. \n\nSpecify this value\n\nOR\n\nEN, EE, lat1, lon1, lat2, lon2. \n\nNot both!!  Last one entered will take precedence. Assumed identical in each phase of the Line object.",
    ),
    (
        "GICLine.x",
        "Reactance at base frequency, ohms. Default = 0.0. This value is generally not important for GIC studies but may be used if desired.",
    ),
    (
        "GICTransformer.%r1",
        "Optional. Percent Resistance, each phase, for H winding (1), (Series winding, if Auto). Default is 0.2. \n\nAlternative way to enter R1 value. It is the actual resistances in ohmns that matter. MVA and kV should be specified.",
    ),
    (
        "GICTransformer.%r2",
        "Optional. Percent Resistance, each phase, for X winding (2), (Common winding, if Auto). Default is 0.2. \n\nAlternative way to enter R2 value. It is the actual resistances in ohms that matter. MVA and kV should be specified.",
    ),
    ("GICTransformer.basefreq", "Base Frequency for ratings."),
    (
        "GICTransformer.bush",
        "Name of High-side(H) bus. Examples:\nBusH=busname\nBusH=busname.1.2.3",
    ),
    (
        "GICTransformer.busnh",
        "Name of Neutral bus for H, or first, winding. Defaults to all phases connected to H-side bus, node 0, if not specified and transformer type is either GSU or YY. (Shunt Wye Connection to ground reference)For Auto, this is automatically set to the X bus.",
    ),
    (
        "GICTransformer.busnx",
        "Name of Neutral bus for X, or Second, winding. Defaults to all phases connected to X-side bus, node 0, if not specified. (Shunt Wye Connection to ground reference)",
    ),
    (
        "GICTransformer.busx",
        "Name of Low-side(X) bus, if type=Auto or YY. ",
    ),
    ("GICTransformer.emergamps", "Maximum or emerg current."),
    (
        "GICTransformer.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    ("GICTransformer.faultrate", "Failure rate per year."),
    (
        "GICTransformer.k",
        "Mvar K factor. Default way to convert GIC Amps in H winding (winding 1) to Mvar. Default is 2.2. Commonly-used simple multiplier for estimating Mvar losses for power flow analysis. \n\nMvar = K * kvLL * GIC per phase / 1000 \n\nMutually exclusive with using the VarCurve property and pu curves.If you specify this (default), VarCurve is ignored.",
    ),
    (
        "GICTransformer.kvll1",
        "Optional. kV LL rating for H winding (winding 1). Default is 500. Required if you are going to export vars for power flow analysis or enter winding resistances in percent.",
    ),
    (
        "GICTransformer.kvll2",
        "Optional. kV LL rating for X winding (winding 2). Default is 138. Required if you are going to export vars for power flow analysis or enter winding resistances in percent..",
    ),
    (
        "GICTransformer.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "GICTransformer.mva",
        "Optional. MVA Rating assumed Transformer. Default is 100. Used for computing vars due to GIC and winding resistances if kV and MVA ratings are specified.",
    ),
    ("GICTransformer.normamps", "Normal rated current."),
    (
        "GICTransformer.pctperm",
        "Percent of failures that become permanent.",
    ),
    ("GICTransformer.phases", "Number of Phases. Default is 3."),
    (
        "GICTransformer.r1",
        "Resistance, each phase, ohms for H winding, (Series winding, if Auto). Default is 0.0001. If ",
    ),
    (
        "GICTransformer.r2",
        "Resistance, each phase, ohms for X winding, (Common winding, if Auto). Default is 0.0001. ",
    ),
    ("GICTransformer.repair", "Hours to repair."),
    (
        "GICTransformer.type",
        "Type of transformer: {GSU* | Auto | YY}. Default is GSU.",
    ),
    (
        "GICTransformer.varcurve",
        "Optional. XYCurve object name. Curve is expected as TOTAL pu vars vs pu GIC amps/phase. Vars are in pu of the MVA property. No Default value. Required only if you are going to export vars for power flow analysis. See K property.",
    ),
    (
        "GICsource.angle",
        "Phase angle in degrees of first phase. Default=0.0.  See Voltage property",
    ),
    ("GICsource.basefreq", "Not used."),
    (
        "GICsource.ee",
        "Eastward Electric field (V/km).  If specified, Voltage and Angle are computed from EN, EE, lat and lon values.",
    ),
    (
        "GICsource.en",
        "Northward Electric field (V/km). If specified, Voltage and Angle are computed from EN, EE, lat and lon values.",
    ),
    (
        "GICsource.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "GICsource.frequency",
        "Source frequency.  Defaults to  0.1 Hz. So GICSource=0 at power frequency.",
    ),
    ("GICsource.lat1", "Latitude of Bus1 of the line(degrees)"),
    ("GICsource.lat2", "Latitude of Bus2 of the line (degrees)"),
    (
        "GICsource.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    ("GICsource.lon1", "Longitude of Bus1 of the line (degrees)"),
    ("GICsource.lon2", "Longitude of Bus2 of the line (degrees)"),
    (
        "GICsource.phases",
        "Number of phases.  Defaults to 3. All three phases are assumed in phase (zero sequence)",
    ),
    ("GICsource.spectrum", "Not used."),
    (
        "GICsource.volts",
        "Voltage magnitude, in volts, of the GIC voltage induced across the associated line. When specified, induced voltage is assumed defined by Voltage and Angle properties. \n\nSpecify this value\n\nOR\n\nEN, EE, lat1, lon1, lat2, lon2. \n\nNot both!!  Last one entered will take precedence. Assumed identical in each phase of the Line object.",
    ),
    ("GenDispatcher.basefreq", "Base Frequency for ratings."),
    (
        "GenDispatcher.element",
        "Full object name of the circuit element, typically a line or transformer, which the control is monitoring. There is no default; must be specified.",
    ),
    (
        "GenDispatcher.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "GenDispatcher.genlist",
        "Array list of generators to be dispatched.  If not specified, all generators in the circuit are assumed dispatchable.",
    ),
    (
        "GenDispatcher.kvarlimit",
        "Max kvar to be delivered through the element.  Uses same dead band as kW.",
    ),
    (
        "GenDispatcher.kwband",
        "Bandwidth (kW) of the dead band around the target limit.No dispatch changes are attempted if the power in the monitored terminal stays within this band.",
    ),
    (
        "GenDispatcher.kwlimit",
        "kW Limit for the monitored element. The generators are dispatched to hold the power in band.",
    ),
    (
        "GenDispatcher.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "GenDispatcher.terminal",
        "Number of the terminal of the circuit element to which the GenDispatcher control is connected. 1 or 2, typically.  Default is 1. Make sure you have the direction on the power matching the sign of kWLimit.",
    ),
    (
        "GenDispatcher.weights",
        "Array of proportional weights corresponding to each generator in the GenList. The needed kW to get back to center band is dispatched to each generator according to these weights. Default is to set all weights to 1.0.",
    ),
    (
        "Generator.%fuel",
        "It is a number between 0 and 100 representing the current amount of fuel available in percentage of FuelkWh. It only applies if UseFuel = Yes/True",
    ),
    (
        "Generator.%reserve",
        "It is a number between 0 and 100 representing the reserve level in percentage of FuelkWh. It only applies if UseFuel = Yes/True",
    ),
    (
        "Generator.balanced",
        "{Yes | No*} Default is No.  For Model=7, force balanced current only for 3-phase generators. Force zero- and negative-sequence to zero.",
    ),
    ("Generator.basefreq", "Base Frequency for ratings."),
    (
        "Generator.bus1",
        "Bus to which the Generator is connected.  May include specific node specification.",
    ),
    (
        "Generator.class",
        "An arbitrary integer number representing the class of Generator so that Generator values may be segregated by class.",
    ),
    ("Generator.conn", "={wye|LN|delta|LL}.  Default is wye."),
    (
        "Generator.d",
        "Damping constant.  Usual range is 0 to 4. Default is 1.0.  Adjust to get damping",
    ),
    (
        "Generator.daily",
        "Dispatch shape to use for daily simulations.  Must be previously defined as a Loadshape object of 24 hrs, typically.  If generator is assumed to be ON continuously, specify Status=FIXED, or designate a Loadshape object that is 1.0 per unit for all hours. Set to NONE to reset to no loadshape. ",
    ),
    (
        "Generator.debugtrace",
        "{Yes | No }  Default is no.  Turn this on to capture the progress of the generator model for each iteration.  Creates a separate file for each generator named \"GEN_name.csv\".",
    ),
    (
        "Generator.dispmode",
        "{Default* | Loadlevel | Price } Default = Default. Dispatch mode. In default mode, gen is either always on or follows dispatch curve as specified. Otherwise, the gen comes on when either the global default load level (Loadshape \"default\") or the price level exceeds the dispatch value.",
    ),
    (
        "Generator.dispvalue",
        "Dispatch value. \nIf = 0.0 (default) then Generator follow dispatch curves, if any. \nIf > 0  then Generator is ON only when either the price signal (in Price dispatch mode) exceeds this value or the active circuit load multiplier * \"default\" loadshape value * the default yearly growth factor exceeds this value.  Then the generator follows dispatch curves (duty, daily, or yearly), if any (see also Status).",
    ),
    (
        "Generator.duty",
        "Load shape to use for duty cycle dispatch simulations such as for wind generation. Must be previously defined as a Loadshape object. Typically would have time intervals less than 1 hr -- perhaps, in seconds. Set Status=Fixed to ignore Loadshape designation. Set to NONE to reset to no loadshape. Designate the number of points to solve using the Set Number=xxxx command. If there are fewer points in the actual shape, the shape is assumed to repeat.",
    ),
    (
        "Generator.dutystart",
        "Starting time offset [hours] into the duty cycle shape for this generator, defaults to 0",
    ),
    (
        "Generator.dynamiceq",
        "The name of the dynamic equation (DynamicExp) that will be used for defining the dynamic behavior of the generator. if not defined, the generator dynamics will follow the built-in dynamic equation.",
    ),
    (
        "Generator.dynout",
        "The name of the variables within the Dynamic equation that will be used to govern the generator dynamics.This generator model requires 2 outputs from the dynamic equation: \n\n1. Shaft speed (velocity) relative to synchronous speed.\n2. Shaft, or power, angle (relative to synchronous reference frame).\n\nThe output variables need to be defined in tha strict order.",
    ),
    (
        "Generator.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "Generator.forceon",
        "{Yes | No}  Forces generator ON despite requirements of other dispatch modes. Stays ON until this property is set to NO, or an internal algorithm cancels the forced ON state.",
    ),
    (
        "Generator.fuelkwh",
        "{*0}Is the nominal level of fuel for the generator (kWh). It only applies if UseFuel = Yes/True",
    ),
    (
        "Generator.h",
        "Per unit mass constant of the machine.  MW-sec/MVA.  Default is 1.0.",
    ),
    (
        "Generator.kv",
        "Nominal rated (1.0 per unit) voltage, kV, for Generator. For 2- and 3-phase Generators, specify phase-phase kV. Otherwise, for phases=1 or phases>3, specify actual kV across each branch of the Generator. If wye (star), specify phase-neutral kV. If delta or phase-phase connected, specify phase-phase kV.",
    ),
    (
        "Generator.kva",
        "kVA rating of electrical machine. Defaults to 1.2* kW if not specified. Applied to machine or inverter definition for Dynamics mode solutions. ",
    ),
    (
        "Generator.kvar",
        "Specify the base kvar.  Alternative to specifying the power factor.  Side effect:  the power factor value is altered to agree based on present value of kW.",
    ),
    (
        "Generator.kw",
        "Total base kW for the Generator.  A positive value denotes power coming OUT of the element, \nwhich is the opposite of a load. This value is modified depending on the dispatch mode. Unaffected by the global load multiplier and growth curves. If you want there to be more generation, you must add more generators or change this value.",
    ),
    (
        "Generator.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Generator.maxkvar",
        "Maximum kvar limit for Model = 3.  Defaults to twice the specified load kvar.  Always reset this if you change PF or kvar properties.",
    ),
    (
        "Generator.minkvar",
        "Minimum kvar limit for Model = 3. Enter a negative number if generator can absorb vars. Defaults to negative of Maxkvar.  Always reset this if you change PF or kvar properties.",
    ),
    (
        "Generator.model",
        "Integer code for the model to use for generation variation with voltage. Valid values are:\n\n1:Generator injects a constant kW at specified power factor.\n2:Generator is modeled as a constant admittance.\n3:Const kW, constant kV.  Somewhat like a conventional transmission power flow P-V generator.\n4:Const kW, Fixed Q (Q never varies)\n5:Const kW, Fixed Q(as a constant reactance)\n6:Compute load injection from User-written Model.(see usage of Xd, Xdp)\n7:Constant kW, kvar, but current-limited below Vminpu. Approximates a simple inverter. See also Balanced.",
    ),
    (
        "Generator.mva",
        "MVA rating of electrical machine.  Alternative to using kVA=.",
    ),
    (
        "Generator.pf",
        "Generator power factor. Default is 0.80. Enter negative for leading powerfactor (when kW and kvar have opposite signs.)\nA positive power factor for a generator signifies that the generator produces vars \nas is typical for a synchronous generator.  Induction machines would be \nspecified with a negative power factor.",
    ),
    (
        "Generator.phases",
        "Number of Phases, this Generator.  Power is evenly divided among phases.",
    ),
    (
        "Generator.pvfactor",
        "Deceleration factor for P-V generator model (Model=3).  Default is 0.1. If the circuit converges easily, you may want to use a higher number such as 1.0. Use a lower number if solution diverges. Use Debugtrace=yes to create a file that will trace the convergence of a generator model.",
    ),
    (
        "Generator.refuel",
        "It is a boolean value (Yes/True, No/False) that can be used to manually refuel the generator when needed. It only applies if UseFuel = Yes/True",
    ),
    (
        "Generator.rneut",
        "Removed due to causing confusion - Add neutral impedance externally.",
    ),
    (
        "Generator.shaftdata",
        "String (in quotes or parentheses) that gets passed to user-written shaft dynamic model for defining the data for that model.",
    ),
    (
        "Generator.shaftmodel",
        "Name of user-written DLL containing a Shaft model, which models the prime mover and determines the power on the shaft for Dynamics studies. Models additional mass elements other than the single-mass model in the DSS default model. Set to \"none\" to negate previous setting.",
    ),
    (
        "Generator.spectrum",
        "Name of harmonic voltage or current spectrum for this generator. Voltage behind Xd\" for machine - default. Current injection for inverter. Default value is \"default\", which is defined when the DSS starts.",
    ),
    (
        "Generator.status",
        "={Fixed | Variable*}.  If Fixed, then dispatch multipliers do not apply. The generator is alway at full power when it is ON.  Default is Variable  (follows curves).",
    ),
    (
        "Generator.usefuel",
        "{Yes | *No}. Activates the use of fuel for the operation of the generator. When the fuel level reaches the reserve level, the generator stops until it gets refueled. By default, the generator is connected to a continuous fuel supply, Use this mode to mimic dependency on fuel level for different generation technologies.",
    ),
    (
        "Generator.userdata",
        "String (in quotes or parentheses) that gets passed to user-written model for defining the data required for that model.",
    ),
    (
        "Generator.usermodel",
        "Name of DLL containing user-written model, which computes the terminal currents for Dynamics studies, overriding the default model.  Set to \"none\" to negate previous setting.",
    ),
    (
        "Generator.vmaxpu",
        "Default = 1.10.  Maximum per unit voltage for which the Model is assumed to apply. Above this value, the load model reverts to a constant impedance model.",
    ),
    (
        "Generator.vminpu",
        "Default = 0.90.  Minimum per unit voltage for which the Model is assumed to apply. Below this value, the load model reverts to a constant impedance model. For model 7, the current is limited to the value computed for constant power at Vminpu.",
    ),
    (
        "Generator.vpu",
        "Per Unit voltage set point for Model = 3  (typical power flow model).  Default is 1.0. ",
    ),
    (
        "Generator.xd",
        "Per unit synchronous reactance of machine. Presently used only for Thevenin impedance for power flow calcs of user models (model=6). Typically use a value 0.4 to 1.0. Default is 1.0",
    ),
    (
        "Generator.xdp",
        "Per unit transient reactance of the machine.  Used for Dynamics mode and Fault studies.  Default is 0.27.For user models, this value is used for the Thevenin/Norton impedance for Dynamics Mode.",
    ),
    (
        "Generator.xdpp",
        "Per unit subtransient reactance of the machine.  Used for Harmonics. Default is 0.20.",
    ),
    (
        "Generator.xneut",
        "Removed due to causing confusion - Add neutral impedance externally.",
    ),
    (
        "Generator.xrdp",
        "Default is 20. X/R ratio for Xdp property for FaultStudy and Dynamic modes.",
    ),
    (
        "Generator.yearly",
        "Dispatch shape to use for yearly simulations.  Must be previously defined as a Loadshape object. If this is not specified, a constant value is assumed (no variation). If the generator is assumed to be ON continuously, specify Status=FIXED, or designate a curve that is 1.0 per unit at all times. Set to NONE to reset to no loadshape. Nominally for 8760 simulations.  If there are fewer points in the designated shape than the number of points in the solution, the curve is repeated.",
    ),
    ("Generic5.basefreq", "Base Frequency for ratings."),
    (
        "Generic5.bus1",
        "Bus to which the Induction Machine is connected.  May include specific node specification.",
    ),
    (
        "Generic5.cc_switch",
        "CC_Switch: default value is false.\nCC_Switch = true --cooperate control on\nCC_Switch = false -- cooperate control off",
    ),
    (
        "Generic5.cluster_num",
        "Cluster_num: has to be coincident with Fmonitor attached. Default value is 0",
    ),
    (
        "Generic5.conn",
        "Connection of stator: Delta or Wye. Default is Delta.",
    ),
    (
        "Generic5.ctrl_mode",
        "ctrl mode:     /// contrl mode     ///    ctrl_mode =0; phases = 3;  // pos avg control---p_ref, V_ref, Q_ref    \\n \n ///    ctrl_mode =1; phases = 1; bus1 = 452.1;      ---p_ref1, V_ref1, Q_ref1 \\n\n///    ctrl_mode =2; phases = 1; bus1 = 452.2;      ---p_ref2, V_ref2, Q_ref2 \\n\n///    ctrl_mode =3; phases = 1; bus1 = 452.3;      ---p_ref3, V_ref3, Q_ref3 \\n\n///    ctrl_mode =4; phases = 3; bus1 = 452.2;      ---p_ref1,2,3, V_ref1,2,3, Q_ref1,2,3",
    ),
    (
        "Generic5.d",
        "Damping constant.  Usual range is 0 to 4. Default is 1.0.  Adjust to get damping in Dynamics mode,",
    ),
    (
        "Generic5.daily",
        "LOADSHAPE object to use for daily simulations.  Must be previously defined as a Loadshape object of 24 hrs, typically. Set Status=Fixed to ignore Loadshape designation. Set to NONE to reset to no loadshape. Default is no variation (constant) if not defined. Side effect: Sets Yearly load shape if not already defined.",
    ),
    ("Generic5.debugtrace", "[Yes | No*] Write DebugTrace file."),
    (
        "Generic5.droop",
        "droop type: integer: 2- Q = kcq_drp2 * (1-v_dg). others: integral droop with kcq.",
    ),
    (
        "Generic5.duty",
        "LOADSHAPE object to use for duty cycle simulations.  Must be previously defined as a Loadshape object.  Typically would have time intervals less than 1 hr. Designate the number of points to solve using the Set Number=xxxx command. If there are fewer points in the actual shape, the shape is assumed to repeat.Set to NONE to reset to no loadshape. Set Status=Fixed to ignore Loadshape designation.  Defaults to Daily curve If not specified.",
    ),
    (
        "Generic5.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "Generic5.h",
        "Per unit mass constant of the machine.  MW-sec/MVA.  Default is 1.0.",
    ),
    ("Generic5.kcd", "kcd: Idi control gain"),
    ("Generic5.kcq", "kcq: Iqi control gain to delta V"),
    ("Generic5.kcq_drp2", "kcq_drp2. the droop gain: 0.0~0.1"),
    ("Generic5.kqi", "kqi: Iqi control gain to delta Q"),
    (
        "Generic5.kv",
        "Nominal rated (1.0 per unit) voltage, kV. For 2- and 3-phase machines, specify phase-phase kV. Otherwise, specify actual kV across each branch of the machine. If wye (star), specify phase-neutral kV. If delta or phase-phase connected, specify phase-phase kV.",
    ),
    ("Generic5.kva", "Rated kVA for the machine."),
    (
        "Generic5.kw",
        "Shaft Power, kW, for the Induction Machine. Output limit of a DG",
    ),
    (
        "Generic5.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Generic5.maxslip",
        "Max slip value to allow. Default is 0.1. Set this before setting slip.",
    ),
    (
        "Generic5.p_ref1kw",
        "P_ref1kW = 10, goes to P_ref1, unit kW, 1st phase set power",
    ),
    (
        "Generic5.p_ref2kw",
        "P_ref2kW = 10, goes to P_ref2, unit kW, 2nd phase set power",
    ),
    (
        "Generic5.p_ref3kw",
        "P_ref3kW = 10, goes to P_ref3, unit kW, 3rd phase set power",
    ),
    (
        "Generic5.p_refkw",
        "P_refkW = 10, goes to P_ref. Ref P Value (kW). P_ref has priority to kW which is nominal value. (Incide variable P_ref is W)",
    ),
    ("Generic5.pbiaskw", "Pbias = -0.1, 0 by default, see PmppkW"),
    (
        "Generic5.pf",
        "[Read Only] Present power factor for the machine. ",
    ),
    ("Generic5.pfctr1", "Pfctr1 = 0.16, see PmppkW"),
    ("Generic5.pfctr2", "Pfctr2 = 1, 1 by default, see PmppkW"),
    ("Generic5.pfctr3", "Pfctr3 = 1, 1 by default, see PmppkW"),
    ("Generic5.pfctr4", "Pfctr4= 1, 1 by default, see PmppkW"),
    ("Generic5.pfctr5", "Pfctr5 =1, 1 by default, see PmppkW"),
    ("Generic5.pfctr6", "Pfctr6 = 1, 1 by default, see PmppkW"),
    (
        "Generic5.phases",
        "Number of Phases, this Induction Machine.  ",
    ),
    (
        "Generic5.pmaxkw",
        "PmaxkW = 100, goes to Pmax, unit kW, set max active power output; Operation limit of active power for DG\n  Pmax should be less than or equal to kW",
    ),
    (
        "Generic5.pminkw",
        "PminkW = 10, goes to Pmin, unit kW; Operation limit of active power for DG",
    ),
    (
        "Generic5.pmppkw",
        "PmppkW = 100, goes to Pmpp, unit kW, input Pmpp to calculate kW;\n  kW := (Pmpp + Pbias)*Pfctr1*Pfctr2*Pfctr3*Pfctr4*Pfctr5*Pfctr6;\nPbias = 0 by default, Pfctr*=1 by default; These properties will overwrite kW.",
    ),
    (
        "Generic5.pqpriority",
        "PQpriority, goes to PQpriority, define how to set Qmax. 0: Q,1: P ",
    ),
    (
        "Generic5.q_ref1kvar",
        "Q_ref1kVAr=10. Unit Qvar. Ref Q kVAr Value: work only when V_ref is not set",
    ),
    (
        "Generic5.q_ref2kvar",
        "Q_ref2kVAr=10. Unit Qvar. Ref Q kVAr Value: work only when V_ref is not set",
    ),
    (
        "Generic5.q_ref3kvar",
        "Q_ref3kVAr=10. Unit Qvar. Ref Q kVAr Value: work only when V_ref is not set",
    ),
    (
        "Generic5.q_refkvar",
        "Q_refkVAr=10. Unit Qvar. Ref Q kVAr Value: work only when V_ref is not set",
    ),
    ("Generic5.qv_flag", "QV_flag : 0-Q_ref mode; 1- V_ref mode"),
    (
        "Generic5.slipoption",
        "Option for slip model. One of {fixedslip | variableslip*  }",
    ),
    (
        "Generic5.spectrum",
        "Name of harmonic voltage or current spectrum for this IndMach012. Voltage behind Xd\" for machine - default. Current injection for inverter. Default value is \"default\", which is defined when the DSS starts.",
    ),
    (
        "Generic5.v_ref1kvln",
        "V_ref1kVLN = 2.16, 1st phase set V, (Unit kV, L-N value): V mode will work if QV_flag =1(by default) V_ref is set which is prior to Q_ref ",
    ),
    (
        "Generic5.v_ref2kvln",
        "V_ref2kVLN = 2.16, 2nd phase set V, (Unit kV, L-N value): V mode will work if QV_flag =1(by default) V_ref is set which is prior to Q_ref ",
    ),
    (
        "Generic5.v_ref3kvln",
        "V_ref3kVLN = 2.16, 3rd phase set V, (Unit kV, L-N value): V mode will work if QV_flag =1(by default) V_ref is set which is prior to Q_ref ",
    ),
    (
        "Generic5.v_refkvln",
        "V_refkVLN = 2.16, pos sequence set V. V_ref (Unit kV, L-N value): V mode will work if QV_flag =1(by default) V_ref is set which is prior to Q_ref",
    ),
    (
        "Generic5.volt_trhd",
        "Volt_Trhd. 0.~0.05. 0 means v has to follow v_ref",
    ),
    (
        "Generic5.yearly",
        "LOADSHAPE object to use for yearly simulations.  Must be previously defined as a Loadshape object. Is set to the Daily load shape  when Daily is defined.  The daily load shape is repeated in this case. Set Status=Fixed to ignore Loadshape designation. Set to NONE to reset to no loadshape. The default is no variation.",
    ),
    (
        "GrowthShape.csvfile",
        "Switch input of growth curve data to a csv file containing (year, mult) points, one per line.",
    ),
    (
        "GrowthShape.dblfile",
        "Switch input of growth curve data to a binary file of doubles containing (year, mult) points, packed one after another.",
    ),
    (
        "GrowthShape.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "GrowthShape.mult",
        "Array of growth multiplier values, or a text file spec, corresponding to the year values. Enter the multiplier by which you would multiply the previous year's load to get the present year's.\n\nExamples:\n\n  Year = [1, 2, 5]   Mult=[1.05, 1.025, 1.02].\n  Year= (File=years.txt) Mult= (file=mults.txt).\n\nText files contain one value per line.",
    ),
    (
        "GrowthShape.npts",
        "Number of points to expect in subsequent vector.",
    ),
    (
        "GrowthShape.sngfile",
        "Switch input of growth curve data to a binary file of singles containing (year, mult) points, packed one after another.",
    ),
    (
        "GrowthShape.year",
        "Array of year values, or a text file spec, corresponding to the multipliers. Enter only those years where the growth changes. May be any integer sequence -- just so it is consistent. See help on Mult.",
    ),
    ("IndMach012.basefreq", "Base Frequency for ratings."),
    (
        "IndMach012.bus1",
        "Bus to which the Induction Machine is connected.  May include specific node specification.",
    ),
    (
        "IndMach012.conn",
        "Connection of stator: Delta or Wye. Default is Delta.",
    ),
    (
        "IndMach012.d",
        "Damping constant.  Usual range is 0 to 4. Default is 1.0.  Adjust to get damping in Dynamics mode,",
    ),
    (
        "IndMach012.daily",
        "LOADSHAPE object to use for daily simulations.  Must be previously defined as a Loadshape object of 24 hrs, typically. Set Status=Fixed to ignore Loadshape designation. Set to NONE to reset to no loadshape. Default is no variation (constant) if not defined. Side effect: Sets Yearly load shape if not already defined.",
    ),
    (
        "IndMach012.debugtrace",
        "[Yes | No*] Write DebugTrace file.",
    ),
    (
        "IndMach012.duty",
        "LOADSHAPE object to use for duty cycle simulations.  Must be previously defined as a Loadshape object.  Typically would have time intervals less than 1 hr. Designate the number of points to solve using the Set Number=xxxx command. If there are fewer points in the actual shape, the shape is assumed to repeat.Set to NONE to reset to no loadshape. Set Status=Fixed to ignore Loadshape designation.  Defaults to Daily curve If not specified.",
    ),
    (
        "IndMach012.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "IndMach012.h",
        "Per unit mass constant of the machine.  MW-sec/MVA.  Default is 1.0.",
    ),
    (
        "IndMach012.kv",
        "Nominal rated (1.0 per unit) voltage, kV. For 2- and 3-phase machines, specify phase-phase kV. Otherwise, specify actual kV across each branch of the machine. If wye (star), specify phase-neutral kV. If delta or phase-phase connected, specify phase-phase kV.",
    ),
    ("IndMach012.kva", "Rated kVA for the machine."),
    (
        "IndMach012.kw",
        "Shaft Power, kW, for the Induction Machine.  A positive value denotes power for a load. \nNegative value denotes an induction generator. ",
    ),
    (
        "IndMach012.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "IndMach012.maxslip",
        "Max slip value to allow. Default is 0.1. Set this before setting slip.",
    ),
    (
        "IndMach012.pf",
        "[Read Only] Present power factor for the machine. ",
    ),
    (
        "IndMach012.phases",
        "Number of Phases, this Induction Machine.  ",
    ),
    (
        "IndMach012.purr",
        "Per unit rotor  resistance. Default is 0.007.",
    ),
    (
        "IndMach012.purs",
        "Per unit stator resistance. Default is 0.0053.",
    ),
    (
        "IndMach012.puxm",
        "Per unit magnetizing reactance.Default is 4.0.",
    ),
    (
        "IndMach012.puxr",
        "Per unit rotor leakage reactance. Default is 0.12.",
    ),
    (
        "IndMach012.puxs",
        "Per unit stator leakage reactance. Default is 0.106.",
    ),
    ("IndMach012.slip", "Initial slip value. Default is 0.007"),
    (
        "IndMach012.slipoption",
        "Option for slip model. One of {fixedslip | variableslip*  }",
    ),
    (
        "IndMach012.spectrum",
        "Name of harmonic voltage or current spectrum for this IndMach012. Voltage behind Xd\" for machine - default. Current injection for inverter. Default value is \"default\", which is defined when the DSS starts.",
    ),
    (
        "IndMach012.yearly",
        "LOADSHAPE object to use for yearly simulations.  Must be previously defined as a Loadshape object. Is set to the Daily load shape  when Daily is defined.  The daily load shape is repeated in this case. Set Status=Fixed to ignore Loadshape designation. Set to NONE to reset to no loadshape. The default is no variation.",
    ),
    (
        "InvControl.activepchangetolerance",
        "Required for VOLTWATT. Default is 0.01\n\nTolerance in pu of the convergence of the control loop associated with active power. For the same control iteration, this value is compared to the difference between the active power limit in pu resulted from the convergence process and the one resulted from the volt-watt function.\n\nThis reactive power tolerance value plus the voltage tolerance value (VoltageChangeTolerance) determine, together, when to stop control iterations by the InvControl.  \n\nIf an InvControl is controlling more than one PVSystem/Storage, each PVSystem/Storage has this quantity calculated independently, and so an individual PVSystem/Storage may reach the tolerance within different numbers of control iterations.",
    ),
    (
        "InvControl.argrahiv",
        "Required for the dynamic reactive current mode (DYNAMICREACCURR), and defaults to 0.1  \n\nThis is a gradient, expressed in unit-less terms of %/%, to establish the ratio by which percentage inductive reactive power production is increased as the  percent delta-voltage decreases above DbVMax. \n\nPercent delta-voltage is defined as the present PVSystem/Storage terminal voltage minus the moving average voltage, expressed as a percentage of the rated voltage for the PVSystem/Storage object. \n\nNote, the moving average voltage for the dynamic reactive current mode is different than the mmoving average voltage for the volt-watt and volt-var modes.",
    ),
    (
        "InvControl.argralowv",
        "Required for the dynamic reactive current mode (DYNAMICREACCURR), and defaults to 0.1  \n\nThis is a gradient, expressed in unit-less terms of %/%, to establish the ratio by which percentage capacitive reactive power production is increased as the  percent delta-voltage decreases below DbVMin. \n\nPercent delta-voltage is defined as the present PVSystem/Storage terminal voltage minus the moving average voltage, expressed as a percentage of the rated voltage for the PVSystem/Storage object. \n\nNote, the moving average voltage for the dynamic reactive current mode is different than the moving average voltage for the volt-watt and volt-var modes.",
    ),
    (
        "InvControl.avgwindowlen",
        "Required for VOLTVAR mode and VOLTWATT mode, and defaults to 0 seconds (0s). \n\nSets the length of the averaging window over which the average PVSystem/Storage terminal voltage is calculated. \n\nUnits are indicated by appending s, m, or h to the integer value. \n\nThe averaging window will calculate the average PVSystem/Storage terminal voltage over the specified period of time, up to and including the last power flow solution. \n\nNote, if the solution stepsize is larger than the window length, then the voltage will be assumed to have been constant over the time-frame specified by the window length.",
    ),
    ("InvControl.basefreq", "Base Frequency for ratings."),
    (
        "InvControl.combimode",
        "Combination of smart inverter functions in which the InvControl will control the PC elements in DERList, according to the options below: \n\nMust be a combination of the following: {VV_VW | VV_DRC}. Default is to not set this property, in which case the single control mode in Mode is active.  \n\nIn combined VV_VW mode, both volt-var and volt-watt control modes are active simultaneously.  See help individually for volt-var mode and volt-watt mode in Mode property.\nNote that the PVSystem/Storage will attempt to achieve both the volt-watt and volt-var set-points based on the capabilities of the inverter in the PVSystem/Storage (kVA rating, etc), any limits set on maximum active power,\n\nIn combined VV_DRC, both the volt-var and the dynamic reactive current modes are simultaneously active.",
    ),
    (
        "InvControl.controlmodel",
        "Integer defining the method for moving across the control curve. It can be one of the following:\n\n0 = Linear mode (default)\n1 = Exponential\n\nUse this property for better tunning your controller and improve the controller response in terms of control iterations needed to reach the target.\nThis property alters the meaning of deltaQ_factor and deltaP_factor properties according to its value (Check help). The method can also be combined with the controller tolerance for improving performance.",
    ),
    (
        "InvControl.dbvmax",
        "Required for the dynamic reactive current mode (DYNAMICREACCURR), and defaults to 1.05 per-unit voltage (referenced to the PVSystem object rated voltage or a windowed average value). \n\nThis parameter is the maximum voltage that defines the voltage dead-band within which no reactive power is allowed to be generated. ",
    ),
    (
        "InvControl.dbvmin",
        "Required for the dynamic reactive current mode (DYNAMICREACCURR), and defaults to 0.95 per-unit voltage (referenced to the PVSystem/Storage object rated voltage or a windowed average value). \n\nThis parameter is the minimum voltage that defines the voltage dead-band within which no reactive power is allowed to be generated. ",
    ),
    (
        "InvControl.deltap_factor",
        "Required for the VOLTWATT modes.  Defaults to -1.0. \n\nDefining -1.0, OpenDSS takes care internally of delta_P itself. It tries to improve convergence as well as speed up process\n\nDefining between 0.05 and 1.0, it sets the maximum change (in unit of the y-axis) from the prior active power output level to the desired active power output level during each control iteration. \n\n\nIf numerical instability is noticed in solutions such as active power changing substantially from one control iteration to the next and/or voltages oscillating between two values with some separation, this is an indication of numerical instability (use the EventLog to diagnose). \n\nIf the maximum control iterations are exceeded, and no numerical instability is seen in the EventLog of via monitors, then try increasing the value of this parameter to reduce the number of control iterations needed to achieve the control criteria, and move to the power flow solution.",
    ),
    (
        "InvControl.deltaq_factor",
        "Required for the VOLTVAR and DYNAMICREACCURR modes.  Defaults to -1.0. \n\nDefining -1.0, OpenDSS takes care internally of delta_Q itself. It tries to improve convergence as well as speed up process\n\nSets the maximum change (in per unit) from the prior var output level to the desired var output level during each control iteration. \n\n\nif numerical instability is noticed in solutions such as var sign changing from one control iteration to the next and voltages oscillating between two values with some separation, this is an indication of numerical instability (use the EventLog to diagnose). \n\nif the maximum control iterations are exceeded, and no numerical instability is seen in the EventLog of via monitors, then try increasing the value of this parameter to reduce the number of control iterations needed to achieve the control criteria, and move to the power flow solution. \n\nWhen operating the controller using exponential control model (see CtrlModel), this parameter represents the sampling time gain of the controller, which is used for accelrating the controller response in terms of control iterations required.",
    ),
    (
        "InvControl.derlist",
        "Array list of PVSystem and/or Storage elements to be controlled. If not specified, all PVSystem and Storage in the circuit are assumed to be controlled by this control. \n\nNo capability of hierarchical control between two controls for a single element is implemented at this time.",
    ),
    (
        "InvControl.dynreacavgwindowlen",
        "Required for the dynamic reactive current mode (DYNAMICREACCURR), and defaults to 1 seconds (1s). do not use a value smaller than 1.0 \n\nSets the length of the averaging window over which the average PVSystem/Storage terminal voltage is calculated for the dynamic reactive current mode. \n\nUnits are indicated by appending s, m, or h to the integer value. \n\nTypically this will be a shorter averaging window than the volt-var and volt-watt averaging window.\n\nThe averaging window will calculate the average PVSystem/Storage terminal voltage over the specified period of time, up to and including the last power flow solution.  Note, if the solution stepsize is larger than the window length, then the voltage will be assumed to have been constant over the time-frame specified by the window length.",
    ),
    (
        "InvControl.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "InvControl.eventlog",
        "{Yes/True | No/False*} Default is NO for InvControl. Log control actions to Eventlog.",
    ),
    (
        "InvControl.hysteresis_offset",
        "Required for VOLTVAR mode, and defaults to 0. \n\nfor the times when the terminal voltage is decreasing, this is the off-set in per-unit voltage of a curve whose shape is the same as vvc_curve. It is offset by a certain negative value of per-unit voltage, which is defined by the base quantity for the x-axis of the volt-var curve (see help for voltage_curvex_ref)\n\nif the PVSystem/Storage terminal voltage has been increasing, and has not changed directions, utilize vvc_curve1 for the volt-var response. \n\nif the PVSystem/Storage terminal voltage has been increasing and changes directions and begins to decrease, then move from utilizing vvc_curve1 to a volt-var curve of the same shape, but offset by a certain per-unit voltage value. \n\nMaintain the same per-unit available var output level (unless head-room has changed due to change in active power or kva rating of PVSystem/Storage).  Per-unit var values remain the same for this internally constructed second curve (hysteresis curve). \n\nif the terminal voltage has been decreasing and changes directions and begins to increase , then move from utilizing the offset curve, back to the vvc_curve1 for volt-var response, but stay at the same per-unit available vars output level.",
    ),
    (
        "InvControl.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "InvControl.lpftau",
        "Not required. Defaults to 0 seconds. \n\nFilter time constant of the LPF option of the RateofChangeMode property. The time constant will cause the low-pass filter to achieve 95% of the target value in 3 time constants.",
    ),
    (
        "InvControl.mode",
        "Smart inverter function in which the InvControl will control the PC elements specified in DERList, according to the options below:\n\nMust be one of: {VOLTVAR | VOLTWATT | DYNAMICREACCURR | WATTPF | WATTVAR | GFM} \nif the user desires to use modes simultaneously, then set the CombiMode property. Setting the Mode to any valid value disables combination mode.\n\nIn volt-var mode. This mode attempts to CONTROL the vars, according to one or two volt-var curves, depending on the monitored voltages, present active power output, and the capabilities of the PVSystem/Storage. \n\nIn volt-watt mode. This mode attempts to LIMIT the watts, according to one defined volt-watt curve, depending on the monitored voltages and the capabilities of the PVSystem/Storage. \n\nIn dynamic reactive current mode. This mode attempts to increasingly counter deviations by CONTROLLING vars, depending on the monitored voltages, present active power output, and the capabilities of the of the PVSystem/Storage.\n\nIn watt-pf mode. This mode attempts to CONTROL the vars, according to a watt-pf curve, depending on the present active power output, and the capabilities of the PVSystem/Storage. \n\nIn watt-var mode. This mode attempts to CONTROL the vars, according to a watt-var curve, depending on the present active power output, and the capabilities of the PVSystem/Storage. \n\nIn GFM mode this control will trigger the GFM control routine for the DERs within the DERList. The GFM actiosn will only take place if the pointed DERs are in GFM mode. The controller parameters are locally setup at the DER.\n\n\nNO DEFAULT",
    ),
    (
        "InvControl.monbus",
        "Name of monitored bus used by the voltage-dependent control modes. Default is bus of the controlled PVSystem/Storage or Storage.",
    ),
    (
        "InvControl.monbusesvbase",
        "Array list of rated voltages of the buses and their nodes presented in the monBus property. This list may have different line-to-line and/or line-to-ground voltages.",
    ),
    (
        "InvControl.monvoltagecalc",
        "Number of the phase being monitored or one of {AVG | MAX | MIN} for all phases. Default=AVG. ",
    ),
    (
        "InvControl.pvsystemlist",
        "Deprecated, use DERList instead.",
    ),
    (
        "InvControl.rateofchangemode",
        "Required for VOLTWATT and VOLTVAR mode.  Must be one of: {INACTIVE* | LPF | RISEFALL }.  The default is INACTIVE.  \n\nAuxiliary option that aims to limit the changes of the desired reactive power and the active power limit between time steps, the alternatives are listed below: \n\nINACTIVE. It indicates there is no limit on rate of change imposed for either active or reactive power output. \n\nLPF. A low-pass RC filter is applied to the desired reactive power and/or the active power limit to determine the output power as a function of a time constant defined in the LPFTau property. \n\nRISEFALL. A rise and fall limit in the change of active and/or reactive power expressed in terms of pu power per second, defined in the RiseFallLimit, is applied to the desired reactive power and/or the active power limit. ",
    ),
    (
        "InvControl.refreactivepower",
        "Required for any mode that has VOLTVAR, DYNAMICREACCURR and WATTVAR. Defaults to VARAVAL.\n\nDefines the base reactive power for both the provided and absorbed reactive power, according to one of the following options: \n\nVARAVAL. The base values for the provided and absorbed reactive power are equal to the available reactive power.\n\nVARMAX: The base values of the provided and absorbed reactive power are equal to the value defined in the kvarMax and kvarMaxAbs properties, respectively.",
    ),
    (
        "InvControl.risefalllimit",
        "Not required.  Defaults to no limit (-1). Must be -1 (no limit) or a positive value.  \n\nLimit in power in pu per second used by the RISEFALL option of the RateofChangeMode property.The base value for this ramp is defined in the RefReactivePower property and/or in VoltwattYAxis.",
    ),
    (
        "InvControl.varchangetolerance",
        "Required for VOLTVAR and DYNAMICREACCURR modes.  Defaults to 0.025 per unit of the base provided or absorbed reactive power described in the RefReactivePower property This parameter should only be modified by advanced users of the InvControl. \n\nTolerance in pu of the convergence of the control loop associated with reactive power. For the same control iteration, this value is compared to the difference, as an absolute value (without sign), between the desired reactive power value in pu and the output reactive power in pu of the controlled element.\n\nThis reactive power tolerance value plus the voltage tolerance value (VoltageChangeTolerance) determine, together, when to stop control iterations by the InvControl.  \n\nIf an InvControl is controlling more than one PVSystem/Storage, each PVSystem/Storage has this quantity calculated independently, and so an individual PVSystem/Storage may reach the tolerance within different numbers of control iterations.",
    ),
    (
        "InvControl.voltage_curvex_ref",
        "Required for VOLTVAR and VOLTWATT modes, and defaults to rated.  Possible values are: {rated|avg|ravg}.  \n\nDefines whether the x-axis values (voltage in per unit) for vvc_curve1 and the volt-watt curve corresponds to:\n\nrated. The rated voltage for the PVSystem/Storage object (1.0 in the volt-var curve equals rated voltage).\n\navg. The average terminal voltage recorded over a certain number of prior power-flow solutions.\nwith the avg setting, 1.0 per unit on the x-axis of the volt-var curve(s) corresponds to the average voltage.\nfrom a certain number of prior intervals.  See avgwindowlen parameter.\n\nravg. Same as avg, with the exception that the avgerage terminal voltage is divided by the rated voltage.",
    ),
    (
        "InvControl.voltagechangetolerance",
        "Defaults to 0.0001 per-unit voltage.  This parameter should only be modified by advanced users of the InvControl.  \n\nTolerance in pu of the control loop convergence associated to the monitored voltage in pu. This value is compared with the difference of the monitored voltage in pu of the current and previous control iterations of the control loop\n\nThis voltage tolerance value plus the var/watt tolerance value (VarChangeTolerance/ActivePChangeTolerance) determine, together, when to stop control iterations by the InvControl. \n\nIf an InvControl is controlling more than one PVSystem/Storage, each PVSystem/Storage has this quantity calculated independently, and so an individual PVSystem/Storage may reach the tolerance within different numbers of control iterations.",
    ),
    (
        "InvControl.voltwatt_curve",
        "Required for VOLTWATT mode. \n\nName of the XYCurve object containing the volt-watt curve. \n\nUnits for the x-axis are per-unit voltage, which may be in per unit of the rated voltage for the PVSystem/Storage, or may be in per unit of the average voltage at the terminals over a user-defined number of prior solutions. \n\nUnits for the y-axis are either in one of the options described in the VoltwattYAxis property. ",
    ),
    (
        "InvControl.voltwattch_curve",
        "Required for VOLTWATT mode for Storage element in CHARGING state. \n\nThe name of an XYCurve object that describes the variation in active power output (in per unit of maximum active power output for the Storage). \n\nUnits for the x-axis are per-unit voltage, which may be in per unit of the rated voltage for the Storage, or may be in per unit of the average voltage at the terminals over a user-defined number of prior solutions. \n\nUnits for the y-axis are either in: (1) per unit of maximum active power output capability of the Storage, or (2) maximum available active power output capability (defined by the parameter: VoltwattYAxis), corresponding to the terminal voltage (x-axis value in per unit). \n\nNo default -- must be specified for VOLTWATT mode for Storage element in CHARGING state.",
    ),
    (
        "InvControl.voltwattyaxis",
        "Required for VOLTWATT mode.  Must be one of: {PMPPPU* | PAVAILABLEPU| PCTPMPPPU | KVARATINGPU}.  The default is PMPPPU.  \n\nUnits for the y-axis of the volt-watt curve while in volt-watt mode. \n\nWhen set to PMPPPU. The y-axis corresponds to the value in pu of Pmpp property of the PVSystem. \n\nWhen set to PAVAILABLEPU. The y-axis corresponds to the value in pu of the available active power of the PVSystem. \n\nWhen set to PCTPMPPPU. The y-axis corresponds to the value in pu of the power Pmpp multiplied by 1/100 of the %Pmpp property of the PVSystem.\n\nWhen set to KVARATINGPU. The y-axis corresponds to the value in pu of the kVA property of the PVSystem.",
    ),
    (
        "InvControl.vsetpoint",
        "Required for Active Voltage Regulation (AVR).",
    ),
    (
        "InvControl.vv_refreactivepower",
        "Deprecated, use RefReactivePower instead.",
    ),
    (
        "InvControl.vvc_curve1",
        "Required for VOLTVAR mode. \n\nName of the XYCurve object containing the volt-var curve. The positive values of the y-axis of the volt-var curve represent values in pu of the provided base reactive power. The negative values of the y-axis are values in pu of the absorbed base reactive power. \nProvided and absorbed base reactive power values are defined in the RefReactivePower property\n\nUnits for the x-axis are per-unit voltage, which may be in per unit of the rated voltage for the PVSystem/Storage, or may be in per unit of the average voltage at the terminals over a user-defined number of prior solutions. ",
    ),
    (
        "InvControl.wattpf_curve",
        "Required for WATTPF mode.\n\nName of the XYCurve object containing the watt-pf curve.\nThe positive values of the y-axis are positive power factor values. The negative values of the the y-axis are negative power factor values. When positive, the output reactive power has the same direction of the output active power, and when negative, it has the opposite direction.\nUnits for the x-axis are per-unit output active power, and the base active power is the Pmpp for PVSystem and kWrated for Storage.\n\nThe y-axis represents the power factor and the reference is power factor equal to 0. \n\nFor example, if the user wants to define the following XY coordinates: (0, 0.9); (0.2, 0.9); (0.5, -0.9); (1, -0.9).\nTry to plot them considering the y-axis reference equal to unity power factor.\n\nThe user needs to translate this curve into a plot in which the y-axis reference is equal to 0 power factor.It means that two new XY coordinates need to be included, in this case they are: (0.35, 1); (0.35, -1).\nTry to plot them considering the y-axis reference equal to 0 power factor.\nThe discontinuity in 0.35pu is not a problem since var is zero for either power factor equal to 1 or -1.",
    ),
    (
        "InvControl.wattvar_curve",
        "Required for WATTVAR mode. \n\nName of the XYCurve object containing the watt-var curve. The positive values of the y-axis of the watt-var curve represent values in pu of the provided base reactive power. The negative values of the y-axis are values in pu of the absorbed base reactive power. \nProvided and absorbed base reactive power values are defined in the RefReactivePower property.\n\nUnits for the x-axis are per-unit output active power, and the base active power is the Pmpp for PVSystem and kWrated for Storage.",
    ),
    (
        "Isource.amps",
        "Magnitude of current source, each phase, in Amps.",
    ),
    (
        "Isource.angle",
        "Phase angle in degrees of first phase: e.g.,Angle=10.3.\nPhase shift between phases is assumed 120 degrees when number of phases <= 3",
    ),
    ("Isource.basefreq", "Base Frequency for ratings."),
    (
        "Isource.bus1",
        "Name of bus to which source is connected.\nbus1=busname\nbus1=busname.1.2.3",
    ),
    (
        "Isource.bus2",
        "Name of bus to which 2nd terminal is connected.\nbus2=busname\nbus2=busname.1.2.3\n\nDefault is Bus1.0.0.0 (grounded-wye connection)",
    ),
    (
        "Isource.daily",
        "LOADSHAPE object to use for the per-unit current for DAILY-mode simulations. Set the Mult property of the LOADSHAPE to the pu curve. Qmult is not used. If UseActual=Yes then the Mult curve should be actual A.\n\nMust be previously defined as a LOADSHAPE object. \n\nSets Yearly curve if it is not already defined.   Set to NONE to reset to no loadshape for Yearly mode. The default is no variation.",
    ),
    (
        "Isource.duty",
        "LOADSHAPE object to use for the per-unit current for DUTYCYCLE-mode simulations. Set the Mult property of the LOADSHAPE to the pu curve. Qmult is not used. If UseActual=Yes then the Mult curve should be actual A.\n\nMust be previously defined as a LOADSHAPE object. \n\nDefaults to Daily load shape when Daily is defined.   Set to NONE to reset to no loadshape for Yearly mode. The default is no variation.",
    ),
    (
        "Isource.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "Isource.frequency",
        "Source frequency.  Defaults to  circuit fundamental frequency.",
    ),
    (
        "Isource.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Isource.phases",
        "Number of phases.  Defaults to 3. For 3 or less, phase shift is 120 degrees.",
    ),
    (
        "Isource.scantype",
        "{pos*| zero | none} Maintain specified sequence for harmonic solution. Default is positive sequence. Otherwise, angle between phases rotates with harmonic.",
    ),
    (
        "Isource.sequence",
        "{pos*| neg | zero} Set the phase angles for the specified symmetrical component sequence for non-harmonic solution modes. Default is positive sequence. ",
    ),
    (
        "Isource.spectrum",
        "Harmonic spectrum assumed for this source.  Default is \"default\".",
    ),
    (
        "Isource.yearly",
        "LOADSHAPE object to use for the per-unit current for YEARLY-mode simulations. Set the Mult property of the LOADSHAPE to the pu curve. Qmult is not used. If UseActual=Yes then the Mult curve should be actual Amp.\n\nMust be previously defined as a LOADSHAPE object. \n\nIs set to the Daily load shape when Daily is defined.  The daily load shape is repeated in this case. Set to NONE to reset to no loadshape for Yearly mode. The default is no variation.",
    ),
    (
        "Line.b0",
        "Alternate way to specify C0. MicroS per unit length",
    ),
    (
        "Line.b1",
        "Alternate way to specify C1. MicroS per unit length",
    ),
    ("Line.basefreq", "Base Frequency for ratings."),
    (
        "Line.bus1",
        "Name of bus to which first terminal is connected.\nExample:\nbus1=busname   (assumes all terminals connected in normal phase order)\nbus1=busname.3.1.2.0 (specify terminal to node connections explicitly)",
    ),
    (
        "Line.bus2",
        "Name of bus to which 2nd terminal is connected.",
    ),
    (
        "Line.c0",
        "Zero-sequence capacitance, nf per unit length. Setting any of R1, R0, X1, X0, C1, C0 forces the program to use the symmetrical component line definition.See also B0.",
    ),
    (
        "Line.c1",
        "Positive-sequence capacitance, nf per unit length.  Setting any of R1, R0, X1, X0, C1, C0 forces the program to use the symmetrical component line definition. See also Cmatrix and B1.",
    ),
    (
        "Line.cmatrix",
        "Nodal Capacitance matrix, lower triangle, nf per unit length.Order of the matrix is the number of phases. May be used to specify the shunt capacitance of any line configuration. Using any of Rmatrix, Xmatrix, Cmatrix forces program to use the matrix values for line impedance definition.  For balanced line models, you may use the standard symmetrical component data definition instead.",
    ),
    (
        "Line.cncables",
        "Array of CNData names for use in a cable constants calculation.\nMust be used in conjunction with the Spacing property.\nSpecify the Spacing first, using \"nphases\" cncables.\nYou may later specify \"nconds-nphases\" wires for separate neutrals",
    ),
    (
        "Line.earthmodel",
        "One of {Carson | FullCarson | Deri}. Default is the global value established with the Set EarthModel command. See the Options Help on EarthModel option. This is used to override the global value for this line. This option applies only when the \"geometry\" property is used.",
    ),
    ("Line.emergamps", "Maximum or emerg current."),
    (
        "Line.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "Line.faultrate",
        "Failure rate PER UNIT LENGTH per year. Length must be same units as LENGTH property. Default is 0.1 fault per unit length per year.",
    ),
    (
        "Line.geometry",
        "Geometry code for LineGeometry Object. Supersedes any previous definition of line impedance. Line constants are computed for each frequency change or rho change. CAUTION: may alter number of phases. You cannot subsequently change the number of phases unless you change how the line impedance is defined.",
    ),
    (
        "Line.length",
        "Length of line. Default is 1.0. If units do not match the impedance data, specify \"units\" property. ",
    ),
    (
        "Line.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Line.linecode",
        "Name of linecode object describing line impedances.\nIf you use a line code, you do not need to specify the impedances here. The line code must have been PREVIOUSLY defined. The values specified last will prevail over those specified earlier (left-to-right sequence of properties).  You can subsequently change the number of phases if symmetrical component quantities are specified.If no line code or impedance data are specified, the line object defaults to 336 MCM ACSR on 4 ft spacing.",
    ),
    (
        "Line.linetype",
        "Code designating the type of line. \nOne of: OH, UG, UG_TS, UG_CN, SWT_LDBRK, SWT_FUSE, SWT_SECT, SWT_REC, SWT_DISC, SWT_BRK, SWT_ELBOW, BUSBAR\n\nOpenDSS currently does not use this internally. For whatever purpose the user defines. Default is OH.",
    ),
    ("Line.normamps", "Normal rated current."),
    (
        "Line.pctperm",
        "Percent of failures that become permanent. Default is 20.",
    ),
    ("Line.phases", "Number of phases, this line."),
    (
        "Line.r0",
        "Zero-sequence Resistance, ohms per unit length. Setting any of R1, R0, X1, X0, C1, C0 forces the program to use the symmetrical component line definition.",
    ),
    (
        "Line.r1",
        "Positive-sequence Resistance, ohms per unit length. Setting any of R1, R0, X1, X0, C1, C0 forces the program to use the symmetrical component line definition. See also Rmatrix.",
    ),
    (
        "Line.ratings",
        "An array of ratings to be used when the seasonal ratings flag is True. It can be used to insert\nmultiple ratings to change during a QSTS simulation to evaluate different ratings in lines.",
    ),
    ("Line.repair", "Hours to repair. Default is 3 hr."),
    (
        "Line.rg",
        "Carson earth return resistance per unit length used to compute impedance values at base frequency. Default is 0.01805 = 60 Hz value in ohms per kft (matches default line impedances). This value is required for harmonic solutions if you wish to adjust the earth return impedances for frequency. If not, set both Rg and Xg = 0.",
    ),
    (
        "Line.rho",
        "Default=100 meter ohms.  Earth resistivity used to compute earth correction factor. Overrides Line geometry definition if specified.",
    ),
    (
        "Line.rmatrix",
        "Resistance matrix, lower triangle, ohms per unit length. Order of the matrix is the number of phases. May be used to specify the impedance of any line configuration. Using any of Rmatrix, Xmatrix, Cmatrix forces program to use the matrix values for line impedance definition. For balanced line models, you may use the standard symmetrical component data definition instead.",
    ),
    (
        "Line.seasons",
        "Defines the number of ratings to be defined for the wire, to be used only when defining seasonal ratings using the \"Ratings\" property.",
    ),
    (
        "Line.spacing",
        "Reference to a LineSpacing for use in a line constants calculation.\nMust be used in conjunction with the Wires property.\nSpecify this before the wires property.",
    ),
    (
        "Line.switch",
        "{y/n | T/F}  Default= no/false.  Designates this line as a switch for graphics and algorithmic purposes. \nSIDE EFFECT: Sets r1 = 1.0; x1 = 1.0; r0 = 1.0; x0 = 1.0; c1 = 1.1 ; c0 = 1.0;  length = 0.001; You must reset if you want something different.",
    ),
    (
        "Line.tscables",
        "Array of TSData names for use in a cable constants calculation.\nMust be used in conjunction with the Spacing property.\nSpecify the Spacing first, using \"nphases\" tscables.\nYou may later specify \"nconds-nphases\" wires for separate neutrals",
    ),
    (
        "Line.units",
        "Length Units = {none | mi|kft|km|m|Ft|in|cm } Default is None - assumes length units match impedance units.",
    ),
    (
        "Line.wires",
        "Array of WireData names for use in an overhead line constants calculation.\nMust be used in conjunction with the Spacing property.\nSpecify the Spacing first, and \"ncond\" wires.\nMay also be used to specify bare neutrals with cables, using \"ncond-nphase\" wires.",
    ),
    (
        "Line.x0",
        "Zero-sequence Reactance, ohms per unit length. Setting any of R1, R0, X1, X0, C1, C0 forces the program to use the symmetrical component line definition.",
    ),
    (
        "Line.x1",
        "Positive-sequence Reactance, ohms per unit length. Setting any of R1, R0, X1, X0, C1, C0 forces the program to use the symmetrical component line definition.  See also Xmatrix",
    ),
    (
        "Line.xg",
        "Carson earth return reactance per unit length used to compute impedance values at base frequency.  For making better frequency adjustments. Default is 0.155081 = 60 Hz value in ohms per kft (matches default line impedances). This value is required for harmonic solutions if you wish to adjust the earth return impedances for frequency. If not, set both Rg and Xg = 0.",
    ),
    (
        "Line.xmatrix",
        "Reactance matrix, lower triangle, ohms per unit length. Order of the matrix is the number of phases. May be used to specify the impedance of any line configuration. Using any of Rmatrix, Xmatrix, Cmatrix forces program to use the matrix values for line impedance definition.  For balanced line models, you may use the standard symmetrical component data definition instead.",
    ),
    (
        "LineCode.b0",
        "Alternate way to specify C0. MicroS per unit length",
    ),
    (
        "LineCode.b1",
        "Alternate way to specify C1. MicroS per unit length",
    ),
    (
        "LineCode.basefreq",
        "Frequency at which impedances are specified.",
    ),
    (
        "LineCode.c0",
        "Zero-sequence capacitance, nf per unit length. Setting any of R1, R0, X1, X0, C1, C0 forces the program to use the symmetrical component line definition. See also B0.",
    ),
    (
        "LineCode.c1",
        "Positive-sequence capacitance, nf per unit length. Setting any of R1, R0, X1, X0, C1, C0 forces the program to use the symmetrical component line definition. See also Cmatrix and B1.",
    ),
    (
        "LineCode.cmatrix",
        "Nodal Capacitance matrix, lower triangle, nf per unit length.Order of the matrix is the number of phases. May be used to specify the shunt capacitance of any line configuration.  For balanced line models, you may use the standard symmetrical component data definition instead.",
    ),
    (
        "LineCode.emergamps",
        "Emergency ampere limit on line (usually one-hour rating).",
    ),
    (
        "LineCode.faultrate",
        "Number of faults per unit length per year.",
    ),
    (
        "LineCode.kron",
        "Kron = Y/N. Default=N.  Perform Kron reduction on the impedance matrix after it is formed, reducing order by 1. Eliminates the conductor designated by the \"Neutral=\" property. Do this after the R, X, and C matrices are defined. Ignored for symmetrical components. May be issued more than once to eliminate more than one conductor by resetting the Neutral property after the previous invoking of this property. Generally, you do not want to do a Kron reduction on the matrix if you intend to solve at a frequency other than the base frequency and exploit the Rg and Xg values.",
    ),
    (
        "LineCode.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "LineCode.linetype",
        "Code designating the type of line. \nOne of: OH, UG, UG_TS, UG_CN, SWT_LDBRK, SWT_FUSE, SWT_SECT, SWT_REC, SWT_DISC, SWT_BRK, SWT_ELBOW, BUSBAR\n\nOpenDSS currently does not use this internally. For whatever purpose the user defines. Default is OH.",
    ),
    (
        "LineCode.neutral",
        "Designates which conductor is the \"neutral\" conductor that will be eliminated by Kron reduction. Default is the last conductor (nphases value). After Kron reduction is set to 0. Subsequent issuing of Kron=Yes will not do anything until this property is set to a legal value. Applies only to LineCodes defined by R, X, and C matrix.",
    ),
    (
        "LineCode.normamps",
        "Normal ampere limit on line.  This is the so-called Planning Limit. It may also be the value above which load will have to be dropped in a contingency.  Usually about 75% - 80% of the emergency (one-hour) rating.",
    ),
    (
        "LineCode.nphases",
        "Number of phases in the line this line code data represents.  Setting this property reinitializes the line code.  Impedance matrix is reset for default symmetrical component.",
    ),
    (
        "LineCode.pctperm",
        "Percentage of the faults that become permanent.",
    ),
    (
        "LineCode.r0",
        "Zero-sequence Resistance, ohms per unit length. Setting any of R1, R0, X1, X0, C1, C0 forces the program to use the symmetrical component line definition.",
    ),
    (
        "LineCode.r1",
        "Positive-sequence Resistance, ohms per unit length. Setting any of R1, R0, X1, X0, C1, C0 forces the program to use the symmetrical component line definition. See also Rmatrix.",
    ),
    (
        "LineCode.ratings",
        "An array of ratings to be used when the seasonal ratings flag is True. It can be used to insert\nmultiple ratings to change during a QSTS simulation to evaluate different ratings in lines.",
    ),
    ("LineCode.repair", "Hours to repair."),
    (
        "LineCode.rg",
        "Carson earth return resistance per unit length used to compute impedance values at base frequency.  For making better frequency adjustments. Default is 0.01805 = 60 Hz value in ohms per kft (matches default line impedances). This value is required for harmonic solutions if you wish to adjust the earth return impedances for frequency. If not, set both Rg and Xg = 0.",
    ),
    (
        "LineCode.rho",
        "Default=100 meter ohms.  Earth resitivity used to compute earth correction factor.",
    ),
    (
        "LineCode.rmatrix",
        "Resistance matrix, lower triangle, ohms per unit length. Order of the matrix is the number of phases. May be used to specify the impedance of any line configuration.  For balanced line models, you may use the standard symmetrical component data definition instead.",
    ),
    (
        "LineCode.seasons",
        "Defines the number of ratings to be defined for the wire, to be used only when defining seasonal ratings using the \"Ratings\" property.",
    ),
    (
        "LineCode.units",
        "One of (ohms per ...) {none|mi|km|kft|m|me|ft|in|cm}.  Default is none; assumes units agree with length units given in Line object",
    ),
    (
        "LineCode.x0",
        "Zero-sequence Reactance, ohms per unit length. Setting any of R1, R0, X1, X0, C1, C0 forces the program to use the symmetrical component line definition.",
    ),
    (
        "LineCode.x1",
        "Positive-sequence Reactance, ohms per unit length. Setting any of R1, R0, X1, X0, C1, C0 forces the program to use the symmetrical component line definition. See also Xmatrix",
    ),
    (
        "LineCode.xg",
        "Carson earth return reactance per unit length used to compute impedance values at base frequency.  For making better frequency adjustments. Default value is 0.155081 = 60 Hz value in ohms per kft (matches default line impedances). This value is required for harmonic solutions if you wish to adjust the earth return impedances for frequency. If not, set both Rg and Xg = 0.",
    ),
    (
        "LineCode.xmatrix",
        "Reactance matrix, lower triangle, ohms per unit length. Order of the matrix is the number of phases. May be used to specify the impedance of any line configuration.  For balanced line models, you may use the standard symmetrical component data definition instead.",
    ),
    (
        "LineGeometry.cncable",
        "Code from CNData. MUST BE PREVIOUSLY DEFINED. no default.\nSpecifies use of Concentric Neutral cable parameter calculation.",
    ),
    (
        "LineGeometry.cncables",
        "Array of CNData names for cable parameter calculation.\nAll must be previously defined, and match \"nphases\" for this geometry.\nYou can later define \"nconds-nphases\" wires for bare neutral conductors.",
    ),
    (
        "LineGeometry.cond",
        "Set this = number of the conductor you wish to define. Default is 1.",
    ),
    (
        "LineGeometry.emergamps",
        "Emergency ampacity, amperes. Defaults to first conductor if not specified.",
    ),
    ("LineGeometry.h", "Height of conductor."),
    (
        "LineGeometry.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "LineGeometry.linetype",
        "Code designating the type of line. \nOne of: OH, UG, UG_TS, UG_CN, SWT_LDBRK, SWT_FUSE, SWT_SECT, SWT_REC, SWT_DISC, SWT_BRK, SWT_ELBOW, BUSBAR\n\nOpenDSS currently does not use this internally. For whatever purpose the user defines. Default is OH.",
    ),
    (
        "LineGeometry.nconds",
        "Number of conductors in this geometry. Default is 3. Triggers memory allocations. Define first!",
    ),
    (
        "LineGeometry.normamps",
        "Normal ampacity, amperes for the line. Defaults to first conductor if not specified.",
    ),
    (
        "LineGeometry.nphases",
        "Number of phases. Default =3; All other conductors are considered neutrals and might be reduced out.",
    ),
    (
        "LineGeometry.ratings",
        "An array of ratings to be used when the seasonal ratings flag is True. It can be used to insert\nmultiple ratings to change during a QSTS simulation to evaluate different ratings in lines.Defaults to first conductor if not specified.",
    ),
    (
        "LineGeometry.reduce",
        "{Yes | No} Default = no. Reduce to Nphases (Kron Reduction). Reduce out neutrals.",
    ),
    (
        "LineGeometry.seasons",
        "Defines the number of ratings to be defined for the wire, to be used only when defining seasonal ratings using the \"Ratings\" property. Defaults to first conductor if not specified.",
    ),
    (
        "LineGeometry.spacing",
        "Reference to a LineSpacing for use in a line constants calculation.\nAlternative to x, h, and units. MUST BE PREVIOUSLY DEFINED.\nMust match \"nconds\" as previously defined for this geometry.\nMust be used in conjunction with the Wires property.",
    ),
    (
        "LineGeometry.tscable",
        "Code from TSData. MUST BE PREVIOUSLY DEFINED. no default.\nSpecifies use of Tape Shield cable parameter calculation.",
    ),
    (
        "LineGeometry.tscables",
        "Array of TSData names for cable parameter calculation.\nAll must be previously defined, and match \"nphases\" for this geometry.\nYou can later define \"nconds-nphases\" wires for bare neutral conductors.",
    ),
    (
        "LineGeometry.units",
        "Units for x and h: {mi|kft|km|m|Ft|in|cm } Initial default is \"ft\", but defaults to last unit defined",
    ),
    (
        "LineGeometry.wire",
        "Code from WireData. MUST BE PREVIOUSLY DEFINED. no default.\nSpecifies use of Overhead Line parameter calculation,\nUnless Tape Shield cable previously assigned to phases, and this wire is a neutral.",
    ),
    (
        "LineGeometry.wires",
        "Array of WireData names for use in a line constants calculation.\nAlternative to individual wire inputs. ALL MUST BE PREVIOUSLY DEFINED.\nMust match \"nconds\" as previously defined for this geometry,\nunless TSData or CNData were previously assigned to phases, and these wires are neutrals.\nMust be used in conjunction with the Spacing property.",
    ),
    ("LineGeometry.x", "x coordinate."),
    (
        "LineSpacing.avgneutralheight",
        "Average height of neutral conductors. Used for equivalent distance modeling (detailed=no) as opposed to detailed cross-section coordinates.",
    ),
    (
        "LineSpacing.avgphaseheight",
        "Average height of phase conductors. Used for equivalent distance modeling (detailed=no) as opposed to detailed cross-section coordinates.",
    ),
    (
        "LineSpacing.detailed",
        "{Yes/True | No/False} Default = Yes. Determines whether the spacing uses a detailed cross-section coordinates with x and h arrays (Yes/True), or uses equivalent spacing fields (No/False). The equivalent spacing fields are EqDistPhPh, EqDistPhN, AvgPhaseHeight and AvgNeutralHeight.",
    ),
    (
        "LineSpacing.eqdistphn",
        "Equivalent distance between phase and neutral conductors (geometric mean distance). Used for equivalent distance modeling (detailed=no) as opposed to detailed cross-section coordinates.",
    ),
    (
        "LineSpacing.eqdistphph",
        "Equivalent distance between phase conductors (geometric mean distance). Used for equivalent distance modeling (detailed=no) as opposed to detailed cross-section coordinates.",
    ),
    ("LineSpacing.h", "Array of wire Heights."),
    (
        "LineSpacing.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "LineSpacing.nconds",
        "Number of wires in this geometry. Default is 3. Triggers memory allocations. Define first!",
    ),
    (
        "LineSpacing.nphases",
        "Number of retained phase conductors. If less than the number of wires, list the retained phase coordinates first.",
    ),
    (
        "LineSpacing.units",
        "Units for x and h: {mi|kft|km|m|Ft|in|cm } Initial default is \"ft\", but defaults to last unit defined",
    ),
    ("LineSpacing.x", "Array of wire X coordinates."),
    (
        "Load.%mean",
        "Percent mean value for load to use for monte carlo studies if no loadshape is assigned to this load. Default is 50.",
    ),
    (
        "Load.%seriesrl",
        "Percent of load that is series R-L for Harmonic studies. Default is 50. Remainder is assumed to be parallel R and L. This can have a significant impact on the amount of damping observed in Harmonics solutions.",
    ),
    (
        "Load.%stddev",
        "Percent Std deviation value for load to use for monte carlo studies if no loadshape is assigned to this load. Default is 10.",
    ),
    (
        "Load.allocationfactor",
        "Default = 0.5.  Allocation factor for allocating loads based on connected kVA at a bus. Side effect:  kW, PF, and kvar are modified by multiplying this factor times the XFKVA (if > 0).",
    ),
    ("Load.basefreq", "Base Frequency for ratings."),
    (
        "Load.bus1",
        "Bus to which the load is connected.  May include specific node specification.",
    ),
    (
        "Load.cfactor",
        "Factor relating average kW to peak kW. Default is 4.0. See kWh and kWhdays. See kVA.",
    ),
    (
        "Load.class",
        "An arbitrary integer number representing the class of load so that load values may be segregated by load value. Default is 1; not used internally.",
    ),
    ("Load.conn", "={wye or LN | delta or LL}.  Default is wye."),
    (
        "Load.cvrcurve",
        "Default is NONE. Curve describing both watt and var factors as a function of time. Refers to a LoadShape object with both Mult and Qmult defined. Define a Loadshape to agree with yearly or daily curve according to the type of analysis being done. If NONE, the CVRwatts and CVRvars factors are used and assumed constant.",
    ),
    (
        "Load.cvrvars",
        "Percent reduction in reactive power (vars) per 1% reduction in voltage from 100% rated. Default=2. \n Typical values range from 2 to 3. Applies to Model=4 only.\n Intended to represent conservation voltage reduction or voltage optimization measures.",
    ),
    (
        "Load.cvrwatts",
        "Percent reduction in active power (watts) per 1% reduction in voltage from 100% rated. Default=1. \n Typical values range from 0.4 to 0.8. Applies to Model=4 only.\n Intended to represent conservation voltage reduction or voltage optimization measures.",
    ),
    (
        "Load.daily",
        "LOADSHAPE object to use for daily simulations.  Must be previously defined as a Loadshape object of 24 hrs, typically. Set Status=Fixed to ignore Loadshape designation. Set to NONE to reset to no loadshape. Default is no variation (constant) if not defined. Side effect: Sets Yearly load shape if not already defined.",
    ),
    (
        "Load.duty",
        "LOADSHAPE object to use for duty cycle simulations.  Must be previously defined as a Loadshape object.  Typically would have time intervals less than 1 hr. Designate the number of points to solve using the Set Number=xxxx command. If there are fewer points in the actual shape, the shape is assumed to repeat.Set to NONE to reset to no loadshape. Set Status=Fixed to ignore Loadshape designation.  Defaults to Daily curve If not specified.",
    ),
    (
        "Load.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "Load.growth",
        "Characteristic  to use for growth factors by years.  Must be previously defined as a Growthshape object. Defaults to circuit default growth factor (see Set Growth command).",
    ),
    (
        "Load.kv",
        "Nominal rated (1.0 per unit) voltage, kV, for load. For 2- and 3-phase loads, specify phase-phase kV. Otherwise, specify actual kV across each branch of the load. If wye (star), specify phase-neutral kV. If delta or phase-phase connected, specify phase-phase kV.",
    ),
    (
        "Load.kva",
        "Specify base Load in kVA (and power factor)\n\nLegal ways to define base load:\nkW, PF\nkW, kvar\nkVA, PF\nXFKVA * Allocationfactor, PF\nkWh/(kWhdays*24) * Cfactor, PF",
    ),
    (
        "Load.kvar",
        "Specify the base kvar for specifying load as kW & kvar.  Assumes kW has been already defined.  Alternative to specifying the power factor.  Side effect:  the power factor and kVA is altered to agree.",
    ),
    (
        "Load.kw",
        "Total base kW for the load.  Normally, you would enter the maximum kW for the load for the first year and allow it to be adjusted by the load shapes, growth shapes, and global load multiplier.\n\nLegal ways to define base load:\nkW, PF\nkW, kvar\nkVA, PF\nXFKVA * Allocationfactor, PF\nkWh/(kWhdays*24) * Cfactor, PF",
    ),
    (
        "Load.kwh",
        "kWh billed for this period. Default is 0. See help on kVA and Cfactor and kWhDays.",
    ),
    (
        "Load.kwhdays",
        "Length of kWh billing period in days (24 hr days). Default is 30. Average demand is computed using this value.",
    ),
    (
        "Load.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Load.model",
        "Integer code for the model to use for load variation with voltage. Valid values are:\n\n1:Standard constant P+jQ load. (Default)\n2:Constant impedance load. \n3:Const P, Quadratic Q (like a motor).\n4:Nominal Linear P, Quadratic Q (feeder mix). Use this with CVRfactor.\n5:Constant Current Magnitude\n6:Const P, Fixed Q\n7:Const P, Fixed Impedance Q\n8:ZIPV (7 values)\n\nFor Types 6 and 7, only the P is modified by load multipliers.",
    ),
    (
        "Load.numcust",
        "Number of customers, this load. Default is 1.",
    ),
    (
        "Load.pf",
        "Load power factor.  Enter negative for leading powerfactor (when kW and kvar have opposite signs.)",
    ),
    (
        "Load.phases",
        "Number of Phases, this load.  Load is evenly divided among phases.",
    ),
    (
        "Load.puxharm",
        "Special reactance, pu (based on kVA, kV properties), for the series impedance branch in the load model for HARMONICS analysis. Generally used to represent motor load blocked rotor reactance. If not specified (that is, set =0, the default value), the series branch is computed from the percentage of the nominal load at fundamental frequency specified by the %SERIESRL property. \n\nApplies to load model in HARMONICS mode only.\n\nA typical value would be approximately 0.20 pu based on kVA * %SeriesRL / 100.0.",
    ),
    (
        "Load.relweight",
        "Relative weighting factor for reliability calcs. Default = 1. Used to designate high priority loads such as hospitals, etc. \n\nIs multiplied by number of customers and load kW during reliability calcs.",
    ),
    (
        "Load.rneut",
        "Default is -1. Neutral resistance of wye (star)-connected load in actual ohms. If entered as a negative value, the neutral can be open, or floating, or it can be connected to node 0 (ground), which is the usual default. If >=0 be sure to explicitly specify the node connection for the neutral, or last, conductor. Otherwise, the neutral impedance will be shorted to ground.",
    ),
    (
        "Load.spectrum",
        "Name of harmonic current spectrum for this load.  Default is \"defaultload\", which is defined when the DSS starts.",
    ),
    (
        "Load.status",
        "={Variable | Fixed | Exempt}.  Default is variable. If Fixed, no load multipliers apply;  however, growth multipliers do apply.  All multipliers apply to Variable loads.  Exempt loads are not modified by the global load multiplier, such as in load duration curves, etc.  Daily multipliers do apply, so setting this property to Exempt is a good way to represent industrial load that stays the same day-after-day for the period study.",
    ),
    (
        "Load.vlowpu",
        "Default = 0.50.  Per unit voltage at which the model switches to same as constant Z model (model=2). This allows more consistent convergence at very low voltaes due to opening switches or solving for fault situations.",
    ),
    (
        "Load.vmaxpu",
        "Default = 1.05.  Maximum per unit voltage for which the MODEL is assumed to apply. Above this value, the load model reverts to a constant impedance model.",
    ),
    (
        "Load.vminemerg",
        "Minimum per unit voltage for load UE evaluations, Emergency limit.  Default = 0, which defaults to system \"vminemerg\" property (see Set Command under Executive).  If this property is specified, it ALWAYS overrides the system specification. This allows you to have different criteria for different loads. Set to zero to revert to the default system value.",
    ),
    (
        "Load.vminnorm",
        "Minimum per unit voltage for load EEN evaluations, Normal limit.  Default = 0, which defaults to system \"vminnorm\" property (see Set Command under Executive).  If this property is specified, it ALWAYS overrides the system specification. This allows you to have different criteria for different loads. Set to zero to revert to the default system value.",
    ),
    (
        "Load.vminpu",
        "Default = 0.95.  Minimum per unit voltage for which the MODEL is assumed to apply. Lower end of normal voltage range.Below this value, the load model reverts to a constant impedance model that matches the model at the transition voltage. See also \"Vlowpu\" which causes the model to match Model=2 below the transition voltage.",
    ),
    (
        "Load.xfkva",
        "Default = 0.0.  Rated kVA of service transformer for allocating loads based on connected kVA at a bus. Side effect:  kW, PF, and kvar are modified. See help on kVA.",
    ),
    (
        "Load.xneut",
        "Neutral reactance of wye(star)-connected load in actual ohms.  May be + or -.",
    ),
    (
        "Load.xrharm",
        "X/R ratio of the special harmonics mode reactance specified by the puXHARM property at fundamental frequency. Default is 6. ",
    ),
    (
        "Load.yearly",
        "LOADSHAPE object to use for yearly simulations.  Must be previously defined as a Loadshape object. Is set to the Daily load shape  when Daily is defined.  The daily load shape is repeated in this case. Set Status=Fixed to ignore Loadshape designation. Set to NONE to reset to no loadshape. The default is no variation.",
    ),
    (
        "Load.zipv",
        "Array of 7 coefficients:\n\n First 3 are ZIP weighting factors for real power (should sum to 1)\n Next 3 are ZIP weighting factors for reactive power (should sum to 1)\n Last 1 is cut-off voltage in p.u. of base kV; load is 0 below this cut-off\n No defaults; all coefficients must be specified if using model=8.",
    ),
    (
        "LoadShape.action",
        "{NORMALIZE | DblSave | SngSave} After defining load curve data, setting action=normalize will modify the multipliers so that the peak is 1.0. The mean and std deviation are recomputed.\n\nSetting action=DblSave or SngSave will cause the present mult and qmult values to be written to either a packed file of double or single. The filename is the loadshape name. The mult array will have a \"_P\" appended on the file name and the qmult array, if it exists, will have \"_Q\" appended.",
    ),
    (
        "LoadShape.csvfile",
        "Switch input of active power load curve data to a CSV text file containing (hour, mult) points, or simply (mult) values for fixed time interval data, one per line. NOTE: This action may reset the number of points to a lower value.",
    ),
    (
        "LoadShape.dblfile",
        "Switch input of active power load curve data to a binary file of doubles containing (hour, mult) points, or simply (mult) values for fixed time interval data, packed one after another. NOTE: This action may reset the number of points to a lower value.",
    ),
    (
        "LoadShape.hour",
        "Array of hour values. Only necessary to define for variable interval data (Interval=0). If you set Interval>0 to denote fixed interval data, DO NOT USE THIS PROPERTY. You can also use the syntax: \nhour = (file=filename)     !for text file one value per line\nhour = (dblfile=filename)  !for packed file of doubles\nhour = (sngfile=filename)  !for packed file of singles ",
    ),
    (
        "LoadShape.interpolation",
        "{AVG* | EDGE} Defines the interpolation method used for connecting distant dots within the load shape.\n\nBy default is AVG (average), which will return a multiplier for missing intervals based on the closest multiplier in time.\nEDGE interpolation keeps the last known value for missing intervals until the next defined multiplier arrives.",
    ),
    (
        "LoadShape.interval",
        "Time interval for fixed interval data, hrs. Default = 1. If Interval = 0 then time data (in hours) may be at either regular or  irregular intervals and time value must be specified using either the Hour property or input files. Then values are interpolated when Interval=0, but not for fixed interval data.  \n\nSee also \"sinterval\" and \"minterval\".",
    ),
    (
        "LoadShape.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "LoadShape.mean",
        "Mean of the active power multipliers.  This is computed on demand the first time a value is needed.  However, you may set it to another value independently. Used for Monte Carlo load simulations.",
    ),
    (
        "LoadShape.memorymapping",
        "{Yes | No* | True | False*} Enables the memory mapping functionality for dealing with large amounts of load shapes. \nBy default is False. Use it to accelerate the model loading when the containing a large number of load shapes.",
    ),
    (
        "LoadShape.minterval",
        "Specify fixed interval in MINUTES. Alternate way to specify Interval property.",
    ),
    (
        "LoadShape.mult",
        "Array of multiplier values for active power (P) or other key value (such as pu V for Vsource). \n\nYou can also use the syntax: \n\nmult = (file=filename)     !for text file one value per line\nmult = (dblfile=filename)  !for packed file of doubles\nmult = (sngfile=filename)  !for packed file of singles \nmult = (file=MyCSVFile.csv, col=3, header=yes)  !for multicolumn CSV files \n\nNote: this property will reset Npts if the  number of values in the files are fewer.\n\nSame as Pmult",
    ),
    (
        "LoadShape.npts",
        "Max number of points to expect in load shape vectors. This gets reset to the number of multiplier values found (in files only) if less than specified.",
    ),
    (
        "LoadShape.pbase",
        "Base P value for normalization. Default is zero, meaning the peak will be used.",
    ),
    (
        "LoadShape.pmax",
        "kW value at the time of max power. Is automatically set upon reading in a loadshape. Use this property to override the value automatically computed or to retrieve the value computed.",
    ),
    ("LoadShape.pmult", "Synonym for \"mult\"."),
    (
        "LoadShape.pqcsvfile",
        "Switch input to a CSV text file containing (active, reactive) power (P, Q) multiplier pairs, one per row. \nIf the interval=0, there should be 3 items on each line: (hour, Pmult, Qmult)",
    ),
    (
        "LoadShape.qbase",
        "Base Q value for normalization. Default is zero, meaning the peak will be used.",
    ),
    (
        "LoadShape.qmax",
        "kvar value at the time of max kW power. Is automatically set upon reading in a loadshape. Use this property to override the value automatically computed or to retrieve the value computed.",
    ),
    (
        "LoadShape.qmult",
        "Array of multiplier values for reactive power (Q).  You can also use the syntax: \nqmult = (file=filename)     !for text file one value per line\nqmult = (dblfile=filename)  !for packed file of doubles\nqmult = (sngfile=filename)  !for packed file of singles \nqmult = (file=MyCSVFile.csv, col=4, header=yes)  !for multicolumn CSV files ",
    ),
    (
        "LoadShape.sinterval",
        "Specify fixed interval in SECONDS. Alternate way to specify Interval property.",
    ),
    (
        "LoadShape.sngfile",
        "Switch input of active power load curve data to a binary file of singles containing (hour, mult) points, or simply (mult) values for fixed time interval data, packed one after another. NOTE: This action may reset the number of points to a lower value.",
    ),
    (
        "LoadShape.stddev",
        "Standard deviation of active power multipliers.  This is computed on demand the first time a value is needed.  However, you may set it to another value independently.Is overwritten if you subsequently read in a curve\n\nUsed for Monte Carlo load simulations.",
    ),
    (
        "LoadShape.useactual",
        "{Yes | No* | True | False*} If true, signifies to Load, Generator, Vsource, or other objects to use the return value as the actual kW, kvar, kV, or other value rather than a multiplier. Nominally for AMI Load data but may be used for other functions.",
    ),
    (
        "Monitor.action",
        "{Clear | Save | Take | Process}\n(C)lears or (S)aves current buffer.\n(T)ake action takes a sample.\n(P)rocesses the data taken so far (e.g. Pst for mode 4).\n\nNote that monitors are automatically reset (cleared) when the Set Mode= command is issued. Otherwise, the user must explicitly reset all monitors (reset monitors command) or individual monitors with the Clear action.",
    ),
    ("Monitor.basefreq", "Base Frequency for ratings."),
    (
        "Monitor.element",
        "Name (Full Object name) of element to which the monitor is connected.",
    ),
    (
        "Monitor.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "Monitor.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Monitor.mode",
        "Bitmask integer designating the values the monitor is to capture: \n0 = Voltages and currents at designated terminal\n1 = Powers at designated terminal\n2 = Tap Position (Transformer Device only)\n3 = State Variables (PCElements only)\n4 = Flicker level and severity index (Pst) for voltages. No adders apply.\n    Flicker level at simulation time step, Pst at 10-minute time step.\n5 = Solution variables (Iterations, etc).\nNormally, these would be actual phasor quantities from solution.\n6 = Capacitor Switching (Capacitors only)\n7 = Storage state vars (Storage device only)\n8 = All winding currents (Transformer device only)\n9 = Losses, watts and var (of monitored device)\n10 = All Winding voltages (Transformer device only)\nNormally, these would be actual phasor quantities from solution.\n11 = All terminal node voltages and line currents of monitored device\n12 = All terminal node voltages LL and line currents of monitored device\nCombine mode with adders below to achieve other results for terminal quantities:\n+16 = Sequence quantities\n+32 = Magnitude only\n+64 = Positive sequence only or avg of all phases\n\nMix adder to obtain desired results. For example:\nMode=112 will save positive sequence voltage and current magnitudes only\nMode=48 will save all sequence voltages and currents, but magnitude only.",
    ),
    (
        "Monitor.ppolar",
        "{Yes/True | No/False} Default = YES. Report power in Apparent power, S, in polar form (Mag/Angle).(default)  Otherwise, is P and Q",
    ),
    (
        "Monitor.residual",
        "{Yes/True | No/False} Default = No.  Include Residual cbannel (sum of all phases) for voltage and current. Does not apply to sequence quantity modes or power modes.",
    ),
    (
        "Monitor.terminal",
        "Number of the terminal of the circuit element to which the monitor is connected. 1 or 2, typically. For monitoring states, attach monitor to terminal 1.",
    ),
    (
        "Monitor.vipolar",
        "{Yes/True | No/False} Default = YES. Report voltage and current in polar form (Mag/Angle). (default)  Otherwise, it will be real and imaginary.",
    ),
    (
        "PVSystem.%cutin",
        "% cut-in power -- % of kVA rating of inverter. When the inverter is OFF, the power from the array must be greater than this for the inverter to turn on.",
    ),
    (
        "PVSystem.%cutout",
        "% cut-out power -- % of kVA rating of inverter. When the inverter is ON, the inverter turns OFF when the power from the array drops below this value.",
    ),
    (
        "PVSystem.%pminkvarmax",
        "Minimum active power as percentage of Pmpp that allows the inverter to produce/absorb reactive power up to its kvarMax or kvarMaxAbs.",
    ),
    (
        "PVSystem.%pminnovars",
        "Minimum active power as percentage of Pmpp under which there is no vars production/absorption.",
    ),
    (
        "PVSystem.%pmpp",
        "Upper limit on active power as a percentage of Pmpp.",
    ),
    (
        "PVSystem.%r",
        "Equivalent percent internal resistance, ohms. Default is 50%. Placed in series with internal voltage source for harmonics and dynamics modes. (Limits fault current to about 2 pu if not current limited -- see LimitCurrent) ",
    ),
    (
        "PVSystem.%x",
        "Equivalent percent internal reactance, ohms. Default is 0%. Placed in series with internal voltage source for harmonics and dynamics modes. ",
    ),
    (
        "PVSystem.amplimit",
        "The current limiter per phase for the IBR when operating in GFM mode. This limit is imposed to prevent the IBR to enter into Safe Mode when reaching the IBR power ratings.\nOnce the IBR reaches this value, it remains there without moving into Safe Mode. This value needs to be set lower than the IBR Amps rating.",
    ),
    (
        "PVSystem.amplimitgain",
        "Use it for fine tunning the current limiter when active, by default is 0.8, it has to be a value between 0.1 and 1. This value allows users to fine tune the IBRs current limiter to match with the user requirements.",
    ),
    (
        "PVSystem.balanced",
        "{Yes | No*} Default is No.  Force balanced current only for 3-phase PVSystems. Forces zero- and negative-sequence to zero. ",
    ),
    ("PVSystem.basefreq", "Base Frequency for ratings."),
    (
        "PVSystem.bus1",
        "Bus to which the PVSystem element is connected.  May include specific node specification.",
    ),
    (
        "PVSystem.class",
        "An arbitrary integer number representing the class of PVSystem element so that PVSystem values may be segregated by class.",
    ),
    ("PVSystem.conn", "={wye|LN|delta|LL}.  Default is wye."),
    (
        "PVSystem.controlmode",
        "Defines the control mode for the inverter. It can be one of {GFM | GFL*}. By default it is GFL (Grid Following Inverter). Use GFM (Grid Forming Inverter) for energizing islanded microgrids, but, if the device is connected to the grid, it is highly recommended to use GFL.\n\nGFM control mode disables any control action set by the InvControl device.",
    ),
    (
        "PVSystem.daily",
        "Dispatch shape to use for daily simulations.  Must be previously defined as a Loadshape object of 24 hrs, typically.  In the default dispatch mode, the PVSystem element uses this loadshape to trigger State changes.",
    ),
    (
        "PVSystem.debugtrace",
        "{Yes | No }  Default is no.  Turn this on to capture the progress of the PVSystem model for each iteration.  Creates a separate file for each PVSystem element named \"PVSystem_name.csv\".",
    ),
    (
        "PVSystem.duty",
        "Load shape to use for duty cycle dispatch simulations such as for solar ramp rate studies. Must be previously defined as a Loadshape object. Typically would have time intervals of 1-5 seconds. Designate the number of points to solve using the Set Number=xxxx command. If there are fewer points in the actual shape, the shape is assumed to repeat.",
    ),
    (
        "PVSystem.dutystart",
        "Starting time offset [hours] into the duty cycle shape for this PVSystem, defaults to 0",
    ),
    (
        "PVSystem.dynamiceq",
        "The name of the dynamic equation (DynamicExp) that will be used for defining the dynamic behavior of the generator. If not defined, the generator dynamics will follow the built-in dynamic equation.",
    ),
    (
        "PVSystem.dynout",
        "The name of the variables within the Dynamic equation that will be used to govern the PVSystem dynamics. This PVsystem model requires 1 output from the dynamic equation:\n\n    1. Current.\n\nThe output variables need to be defined in the same order.",
    ),
    (
        "PVSystem.effcurve",
        "An XYCurve object, previously defined, that describes the PER UNIT efficiency vs PER UNIT of rated kVA for the inverter. Inverter output power is discounted by the multiplier obtained from this curve.",
    ),
    (
        "PVSystem.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "PVSystem.irradiance",
        "Get/set the present irradiance value in kW/sq-m. Used as base value for shape multipliers. Generally entered as peak value for the time period of interest and the yearly, daily, and duty load shape objects are defined as per unit multipliers (just like Loads/Generators).",
    ),
    (
        "PVSystem.kp",
        "It is the proportional gain for the PI controller within the inverter. Use it to modify the controller response in dynamics simulation mode.",
    ),
    (
        "PVSystem.kv",
        "Nominal rated (1.0 per unit) voltage, kV, for PVSystem element. For 2- and 3-phase PVSystem elements, specify phase-phase kV. Otherwise, specify actual kV across each branch of the PVSystem element. If 1-phase wye (star or LN), specify phase-neutral kV. If 1-phase delta or phase-phase connected, specify phase-phase kV.",
    ),
    (
        "PVSystem.kva",
        "kVA rating of inverter. Used as the base for Dynamics mode and Harmonics mode values.",
    ),
    (
        "PVSystem.kvar",
        "Get/set the present kvar value.  Setting this property forces the inverter to operate in constant kvar mode.",
    ),
    (
        "PVSystem.kvarmax",
        "Indicates the maximum reactive power GENERATION (un-signed numerical variable in kvar) for the inverter (as an un-signed value). Defaults to kVA rating of the inverter.",
    ),
    (
        "PVSystem.kvarmaxabs",
        "Indicates the maximum reactive power ABSORPTION (un-signed numerical variable in kvar) for the inverter (as an un-signed value). Defaults to kVA rating of the inverter.",
    ),
    (
        "PVSystem.kvdc",
        "Indicates the rated voltage (kV) at the input of the inverter at the peak of PV energy production. The value is normally greater or equal to the kV base of the PV system. It is used for dynamics simulation ONLY.",
    ),
    (
        "PVSystem.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "PVSystem.limitcurrent",
        "Limits current magnitude to Vminpu value for both 1-phase and 3-phase PVSystems similar to Generator Model 7. For 3-phase, limits the positive-sequence current but not the negative-sequence.",
    ),
    (
        "PVSystem.model",
        "Integer code (default=1) for the model to use for power output variation with voltage. Valid values are:\n\n1:PVSystem element injects a CONSTANT kW at specified power factor.\n2:PVSystem element is modeled as a CONSTANT ADMITTANCE.\n3:Compute load injection from User-written Model.",
    ),
    (
        "PVSystem.p-tcurve",
        "An XYCurve object, previously defined, that describes the PV array PER UNIT Pmpp vs Temperature curve. Temperature units must agree with the Temperature property and the Temperature shapes used for simulations. The Pmpp values are specified in per unit of the Pmpp value for 1 kW/sq-m irradiance. The value for the temperature at which Pmpp is defined should be 1.0. The net array power is determined by the irradiance * Pmpp * f(Temperature)",
    ),
    (
        "PVSystem.pf",
        "Nominally, the power factor for the output power. Default is 1.0. Setting this property will cause the inverter to operate in constant power factor mode.Enter negative when kW and kvar have opposite signs.\nA positive power factor signifies that the PVSystem element produces vars \nas is typical for a generator.  ",
    ),
    (
        "PVSystem.pfpriority",
        "{Yes/No*/True/False} Set inverter to operate with PF priority when in constant PF mode. If \"Yes\", value assigned to \"WattPriority\" is neglected. If controlled by an InvControl with either Volt-Var or DRC or both functions activated, PF priority is neglected and \"WattPriority\" is considered. Default = No.",
    ),
    (
        "PVSystem.phases",
        "Number of Phases, this PVSystem element.  Power is evenly divided among phases.",
    ),
    (
        "PVSystem.pitol",
        "It is the tolerance (%) for the closed loop controller of the inverter. For dynamics simulation mode.",
    ),
    (
        "PVSystem.pmpp",
        "Get/set the rated max power of the PV array for 1.0 kW/sq-m irradiance and a user-selected array temperature. The P-TCurve should be defined relative to the selected array temperature.",
    ),
    (
        "PVSystem.safemode",
        "(Read only) Indicates whether the inverter entered (Yes) or not (No) into Safe Mode.",
    ),
    (
        "PVSystem.safevoltage",
        "Indicates the voltage level (%) respect to the base voltage level for which the Inverter will operate. If this threshold is violated, the Inverter will enter safe mode (OFF). For dynamic simulation. By default is 80%",
    ),
    (
        "PVSystem.spectrum",
        "Name of harmonic voltage or current spectrum for this PVSystem element. A harmonic voltage source is assumed for the inverter. Default value is \"default\", which is defined when the DSS starts.",
    ),
    (
        "PVSystem.tdaily",
        "Temperature shape to use for daily simulations.  Must be previously defined as a TShape object of 24 hrs, typically.  The PVSystem element uses this TShape to determine the Pmpp from the Pmpp vs T curve. Units must agree with the Pmpp vs T curve.",
    ),
    (
        "PVSystem.tduty",
        "Temperature shape to use for duty cycle dispatch simulations such as for solar ramp rate studies. Must be previously defined as a TShape object. Typically would have time intervals of 1-5 seconds. Designate the number of points to solve using the Set Number=xxxx command. If there are fewer points in the actual shape, the shape is assumed to repeat. The PVSystem model uses this TShape to determine the Pmpp from the Pmpp vs T curve. Units must agree with the Pmpp vs T curve.",
    ),
    (
        "PVSystem.temperature",
        "Get/set the present Temperature. Used as fixed value corresponding to PTCurve property. A multiplier is obtained from the Pmpp-Temp curve and applied to the nominal Pmpp from the irradiance to determine the net array output.",
    ),
    (
        "PVSystem.tyearly",
        "Temperature shape to use for yearly simulations.  Must be previously defined as a TShape object. If this is not specified, the Daily dispatch shape, if any, is repeated during Yearly solution modes. The PVSystem element uses this TShape to determine the Pmpp from the Pmpp vs T curve. Units must agree with the Pmpp vs T curve.",
    ),
    (
        "PVSystem.userdata",
        "String (in quotes or parentheses) that gets passed to user-written model for defining the data required for that model.",
    ),
    (
        "PVSystem.usermodel",
        "Name of DLL containing user-written model, which computes the terminal currents for Dynamics studies, overriding the default model.  Set to \"none\" to negate previous setting.",
    ),
    (
        "PVSystem.varfollowinverter",
        "Boolean variable (Yes|No) or (True|False). Defaults to False which indicates that the reactive power generation/absorption does not respect the inverter status.When set to True, the PVSystem reactive power generation/absorption will cease when the inverter status is off, due to panel kW dropping below %Cutout.  The reactive power generation/absorption will begin again when the panel kW is above %Cutin.  When set to False, the PVSystem will generate/absorb reactive power regardless of the status of the inverter.",
    ),
    (
        "PVSystem.vmaxpu",
        "Default = 1.10.  Maximum per unit voltage for which the Model is assumed to apply. Above this value, the load model reverts to a constant impedance model.",
    ),
    (
        "PVSystem.vminpu",
        "Default = 0.90.  Minimum per unit voltage for which the Model is assumed to apply. Below this value, the load model reverts to a constant impedance model except for Dynamics model. In Dynamics mode, the current magnitude is limited to the value the power flow would compute for this voltage.",
    ),
    (
        "PVSystem.wattpriority",
        "{Yes/No*/True/False} Set inverter to watt priority instead of the default var priority",
    ),
    (
        "PVSystem.yearly",
        "Dispatch shape to use for yearly simulations.  Must be previously defined as a Loadshape object. If this is not specified, the Daily dispatch shape, if any, is repeated during Yearly solution modes. In the default dispatch mode, the PVSystem element uses this loadshape to trigger State changes.",
    ),
    (
        "PlotOption.1phlinestyle",
        "Line style for drawing 1-phase lines. A number in the range of [1..7].Default is 1 (solid). Use 3 for dotted; 2 for dashed.",
    ),
    (
        "PlotOption.3phlinestyle",
        "Line style for drawing 3-phase lines. A number in the range of [1..7].Default is 1 (solid). Use 3 for dotted; 2 for dashed.",
    ),
    (
        "PlotOption.bases",
        "Array of base values for each channel for monitor plot. Useful for creating per unit plots. Default is 1.0 for each channel.  Set Base= property after defining channels.\n\nPlot Type=Monitor Object=MyMonitor Channels=[1, 3, 5] Bases=[2400 2400 2400]\n\nDo \"Show Monitor MyMonitor\" to see channel range and definitions.",
    ),
    (
        "PlotOption.buslist",
        "{Array of Bus Names | File=filename } This is for the Daisy plot. \n\nPlot daisy power max=5000 dots=N Buslist=[file=MyBusList.txt]\n\nA \"daisy\" marker is plotted for each bus in the list. Bus name may be repeated, which results in multiple markers distributed around the bus location. This gives the appearance of a daisy if there are several symbols at a bus. Not needed for plotting active generators.",
    ),
    (
        "PlotOption.c1",
        "RGB color number or standard color name for color C1. This is the default color for circuit plots. Default is blue. See options in the Plot menu.\n\nStandard color names are: \n\n Black  \n Maroon \n Green  \n Olive  \n Navy   \n Purple \n Teal   \n Gray   \n Silver \n Red    \n Lime   \n Yellow \n Blue   \n Fuchsia\n Aqua   \n LtGray \n DkGray \n White  ",
    ),
    (
        "PlotOption.c2",
        "RGB color number or standard color name for color C2. Used for gradients and tricolor plots such as circuit voltage.\n\nSee Help on C1 for list of standard color names.",
    ),
    (
        "PlotOption.c3",
        "RGB color number or standard color name for color C3. Used for gradients and tricolor plots such a circuit voltage.\n\nSee Help on C1 for list of standard color names.",
    ),
    (
        "PlotOption.channels",
        "Array of channel numbers for monitor plot. Example\n\nPlot Type=Monitor Object=MyMonitor Channels=[1, 3, 5]\n\nDo \"Show Monitor MyMonitor\" to see channel definitions.",
    ),
    (
        "PlotOption.dots",
        "Yes or No*. Places a marker on the circuit plot at the bus location. See Set Markercode under options.",
    ),
    (
        "PlotOption.labels",
        "Yes or No*. If yes, bus labels (abbreviated) are printed on the circuit plot.",
    ),
    (
        "PlotOption.max",
        "Enter 0 (the default value) or the value corresponding to max scale or line thickness in the circuit plots. Power and Losses in kW. Also, use this to specify the max value corresponding to color C2 in General plots.",
    ),
    (
        "PlotOption.min",
        "Enter 0 (the default value) or the value corresponding to min value corresponding to color C1 in General bus data plots.",
    ),
    (
        "PlotOption.object",
        "Object to be plotted. One of [Meter Name (zones plot) | Monitor Name | LoadShape Name | File Name for General bus data | File Name Circuit branch data]",
    ),
    (
        "PlotOption.phases",
        "{default* | ALL | PRIMARY | LL3ph | LLALL | LLPRIMARY | (phase number)} For Profile plot. Specify which phases you want plotted.\n\ndefault = plot only nodes 1-3 at 3-phase buses (default)\nALL = plot all nodes\nPRIMARY = plot all nodes -- primary only (voltage > 1kV)\nLL3ph = 3-ph buses only -- L-L voltages)\nLLALL = plot all nodes -- L-L voltages)\nLLPRIMARY = plot all nodes -- L-L voltages primary only)\n(phase number) = plot all nodes on selected phase\n\nNote: Only nodes downline from an energy meter are plotted.",
    ),
    (
        "PlotOption.plotid",
        "Plot identifier for dynamic updates of \"profile\" and \"scatter\" plots in the OpenDSS Viewer (See \"plot type\" for more details).When multiple \"plot\" commands are executed with the same PlotID, the same figure will be updated with the most recent simulation results.\n\nThis identifier could be declared as an integer number or a string without spaces.\n\nExample:\n\nset OpenDSSViewer=true ! OpenDSS Viewer enabled\nsolve\nplot scatter PlotID=plotA  !Generates a new scatter plot\nsolve\nplot scatter PlotID=plotB  !Generates a new scatter plot\nsolve\nplot scatter PlotID=plotA  !Updates the data in plotA\nsolve\nplot scatter PlotID=plotA  !Updates the data in plotA",
    ),
    (
        "PlotOption.profilescale",
        "PUKM | 120KFT, default is PUKM\nPUKM = per-unit voltage vs. distance in km\n120KFT = voltage on 120-V base vs. distance in kft.",
    ),
    (
        "PlotOption.quantity",
        "One of {Voltage | Current | Power | Losses | Capacity | (Value Index for General, AutoAdd, or Circuit[w/ file]) }",
    ),
    (
        "PlotOption.r2",
        "pu value for tri-color plot mid range [default=.50 of max scale]. Corresponds to color C2.",
    ),
    (
        "PlotOption.r3",
        "pu value for tri-color plot max range [default=.85 of max scale]. Corresponds to color C3.",
    ),
    (
        "PlotOption.showloops",
        "{Yes | No*} Shows loops on Circuit plot. Requires an EnergyMeter to be defined.",
    ),
    (
        "PlotOption.subs",
        "{Yes | No*} Displays a marker at each transformer declared to be a substation. At least one bus coordinate must be defined for the transformer. See MarkTransformer and TransMarkerCode options.",
    ),
    (
        "PlotOption.thickness",
        "Max thickness allowed for lines in circuit plots (default=7).",
    ),
    (
        "PlotOption.type",
        "One of {Circuit | Monitor | Daisy | Zones | AutoAdd | General (bus data) | Loadshape | Tshape | Priceshape |Profile} \nA \"Daisy\" plot is a special circuit plot that places a marker at each Generator location or at buses in the BusList property, if defined. A Zones plot shows the meter zones (see help on Object). Autoadd shows the autoadded generators. General plot shows quantities associated with buses using gradient colors between C1 and C2. Values are read from a file (see Object). Loadshape plots the specified loadshape. Examples:\n\nPlot type=circuit quantity=power\nPlot Circuit Losses 1phlinestyle=3\nPlot Circuit quantity=3 object=mybranchdata.csv\nPlot daisy power max=5000 dots=N Buslist=[file=MyBusList.txt]\nPlot General quantity=1 object=mybusdata.csv\nPlot Loadshape object=myloadshape\nPlot Tshape object=mytemperatureshape\nPlot Priceshape object=mypriceshape\nPlot Profile\nPlot Profile Phases=Primary\n\nAdditional plots with the OpenDSS Viewer (These plots are enabled with the \"OpenDSSViewer\" option):\n- Plot evolution  ! Probabilistic density evolution plot with the line-to-ground magnitude of all load voltages in per unit base.\n- Plot energymeter object=system  ! System energy meter plot. The \"DemandInterval\" option is required.\n- Plot energymeter object=Totals  ! Totals energy meter plot. The \"DemandInterval\" option is required.\n- Plot energymeter object=voltexception  ! Voltage exception plot. The \"DemandInterval\" and \"VoltExceptionReport\" options are required.\n- Plot energymeter object=overloads  ! Overload report plot. The \"DemandInterval\" and \"OverloadReport\" options are required.\n- Plot energymeter object=myMeter  ! Energy meter plot. The \"DemandInterval\" and \"DIVerbose\" options are required.\n- Plot loadshape object=myLoadshape  ! Loadshapes with the OpenDSS Viewer functionalities.\n- Plot matrix incidence  ! Incidence matrix plot (Requires: CalcIncMatrix or CalcIncMatrix_O).\n- Plot matrix laplacian  ! Laplacian matrix plot (Requires: CalcLaplacian).\n- Plot monitor object=myMonitor  ! Monitors with the OpenDSS Viewer functionalities. All channels are included in this plot.\n- Plot phasevoltage object=myMeter  ! Phase voltage plot associated to an energy meter. The \"DemandInterval\", \"DIVerbose\" options and the \"PhaseVoltageReport\" parameter are required.\n- Plot profile  ! 3D and 2D versions of the voltage profile.\n- Plot scatter  ! Scatter plot with geovisualization of line-to-ground bus voltage magnitudes in per unit.",
    ),
    (
        "PriceShape.action",
        "{DblSave | SngSave} After defining Price curve data... Setting action=DblSave or SngSave will cause the present \"Price\" values to be written to either a packed file of double or single. The filename is the PriceShape name. ",
    ),
    (
        "PriceShape.csvfile",
        "Switch input of  Price curve data to a csv file containing (hour, Price) points, or simply (Price) values for fixed time interval data, one per line. NOTE: This action may reset the number of points to a lower value.",
    ),
    (
        "PriceShape.dblfile",
        "Switch input of  Price curve data to a binary file of doubles containing (hour, Price) points, or simply (Price) values for fixed time interval data, packed one after another. NOTE: This action may reset the number of points to a lower value.",
    ),
    (
        "PriceShape.hour",
        "Array of hour values. Only necessary to define this property for variable interval data. If the data are fixed interval, do not use this property. You can also use the syntax: \nhour = (file=filename)     !for text file one value per line\nhour = (dblfile=filename)  !for packed file of doubles\nhour = (sngfile=filename)  !for packed file of singles ",
    ),
    (
        "PriceShape.interval",
        "Time interval for fixed interval data, hrs. Default = 1. If Interval = 0 then time data (in hours) may be at irregular intervals and time value must be specified using either the Hour property or input files. Then values are interpolated when Interval=0, but not for fixed interval data.  \n\nSee also \"sinterval\" and \"minterval\".",
    ),
    (
        "PriceShape.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "PriceShape.mean",
        "Mean of the Price curve values.  This is computed on demand the first time a value is needed.  However, you may set it to another value independently. Used for Monte Carlo load simulations.",
    ),
    (
        "PriceShape.minterval",
        "Specify fixed interval in MINUTES. Alternate way to specify Interval property.",
    ),
    (
        "PriceShape.npts",
        "Max number of points to expect in price shape vectors. This gets reset to the number of Price values found if less than specified.",
    ),
    (
        "PriceShape.price",
        "Array of Price values.  Units should be compatible with the object using the data. You can also use the syntax: \nPrice = (file=filename)     !for text file one value per line\nPrice = (dblfile=filename)  !for packed file of doubles\nPrice = (sngfile=filename)  !for packed file of singles \n\nNote: this property will reset Npts if the  number of values in the files are fewer.",
    ),
    (
        "PriceShape.sinterval",
        "Specify fixed interval in SECONDS. Alternate way to specify Interval property.",
    ),
    (
        "PriceShape.sngfile",
        "Switch input of  Price curve data to a binary file of singles containing (hour, Price) points, or simply (Price) values for fixed time interval data, packed one after another. NOTE: This action may reset the number of points to a lower value.",
    ),
    (
        "PriceShape.stddev",
        "Standard deviation of the Prices.  This is computed on demand the first time a value is needed.  However, you may set it to another value independently.Is overwritten if you subsequently read in a curve\n\nUsed for Monte Carlo load simulations.",
    ),
    ("Reactor.basefreq", "Base Frequency for ratings."),
    (
        "Reactor.bus1",
        "Name of first bus. Examples:\nbus1=busname\nbus1=busname.1.2.3\n\nBus2 property will default to this bus, node 0, unless previously specified. Only Bus1 need be specified for a Yg shunt reactor.",
    ),
    (
        "Reactor.bus2",
        "Name of 2nd bus. Defaults to all phases connected to first bus, node 0, (Shunt Wye Connection) except when Bus2 is specifically defined.\n\nNot necessary to specify for delta (LL) connection",
    ),
    (
        "Reactor.conn",
        "={wye | delta |LN |LL}  Default is wye, which is equivalent to LN. If Delta, then only one terminal.",
    ),
    (
        "Reactor.emergamps",
        "Maximum or emerg current. Defaults to 135% of per-phase rated current when reactor is specified with rated power and voltage.",
    ),
    (
        "Reactor.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    ("Reactor.faultrate", "Failure rate per year."),
    (
        "Reactor.kv",
        "For 2, 3-phase, kV phase-phase. Otherwise specify actual coil rating.",
    ),
    (
        "Reactor.kvar",
        "Total kvar, all phases.  Evenly divided among phases. Only determines X. Specify R separately",
    ),
    (
        "Reactor.lcurve",
        "Name of XYCurve object, previously defined, describing per-unit variation of phase inductance, L=X/w, vs. frequency. Applies to reactance specified by X, LmH, Z, or kvar property.L generally decreases somewhat with frequency above the base frequency, approaching a limit at a few kHz.",
    ),
    (
        "Reactor.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Reactor.lmh",
        "Inductance, mH. Alternate way to define the reactance, X, property.",
    ),
    (
        "Reactor.normamps",
        "Normal rated current. Defaults to per-phase rated current when reactor is specified with rated power and voltage.",
    ),
    (
        "Reactor.parallel",
        "{Yes | No}  Default=No. Indicates whether Rmatrix and Xmatrix are to be considered in parallel. Default is series. For other models, specify R and Rp.",
    ),
    (
        "Reactor.pctperm",
        "Percent of failures that become permanent.",
    ),
    ("Reactor.phases", "Number of phases."),
    (
        "Reactor.r",
        "Resistance (in series with reactance), each phase, ohms. This property applies to REACTOR specified by either kvar or X. See also help on Z.",
    ),
    (
        "Reactor.rcurve",
        "Name of XYCurve object, previously defined, describing per-unit variation of phase resistance, R, vs. frequency. Applies to resistance specified by R or Z property. If actual values are not known, R often increases by approximately the square root of frequency.",
    ),
    ("Reactor.repair", "Hours to repair."),
    (
        "Reactor.rmatrix",
        "Resistance matrix, lower triangle, ohms at base frequency. Order of the matrix is the number of phases. Mutually exclusive to specifying parameters by kvar or X.",
    ),
    (
        "Reactor.rp",
        "Resistance in parallel with R and X (the entire branch). Assumed infinite if not specified.",
    ),
    (
        "Reactor.x",
        "Reactance, each phase, ohms at base frequency. See also help on Z and LmH properties.",
    ),
    (
        "Reactor.xmatrix",
        "Reactance matrix, lower triangle, ohms at base frequency. Order of the matrix is the number of phases. Mutually exclusive to specifying parameters by kvar or X.",
    ),
    (
        "Reactor.z",
        "Alternative way of defining R and X properties. Enter a 2-element array representing R +jX in ohms. Example:\n\nZ=[5  10]   ! equivalent to R=5  X=10 ",
    ),
    (
        "Reactor.z0",
        "Zer0-sequence impedance, ohms, as a 2-element array representing a complex number. Example: \n\nZ0=[3, 4]  ! represents 3 + j4 \n\nUsed to define the impedance matrix of the REACTOR if Z1 is also specified. \n\nNote: Z0 defaults to Z1 if it is not specifically defined. ",
    ),
    (
        "Reactor.z1",
        "Positive-sequence impedance, ohms, as a 2-element array representing a complex number. Example: \n\nZ1=[1, 2]  ! represents 1 + j2 \n\nIf defined, Z1, Z2, and Z0 are used to define the impedance matrix of the REACTOR. Z1 MUST BE DEFINED TO USE THIS OPTION FOR DEFINING THE MATRIX.\n\nSide Effect: Sets Z2 and Z0 to same values unless they were previously defined.",
    ),
    (
        "Reactor.z2",
        "Negative-sequence impedance, ohms, as a 2-element array representing a complex number. Example: \n\nZ2=[1, 2]  ! represents 1 + j2 \n\nUsed to define the impedance matrix of the REACTOR if Z1 is also specified. \n\nNote: Z2 defaults to Z1 if it is not specifically defined. If Z2 is not equal to Z1, the impedance matrix is asymmetrical.",
    ),
    ("Recloser.action", "DEPRECATED. See \"State\" property"),
    ("Recloser.basefreq", "Base Frequency for ratings."),
    (
        "Recloser.debugtrace",
        "{Yes/True* | No/False} Default is No for Recloser. Write extra details to Eventlog.",
    ),
    (
        "Recloser.delay",
        "DEPRECATED. See \"MechanicalDelay\" property.",
    ),
    (
        "Recloser.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "Recloser.eventlog",
        "{Yes/True* | No/False} Default is Yes for Recloser. Write trips, reclose and reset events to EventLog.",
    ),
    (
        "Recloser.gndfastcurve",
        "Name of the TCC Curve object that determines the Ground Fast trip.  Must have been previously defined as a TCC_Curve object or specified as \"none\" (ignored). Default is \"none\". Multiplying the current values in the curve by the \"GndFastPickup\" value gives the actual current.",
    ),
    (
        "Recloser.gndfastpickup",
        "Multiplier for the ground fast TCC curve. Defaults to 1.0.",
    ),
    (
        "Recloser.gndinst",
        "Actual amps for instantaneous ground trip which is assumed to happen in 0.01 sec + Mechanical Delay Time. Default is 0.0, which signifies no inst trip.",
    ),
    (
        "Recloser.gndslowcurve",
        "Name of the TCC Curve object that determines the Ground Slow trip.  Must have been previously defined as a TCC_Curve object or specified as \"none\" (ignored). Default is \"none\". Multiplying the current values in the curve by the \"GndSlowPickup\" value gives the actual current.",
    ),
    (
        "Recloser.gndslowpickup",
        "Multiplier for the ground slow TCC curve. Defaults to 1.0.",
    ),
    (
        "Recloser.grounddelayed",
        "DEPRECATED. See \"GndSlowCurve\" property.",
    ),
    (
        "Recloser.groundfast",
        "DEPRECATED. See \"GndFastCurve\" property.",
    ),
    (
        "Recloser.groundinst",
        "DEPRECATED. See \"GndInst\" property.",
    ),
    (
        "Recloser.groundtrip",
        "DEPRECATED. Assigned value is specified to \"GndPickupFast\" and \"GndPickupSlow\" properties for backwards compatibility. See \"GndPickupFast\" and \"GndPickupSlow\" properties.",
    ),
    (
        "Recloser.interruptingrating",
        "Recloser rated interrupting current in Amps. Defaults to 0. Not used internally for either power flow or reporting.",
    ),
    (
        "Recloser.like",
        "Make like another object, e.g.:

New Capacitor.C2 like=c1  ...",
    ),
    (
        "Recloser.lock",
        "{Yes | No*} Controlled switch is locked in its present open / closed state or unlocked. When locked, the recloser will not respond to either a manual state change issued by the user or a state change issued internally by OpenDSS when reseting the control. Note this locking mechanism is different from the recloser automatic lockout after specifed number of shots.",
    ),
    (
        "Recloser.mechanicaldelay",
        "Fixed delay time (sec) added to Recloser trip time. Default is 0.0. Used to represent breaker time or any other delay.",
    ),
    (
        "Recloser.monitoredobj",
        "Full object name of the circuit element, typically a line, transformer, load, or generator, to which the Recloser's PT and/or CT are connected. This is the \"monitored\" element. There is no default; must be specified.",
    ),
    (
        "Recloser.monitoredterm",
        "Number of the terminal of the circuit element to which the Recloser is connected. 1 or 2, typically.  Default is 1.",
    ),
    (
        "Recloser.normal",
        "ARRAY of strings {Open | Closed} representing the Normal state of the recloser in each phase of the controlled element. The recloser reverts to this state for reset, change of mode, etc. Defaults to \"State\" if not specifically declared.  Setting this property to {Open | Closed} sets the normal state to the specified value for all phases (ganged operation).",
    ),
    (
        "Recloser.numfast",
        "Number of Fast (fuse saving) operations.  Default is 1. (See \"Shots\")",
    ),
    (
        "Recloser.phasedelayed",
        "DEPRECATED. See \"PhSlowCurve\" property.",
    ),
    (
        "Recloser.phasefast",
        "DEPRECATED. See \"PhFastCurve\" property.",
    ),
    ("Recloser.phaseinst", "DEPRECATED. See \"PhInst\" property."),
    (
        "Recloser.phasetrip",
        "DEPRECATED. Assigned value is specified to \"PhPickupFast\" and \"PhPickupSlow\" properties for backwards compatibility. See \"PhPickupFast\" and \"PhPickupSlow\" properties.",
    ),
    (
        "Recloser.phfastcurve",
        "Name of the TCC Curve object that determines the Phase Fast trip. Must have been previously defined as a TCC_Curve object or specified as \"none\" (ignored). Default is \"none\". Multiplying the current values in the curve by the \"PhFastPickup\" value gives the actual current.",
    ),
    (
        "Recloser.phfastpickup",
        "Multiplier for the phase fast TCC curve. Defaults to 1.0.",
    ),
    (
        "Recloser.phinst",
        "Actual amps for instantaneous phase trip which is assumed to happen in 0.01 sec + Mechanical Delay Time. Default is 0.0, which signifies no inst trip.",
    ),
    (
        "Recloser.phslowcurve",
        "Name of the TCC Curve object that determines the Phase Slow trip. Must have been previously defined as a TCC_Curve object or specified as \"none\" (ignored). Default is \"none\". Multiplying the current values in the curve by the \"PhSlowPickup\" value gives the actual current.",
    ),
    (
        "Recloser.phslowpickup",
        "Multiplier for the phase slow TCC curve. Defaults to 1.0.",
    ),
    (
        "Recloser.ratedcurrent",
        "Recloser continuous rated current in Amps. Defaults to 0. Not used internally for either power flow or reporting.",
    ),
    (
        "Recloser.recloseintervals",
        "Array of reclose intervals.  Default for Recloser is (0.5, 2.0, 2.0) seconds. A locked out Recloser must be closed manually (action=close).",
    ),
    (
        "Recloser.reset",
        "{Yes | No} If Yes, forces Reset of recloser to Normal state and removes Lock independently of any internal reset command for mode change, etc.",
    ),
    (
        "Recloser.resettime",
        "Reset time in sec for Recloser. Default is 15.",
    ),
    (
        "Recloser.shots",
        "Total Number of fast and delayed shots to lockout.  Default is 4. This is one more than the number of reclose intervals.",
    ),
    (
        "Recloser.singlephlockout",
        "{Yes | No*} Enables single-phase lockout for multi-phase controlled elements with single-phase tripping. Does not have impact if single-phase trip is not enabled.",
    ),
    (
        "Recloser.singlephtrip",
        "{Yes | No*} Enables single-phase tripping and reclosing for multi-phase controlled elements. Previously locked out phases do not operate/reclose even considering multi-phase tripping.",
    ),
    (
        "Recloser.state",
        "ARRAY of strings {Open | Closed} representing the Actual state of the recloser in each phase of the controlled element. Upon setting, immediately forces the state of the recloser. Simulates manual control on Recloser. Defaults to Closed for all phases. Setting this property to {Open | Closed} sets the actual state to the specified value for all phases (ganged operation). \"Open\" causes the controlled element or respective phase to open and lock out. \"Closed\" causes the controlled element or respective phase to close and the recloser to reset to its first operation.",
    ),
    (
        "Recloser.switchedobj",
        "Name of circuit element switch that the Recloser controls. Specify the full object name.Defaults to the same as the Monitored element. This is the \"controlled\" element.",
    ),
    (
        "Recloser.switchedterm",
        "Number of the terminal of the controlled element in which the switch is controlled by the Recloser. 1 or 2, typically.  Default is 1.",
    ),
    (
        "Recloser.tdgndfast",
        "Time dial for Ground Fast trip curve. Multiplier on time axis of specified curve. Default=1.0.",
    ),
    (
        "Recloser.tdgndslow",
        "Time dial for Ground Slow trip curve. Multiplier on time axis of specified curve. Default=1.0.",
    ),
    (
        "Recloser.tdgrdelayed",
        "DEPRECATED. Assigned value is specified to \"TDGndSlow\" property for backwards compatibility. See \"TDGndSlow\" property.",
    ),
    (
        "Recloser.tdgrfast",
        "DEPRECATED. See \"TDGndFast\" property.",
    ),
    (
        "Recloser.tdphdelayed",
        "DEPRECATED. Assigned value is specified to \"TDPhSlow\" property for backwards compatibility. See \"TDPhSlow\" property.",
    ),
    (
        "Recloser.tdphfast",
        "Time dial for Phase Fast trip curve. Multiplier on time axis of specified curve. Default=1.0.",
    ),
    (
        "Recloser.tdphslow",
        "Time dial for Phase Slow trip curve. Multiplier on time axis of specified curve. Default=1.0.",
    ),
    (
        "RegControl.band",
        "Bandwidth in VOLTS for the controlled bus (see help for ptratio property).  Default is 3.0",
    ),
    ("RegControl.basefreq", "Base Frequency for ratings."),
    (
        "RegControl.bus",
        "Name of a bus (busname.nodename) in the system to use as the controlled bus instead of the bus to which the transformer winding is connected or the R and X line drop compensator settings.  Do not specify this value if you wish to use the line drop compensator settings.  Default is null string. Assumes the base voltage for this bus is the same as the transformer winding base specified above. Note: This bus (1-phase) WILL BE CREATED by the regulator control upon SOLVE if not defined by some other device. You can specify the node of the bus you wish to sample (defaults to 1). If specified, the RegControl is redefined as a 1-phase device since only one voltage is used.",
    ),
    (
        "RegControl.cogen",
        "{Yes|No*} Default is No. The Cogen feature is activated. Continues looking forward if power reverses, but switches to reverse-mode LDC, vreg and band values.",
    ),
    (
        "RegControl.ctprim",
        "Rating, in Amperes, of the primary CT rating for which the line amps convert to control rated amps.The typical default secondary ampere rating is 0.2 Amps (check with manufacturer specs). Current at which the LDC voltages match the R and X settings.",
    ),
    (
        "RegControl.debugtrace",
        "{Yes | No* }  Default is no.  Turn this on to capture the progress of the regulator model for each control iteration.  Creates a separate file for each RegControl named \"REG_name.csv\".",
    ),
    (
        "RegControl.delay",
        "Time delay, in seconds, from when the voltage goes out of band to when the tap changing begins. This is used to determine which regulator control will act first. Default is 15.  You may specify any floating point number to achieve a model of whatever condition is necessary.",
    ),
    (
        "RegControl.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "RegControl.eventlog",
        "{Yes/True | No/False*} Default is NO for regulator control. Log control actions to Eventlog.",
    ),
    (
        "RegControl.inversetime",
        "{Yes | No* } Default is no.  The time delay is adjusted inversely proportional to the amount the voltage is outside the band down to 10%.",
    ),
    (
        "RegControl.ldc_z",
        "Z value for Beckwith LDC_Z control option. Volts adjustment at rated control current.",
    ),
    (
        "RegControl.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "RegControl.maxtapchange",
        "Maximum allowable tap change per control iteration in STATIC control mode.  Default is 16. \n\nSet this to 1 to better approximate actual control action. \n\nSet this to 0 to fix the tap in the current position.",
    ),
    (
        "RegControl.ptphase",
        "For multi-phase transformers, the number of the phase being monitored or one of { MAX | MIN} for all phases. Default=1. Must be less than or equal to the number of phases. Ignored for regulated bus.",
    ),
    (
        "RegControl.ptratio",
        "Ratio of the PT that converts the controlled winding voltage to the regulator control voltage. Default is 60.  If the winding is Wye, the line-to-neutral voltage is used.  Else, the line-to-line voltage is used. SIDE EFFECT: Also sets RemotePTRatio property.",
    ),
    (
        "RegControl.r",
        "R setting on the line drop compensator in the regulator, expressed in VOLTS.",
    ),
    (
        "RegControl.remoteptratio",
        "When regulating a bus (the Bus= property is set), the PT ratio required to convert actual voltage at the remote bus to control voltage. Is initialized to PTratio property. Set this property after setting PTratio.",
    ),
    (
        "RegControl.reset",
        "{Yes | No} If Yes, forces Reset of this RegControl.",
    ),
    (
        "RegControl.rev_z",
        "Reverse Z value for Beckwith LDC_Z control option.",
    ),
    (
        "RegControl.revband",
        "Bandwidth for operating in the reverse direction.",
    ),
    (
        "RegControl.revdelay",
        "Time Delay in seconds (s) for executing the reversing action once the threshold for reversing has been exceeded. Default is 60 s.",
    ),
    (
        "RegControl.reversible",
        "{Yes |No*} Indicates whether or not the regulator can be switched to regulate in the reverse direction. Default is No.Typically applies only to line regulators and not to LTC on a substation transformer.",
    ),
    (
        "RegControl.revneutral",
        "{Yes | No*} Default is no. Set this to Yes if you want the regulator to go to neutral in the reverse direction or in cogen operation.",
    ),
    (
        "RegControl.revr",
        "R line drop compensator setting for reverse direction.",
    ),
    (
        "RegControl.revthreshold",
        "kW reverse power threshold for reversing the direction of the regulator. Default is 100.0 kw.",
    ),
    (
        "RegControl.revvreg",
        "Voltage setting in volts for operation in the reverse direction.",
    ),
    (
        "RegControl.revx",
        "X line drop compensator setting for reverse direction.",
    ),
    (
        "RegControl.tapdelay",
        "Delay in sec between tap changes. Default is 2. This is how long it takes between changes after the first change.",
    ),
    (
        "RegControl.tapnum",
        "An integer number indicating the tap position that the controlled transformer winding tap position is currently at, or is being set to.  If being set, and the value is outside the range of the transformer min or max tap, then set to the min or max tap position as appropriate. Default is 0",
    ),
    (
        "RegControl.tapwinding",
        "Winding containing the actual taps, if different than the WINDING property. Defaults to the same winding as specified by the WINDING property.",
    ),
    (
        "RegControl.transformer",
        "Name of Transformer or AutoTrans element to which the RegControl is connected. Do not specify the full object name; \"Transformer\" or \"AutoTrans\" is assumed for the object class.  Example:\n\nTransformer=Xfmr1",
    ),
    (
        "RegControl.vlimit",
        "Voltage Limit for bus to which regulated winding is connected (e.g. first customer). Default is 0.0. Set to a value greater then zero to activate this function.",
    ),
    (
        "RegControl.vreg",
        "Voltage regulator setting, in VOLTS, for the winding being controlled.  Multiplying this value times the ptratio should yield the voltage across the WINDING of the controlled transformer. Default is 120.0",
    ),
    (
        "RegControl.winding",
        "Number of the winding of the transformer element that the RegControl is monitoring. 1 or 2, typically.  Side Effect: Sets TAPWINDING property to the same winding.",
    ),
    (
        "RegControl.x",
        "X setting on the line drop compensator in the regulator, expressed in VOLTS.",
    ),
    (
        "Relay.46%pickup",
        "Percent pickup current for 46 relay (neg seq current).  Default is 20.0.   When current exceeds this value * BaseAmps, I-squared-t calc starts.",
    ),
    (
        "Relay.46baseamps",
        "Base current, Amps, for 46 relay (neg seq current).  Used for establishing pickup and per unit I-squared-t.",
    ),
    (
        "Relay.46isqt",
        "Negative Sequence I-squared-t trip value for 46 relay (neg seq current).  Default is 1 (trips in 1 sec for 1 per unit neg seq current).  Should be 1 to 99.",
    ),
    (
        "Relay.47%pickup",
        "Percent voltage pickup for 47 relay (Neg seq voltage). Default is 2. Specify also base voltage (kvbase) and delay time value.   ",
    ),
    ("Relay.action", "DEPRECATED. See \"State\" property"),
    ("Relay.basefreq", "Base Frequency for ratings."),
    (
        "Relay.breakertime",
        "Fixed delay time (sec) added to relay time. Default is 0.0. Designed to represent breaker time or some other delay after a trip decision is made.Use Delay property for setting a fixed trip time delay.Added to trip time of current and voltage relays. Could use in combination with inst trip value to obtain a definite time overcurrent relay.",
    ),
    (
        "Relay.debugtrace",
        "{Yes/True* | No/False* } Default is No for Relay. Write extra details to Eventlog.",
    ),
    (
        "Relay.delay",
        "Trip time delay (sec) for DEFINITE TIME relays. Default is 0.0 for current, voltage and DOC relays. If >0 then this value is used instead of curves. Used by Generic, RevPower, 46 and 47 relays. Defaults to 0.1 s for these relays.",
    ),
    (
        "Relay.distreverse",
        "{Yes/True* | No/False} Default is No; reverse direction for distance and td21 types.",
    ),
    (
        "Relay.doc_delayinner",
        "Trip time delay (sec) for operation in inner region for DOC relay, defined when \"DOC_TripSettingMag\" or \"DOC_TripSettingHigh\" are activate. Default is -1.0 (deactivated), meaning that the relay characteristic is insensitive in the inner region (no trip). Set to 0 for instantaneous trip and >0 for a definite time delay. If \"DOC_PhaseCurveInner\" is specified, time delay from curve is utilized instead.",
    ),
    (
        "Relay.doc_p1blocking",
        "{Yes/True* | No/False} Blocking element that impedes relay from tripping if balanced net three-phase active power is in the forward direction (i.e., flowing into the monitored terminal). For a delayed trip, if at any given time the reverse power flow condition stops, the tripping is reset. Default=True.",
    ),
    (
        "Relay.doc_phasecurveinner",
        "Name of the TCC Curve object that determines the phase trip for operation in inner region for DOC relay. Must have been previously defined as a TCC_Curve object. Default is none (ignored). Multiplying the current values in the curve by the \"DOC_PhaseTripInner\" value gives the actual current.",
    ),
    (
        "Relay.doc_phasetripinner",
        "Multiplier for the \"DOC_PhaseCurveInner\" TCC curve.  Defaults to 1.0.",
    ),
    (
        "Relay.doc_tdphaseinner",
        "Time dial for \"DOC_PhaseCurveInner\" TCC curve. Multiplier on time axis of specified curve. Default=1.0.",
    ),
    (
        "Relay.doc_tiltanglehigh",
        "Tilt angle for high-current trip line. Default is 90.",
    ),
    (
        "Relay.doc_tiltanglelow",
        "Tilt angle for low-current trip line. Default is 90.",
    ),
    (
        "Relay.doc_tripsettinghigh",
        "Resistive trip setting for high-current line.  Default is -1 (deactivated). To activate, set a positive value. Must be greater than \"DOC_TripSettingLow\".",
    ),
    (
        "Relay.doc_tripsettinglow",
        "Resistive trip setting for low-current line. Default is 0.",
    ),
    (
        "Relay.doc_tripsettingmag",
        "Trip setting for current magnitude (defines a circle in the relay characteristics). Default is -1 (deactivated). To activate, set a positive value.",
    ),
    (
        "Relay.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "Relay.eventlog",
        "{Yes/True | No/False* } Default is No for Relay. Write trips, reclose and reset events to EventLog.",
    ),
    (
        "Relay.groundcurve",
        "Name of the TCC Curve object that determines the ground trip.  Must have been previously defined as a TCC_Curve object. Default is none (ignored).For overcurrent relay, multiplying the current values in the curve by the \"groundtrip\" valuw gives the actual current.",
    ),
    (
        "Relay.groundinst",
        "Actual  amps for instantaneous ground trip which is assumed to happen in 0.01 sec + Delay Time.Default is 0.0, which signifies no inst trip.",
    ),
    (
        "Relay.groundtrip",
        "Multiplier or actual ground amps (3I0) for the ground TCC curve.  Defaults to 1.0.",
    ),
    (
        "Relay.kvbase",
        "Voltage base (kV) for the relay. Specify line-line for 3 phase devices); line-neutral for 1-phase devices.  Relay assumes the number of phases of the monitored element.  Default is 0.0, which results in assuming the voltage values in the \"TCC\" curve are specified in actual line-to-neutral volts.",
    ),
    (
        "Relay.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Relay.mground",
        "Ground reach multiplier in per-unit for Distance and TD21 functions. Default=0.7",
    ),
    (
        "Relay.monitoredobj",
        "Full object name of the circuit element, typically a line, transformer, load, or generator, to which the relay's PT and/or CT are connected. This is the \"monitored\" element. There is no default; must be specified.",
    ),
    (
        "Relay.monitoredterm",
        "Number of the terminal of the circuit element to which the Relay is connected. 1 or 2, typically.  Default is 1.",
    ),
    (
        "Relay.mphase",
        "Phase reach multiplier in per-unit for Distance and TD21 functions. Default=0.7",
    ),
    (
        "Relay.normal",
        "{Open | Closed} Normal state of the relay. The relay reverts to this state for reset, change of mode, etc. Defaults to \"State\" if not specifically declared.",
    ),
    (
        "Relay.overtrip",
        "Trip setting (high value) for Generic relay variable.  Relay trips in definite time if value of variable exceeds this value.",
    ),
    (
        "Relay.overvoltcurve",
        "TCC Curve object to use for overvoltage relay.  Curve is assumed to be defined with per unit voltage values. Voltage base should be defined for the relay. Default is none (ignored).",
    ),
    (
        "Relay.phasecurve",
        "Name of the TCC Curve object that determines the phase trip.  Must have been previously defined as a TCC_Curve object. Default is none (ignored). For overcurrent relay, multiplying the current values in the curve by the \"phasetrip\" value gives the actual current.",
    ),
    (
        "Relay.phaseinst",
        "Actual  amps (Current relay) or kW (reverse power relay) for instantaneous phase trip which is assumed to happen in 0.01 sec + Delay Time. Default is 0.0, which signifies no inst trip. Use this value for specifying the Reverse Power threshold (kW) for reverse power relays.",
    ),
    (
        "Relay.phasetrip",
        "Multiplier or actual phase amps for the phase TCC curve.  Defaults to 1.0.",
    ),
    (
        "Relay.recloseintervals",
        "Array of reclose intervals. If none, specify \"NONE\". Default for overcurrent relay is (0.5, 2.0, 2.0) seconds. Default for a voltage relay is (5.0). In a voltage relay, this is  seconds after restoration of voltage that the reclose occurs. Reverse power relay is one shot to lockout, so this is ignored.  A locked out relay must be closed manually (set action=close).",
    ),
    (
        "Relay.reset",
        "Reset time in sec for relay.  Default is 15. If this much time passes between the last pickup event, and the relay has not locked out, the operation counter resets.",
    ),
    (
        "Relay.shots",
        "Number of shots to lockout.  Default is 4. This is one more than the number of reclose intervals.",
    ),
    (
        "Relay.state",
        "{Open | Closed} Actual state of the relay. Upon setting, immediately forces state of the relay, overriding the Relay control. Simulates manual control on relay. Defaults to Closed. \"Open\" causes the controlled element to open and lock out. \"Closed\" causes the controlled element to close and the relay to reset to its first operation.",
    ),
    (
        "Relay.switchedobj",
        "Name of circuit element switch that the Relay controls. Specify the full object name.Defaults to the same as the Monitored element. This is the \"controlled\" element.",
    ),
    (
        "Relay.switchedterm",
        "Number of the terminal of the controlled element in which the switch is controlled by the Relay. 1 or 2, typically.  Default is 1.",
    ),
    (
        "Relay.tdground",
        "Time dial for Ground trip curve. Multiplier on time axis of specified curve. Default=1.0.",
    ),
    (
        "Relay.tdphase",
        "Time dial for Phase trip curve. Multiplier on time axis of specified curve. Default=1.0.",
    ),
    (
        "Relay.type",
        "One of a legal relay type:\n  Current\n  Voltage\n  Reversepower\n  46 (neg seq current)\n  47 (neg seq voltage)\n  Generic (generic over/under relay)\n  Distance\n  TD21\n  DOC (directional overcurrent)\n\nDefault is overcurrent relay (Current) Specify the curve and pickup settings appropriate for each type. Generic relays monitor PC Element Control variables and trip on out of over/under range in definite time.",
    ),
    (
        "Relay.undertrip",
        "Trip setting (low value) for Generic relay variable.  Relay trips in definite time if value of variable is less than this value.",
    ),
    (
        "Relay.undervoltcurve",
        "TCC Curve object to use for undervoltage relay.  Curve is assumed to be defined with per unit voltage values. Voltage base should be defined for the relay. Default is none (ignored).",
    ),
    (
        "Relay.variable",
        "Name of variable in PC Elements being monitored.  Only applies to Generic relay.",
    ),
    (
        "Relay.z0ang",
        "Zero sequence reach impedance angle in degrees for Distance and TD21 functions. Default=68.0",
    ),
    (
        "Relay.z0mag",
        "Zero sequence reach impedance in primary ohms for Distance and TD21 functions. Default=2.1",
    ),
    (
        "Relay.z1ang",
        "Positive sequence reach impedance angle in degrees for Distance and TD21 functions. Default=64.0",
    ),
    (
        "Relay.z1mag",
        "Positive sequence reach impedance in primary ohms for Distance and TD21 functions. Default=0.7",
    ),
    (
        "Sensor.%error",
        "Assumed percent error in the measurement. Default is 1.",
    ),
    (
        "Sensor.action",
        "NOT IMPLEMENTED.Action options: \nSQERROR: Show square error of the present value of the monitored terminal  \nquantity vs the sensor value. Actual values - convert to per unit in calling program.  \nValue reported in result window/result variable.",
    ),
    ("Sensor.basefreq", "Base Frequency for ratings."),
    (
        "Sensor.clear",
        "{ Yes | No }. Clear=Yes clears sensor values. Should be issued before putting in a new set of measurements.",
    ),
    (
        "Sensor.conn",
        "Voltage sensor Connection: { wye | delta | LN | LL }.  Default is wye. Applies to voltage measurement only. \nCurrents are always assumed to be line currents.\nIf wye or LN, voltage is assumed measured line-neutral; otherwise, line-line.",
    ),
    (
        "Sensor.currents",
        "Array of Currents (amps) measured by the current sensor. Specify this or power quantities; not both.",
    ),
    (
        "Sensor.deltadirection",
        "{1 or -1}  Default is 1:  1-2, 2-3, 3-1.  For reverse rotation, enter -1. Any positive or negative entry will suffice.",
    ),
    (
        "Sensor.element",
        "Name (Full Object name) of element to which the Sensor is connected.",
    ),
    (
        "Sensor.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "Sensor.kvars",
        "Array of Reactive power (kvar) measurements at the sensor. Is converted into Currents along with p=[...]",
    ),
    (
        "Sensor.kvbase",
        "Voltage base for the sensor, in kV. If connected to a 2- or 3-phase terminal, \nspecify L-L voltage. For 1-phase devices specify L-N or actual 1-phase voltage. Like many other DSS devices, default is 12.47kV.",
    ),
    (
        "Sensor.kvs",
        "Array of Voltages (kV) measured by the voltage sensor. For Delta-connected sensors, Line-Line voltages are expected. For Wye, Line-Neutral are expected.",
    ),
    (
        "Sensor.kws",
        "Array of Active power (kW) measurements at the sensor. Is converted into Currents along with q=[...]\nWill override any currents=[...] specification.",
    ),
    (
        "Sensor.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Sensor.terminal",
        "Number of the terminal of the circuit element to which the Sensor is connected. 1 or 2, typically. Default is 1.",
    ),
    ("Sensor.weight", "Weighting factor: Default is 1."),
    (
        "ShowOption.autoadded",
        "Shows auto added capacitors or generators. See AutoAdd solution mode.",
    ),
    (
        "ShowOption.buses",
        "Report showing all buses and nodes currently defined.",
    ),
    (
        "ShowOption.busflow",
        "Creates a report showing power and current flows as well as voltages around a selected bus. Syntax:\n\nShow BUSFlow busname [MVA|kVA*] [Seq* | Elements]\n\nShow busflow busxxx kVA elem\nShow busflow busxxx MVA seq\n\nNOTE: The Show menu will prompt you for these values.",
    ),
    (
        "ShowOption.controlled",
        "Show Controlled elements and the names of the controls connected to them in CSV format.",
    ),
    (
        "ShowOption.controlqueue",
        "Shows the present contents of the control queue.",
    ),
    (
        "ShowOption.convergence",
        "Report on the convergence of each node voltage.",
    ),
    (
        "ShowOption.currents",
        "Report showing currents from most recent solution. syntax: \n\nShow Currents  [[residual=]yes|no*] [Seq* | Elements]\n\nIf \"residual\" flag is yes, the sum of currents in all conductors is reported. Default is to report Sequence currents; otherwise currents in all conductors are reported.",
    ),
    (
        "ShowOption.deltav",
        "Show voltages ACROSS each 2-terminal element, phase-by-phase. ",
    ),
    (
        "ShowOption.elements",
        "Shows names of all elements in circuit or all elements of a specified class. Syntax: \n\nShow ELements [Classname] \n\nUseful for creating scripts that act on selected classes of elements. ",
    ),
    (
        "ShowOption.eventlog",
        "Shows the present event log. (Regulator tap changes, capacitor switching, etc.)",
    ),
    (
        "ShowOption.faults",
        "After fault study solution, shows fault currents.",
    ),
    (
        "ShowOption.generators",
        "Report showing generator elements currently defined and the values of the energy meters \nassociated with each generator.",
    ),
    (
        "ShowOption.isolated",
        "Report showing buses and elements that are isolated from the main source.",
    ),
    (
        "ShowOption.kvbasemismatch",
        "Creates a report of Load and Generator elements for which the base voltage does not match the Bus base voltage. Scripts for correcting the voltage base are suggested.",
    ),
    (
        "ShowOption.lineconstants",
        "Creates two report files for the line constants (impedances) of every LINEGEOMETRY element currently defined. One file shows the main report with the matrices. The other file contains corresponding LINECODE definitions that you may use in subsequent simulations.  Syntax:\n\nShow LIneConstants [frequency] [none|mi|km|kft|m|me|ft|in|cm] [rho]\n\nSpecify the frequency, length units and earth resistivity (meter-ohms). Examples:\n\nShow Lineconstants 60 kft 100\nShow Linecon 50 km 1000",
    ),
    (
        "ShowOption.loops",
        "Shows closed loops detected by EnergyMeter elements that are possibly unwanted. Otherwise, loops are OK.",
    ),
    (
        "ShowOption.losses",
        "Reports losses in each element and in the entire circuit.",
    ),
    (
        "ShowOption.meters",
        "Shows the present values of the registers in the EnergyMeter elements.",
    ),
    (
        "ShowOption.mismatch",
        "Shows the current mismatches at each node in amperes and percent of max currents at node.",
    ),
    (
        "ShowOption.monitor",
        "Shows the contents of a selected monitor. Syntax: \n\n Show Monitor  monitorname",
    ),
    (
        "ShowOption.overloads",
        "Shows overloaded power delivery elements.",
    ),
    (
        "ShowOption.panel",
        "Shows control panel. (not necessary for standalone version)",
    ),
    (
        "ShowOption.powers",
        "Report on powers flowing in circuit from most recent solution. \nPowers may be reported in kVA or MVA and in sequence quantities or in every conductor of each element. Syntax:\n\nShow Powers [MVA|kVA*] [Seq* | Elements]\n\nSequence powers in kVA is the default. Examples:\n\nShow powers\nShow power kva element\nShow power mva elem",
    ),
    ("ShowOption.querylog", "Show Query Log file. "),
    (
        "ShowOption.ratings",
        "Shows ratings of power delivery elements.",
    ),
    (
        "ShowOption.result",
        "Show last result (in @result variable).",
    ),
    (
        "ShowOption.taps",
        "Shows the regulator/LTC taps from the most recent solution.",
    ),
    (
        "ShowOption.topology",
        "Shows the topology as seen by the SwtControl elements.",
    ),
    (
        "ShowOption.unserved",
        "Shows loads that are \"unserved\". That is, loads for which the voltage is too low, or a branch on the source side is overloaded. If UEonly is specified, shows only those loads in which the emergency rating has been exceeded. Syntax:\n\nShow Unserved [UEonly] (unserved loads)",
    ),
    (
        "ShowOption.variables",
        "Shows internal state variables of devices (Power conversion elements) that report them.",
    ),
    (
        "ShowOption.voltages",
        "Reports voltages from most recent solution. Voltages are reported with respect to \nsystem reference (Node 0) by default (LN option), but may also be reported Line-Line (LL option).\nThe voltages are normally reported by bus/node, but may also be reported by circuit element. Syntax:\n\nShow Voltages [LL |LN*]  [Seq* | Nodes | Elements]\n\nShow Voltages\nShow Voltage LN Nodes\nShow Voltages LL Nodes\nShow Voltage LN Elem",
    ),
    (
        "ShowOption.y",
        "Show the system Y matrix. Could be a large file!",
    ),
    (
        "ShowOption.yprim",
        "Show the primitive admittance (y) matrix for the active element.",
    ),
    (
        "ShowOption.zone",
        "Shows the zone for a selected EnergyMeter element. Shows zone either in a text file or in a graphical tree view.\n\nShow Zone  energymetername [Treeview]",
    ),
    (
        "Spectrum.%mag",
        "Array of magnitude values, assumed to be in PERCENT. You can also use the syntax\n%mag = (file=filename)     !for text file one value per line\n%mag = (dblfile=filename)  !for packed file of doubles\n%mag = (sngfile=filename)  !for packed file of singles ",
    ),
    (
        "Spectrum.angle",
        "Array of phase angle values, degrees.You can also use the syntax\nangle = (file=filename)     !for text file one value per line\nangle = (dblfile=filename)  !for packed file of doubles\nangle = (sngfile=filename)  !for packed file of singles ",
    ),
    (
        "Spectrum.csvfile",
        "File of spectrum points with (harmonic, magnitude-percent, angle-degrees) values, one set of 3 per line, in CSV format. If fewer than NUMHARM frequencies found in the file, NUMHARM is set to the smaller value.",
    ),
    (
        "Spectrum.harmonic",
        "Array of harmonic values. You can also use the syntax\nharmonic = (file=filename)     !for text file one value per line\nharmonic = (dblfile=filename)  !for packed file of doubles\nharmonic = (sngfile=filename)  !for packed file of singles ",
    ),
    (
        "Spectrum.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Spectrum.numharm",
        "Number of frequencies in this spectrum. (See CSVFile)",
    ),
    (
        "Storage.%charge",
        "Charging rate (input power) in percentage of rated kW. Default = 100.",
    ),
    (
        "Storage.%cutin",
        "Cut-in power as a percentage of inverter kVA rating. It is the minimum DC power necessary to turn the inverter ON when it is OFF. Must be greater than or equal to %CutOut. Defaults to 2 for PVSystems and 0 for Storage elements which means that the inverter state will be always ON for this element.",
    ),
    (
        "Storage.%cutout",
        "Cut-out power as a percentage of inverter kVA rating. It is the minimum DC power necessary to keep the inverter ON. Must be less than or equal to %CutIn. Defaults to 0, which means that, once ON, the inverter state will be always ON for this element.",
    ),
    (
        "Storage.%discharge",
        "Discharge rate (output power) in percentage of rated kW. Default = 100.",
    ),
    (
        "Storage.%effcharge",
        "Percentage efficiency for CHARGING the Storage element. Default = 90.",
    ),
    (
        "Storage.%effdischarge",
        "Percentage efficiency for DISCHARGING the Storage element. Default = 90.",
    ),
    ("Storage.%idlingkvar", "Deprecated."),
    (
        "Storage.%idlingkw",
        "Percentage of rated kW consumed by idling losses. Default = 1.",
    ),
    (
        "Storage.%kwrated",
        "Upper limit on active power as a percentage of kWrated. Defaults to 100 (disabled).",
    ),
    (
        "Storage.%pminkvarmax",
        "Minimum active power as percentage of kWrated that allows the inverter to produce/absorb reactive power up to its maximum reactive power, which can be either kvarMax or kvarMaxAbs, depending on the current operation quadrant. Defaults to 0 (disabled).",
    ),
    (
        "Storage.%pminnovars",
        "Minimum active power as percentage of kWrated under which there is no vars production/absorption. Defaults to 0 (disabled).",
    ),
    (
        "Storage.%r",
        "Equivalent percentage internal resistance, ohms. Default is 0. Placed in series with internal voltage source for harmonics and dynamics modes. Use a combination of %IdlingkW, %EffCharge and %EffDischarge to account for losses in power flow modes.",
    ),
    (
        "Storage.%reserve",
        "Percentage of rated kWh Storage capacity to be held in reserve for normal operation. Default = 20. \nThis is treated as the minimum energy discharge level unless there is an emergency. For emergency operation set this property lower. Cannot be less than zero.",
    ),
    (
        "Storage.%stored",
        "Present amount of energy stored, % of rated kWh. Default is 100.",
    ),
    (
        "Storage.%x",
        "Equivalent percentage internal reactance, ohms. Default is 50%. Placed in series with internal voltage source for harmonics and dynamics modes. (Limits fault current to 2 pu.",
    ),
    (
        "Storage.amplimit",
        "The current limiter per phase for the IBR when operating in GFM mode. This limit is imposed to prevent the IBR to enter into Safe Mode when reaching the IBR power ratings.\nOnce the IBR reaches this value, it remains there without moving into Safe Mode. This value needs to be set lower than the IBR Amps rating.",
    ),
    (
        "Storage.amplimitgain",
        "Use it for fine tunning the current limiter when active, by default is 0.8, it has to be a value between 0.1 and 1. This value allows users to fine tune the IBRs current limiter to match with the user requirements.",
    ),
    (
        "Storage.balanced",
        "{Yes | No*} Default is No. Force balanced current only for 3-phase Storage. Forces zero- and negative-sequence to zero. ",
    ),
    ("Storage.basefreq", "Base Frequency for ratings."),
    (
        "Storage.bus1",
        "Bus to which the Storage element is connected.  May include specific node specification.",
    ),
    (
        "Storage.chargetrigger",
        "Dispatch trigger value for charging the Storage. \n\nIf = 0.0 the Storage element state is changed by the State command or StorageController object.  \n\nIf <> 0  the Storage element state is set to CHARGING when this trigger level is GREATER than either the specified Loadshape curve value or the price signal or global Loadlevel value, depending on dispatch mode. See State property.",
    ),
    (
        "Storage.class",
        "An arbitrary integer number representing the class of Storage element so that Storage values may be segregated by class.",
    ),
    ("Storage.conn", "={wye|LN|delta|LL}.  Default is wye."),
    (
        "Storage.controlmode",
        "Defines the control mode for the inverter. It can be one of {GFM | GFL*}. By default it is GFL (Grid Following Inverter). Use GFM (Grid Forming Inverter) for energizing islanded microgrids, but, if the device is connected to the grid, it is highly recommended to use GFL.\n\nGFM control mode disables any control action set by the InvControl device.",
    ),
    (
        "Storage.daily",
        "Dispatch shape to use for daily simulations.  Must be previously defined as a Loadshape object of 24 hrs, typically.  In the default dispatch mode, the Storage element uses this loadshape to trigger State changes.",
    ),
    (
        "Storage.debugtrace",
        "{Yes | No }  Default is no.  Turn this on to capture the progress of the Storage model for each iteration.  Creates a separate file for each Storage element named \"Storage_name.csv\".",
    ),
    (
        "Storage.dischargetrigger",
        "Dispatch trigger value for discharging the Storage. \nIf = 0.0 the Storage element state is changed by the State command or by a StorageController object. \nIf <> 0  the Storage element state is set to DISCHARGING when this trigger level is EXCEEDED by either the specified Loadshape curve value or the price signal or global Loadlevel value, depending on dispatch mode. See State property.",
    ),
    (
        "Storage.dispmode",
        "{DEFAULT | FOLLOW | EXTERNAL | LOADLEVEL | PRICE } Default = \"DEFAULT\". Dispatch mode. \n\nIn DEFAULT mode, Storage element state is triggered to discharge or charge at the specified rate by the loadshape curve corresponding to the solution mode. \n\nIn FOLLOW mode the kW output of the Storage element follows the active loadshape multiplier until Storage is either exhausted or full. The element discharges for positive values and charges for negative values.  The loadshape is based on rated kW. \n\nIn EXTERNAL mode, Storage element state is controlled by an external Storagecontroller. This mode is automatically set if this Storage element is included in the element list of a StorageController element. \n\nFor the other two dispatch modes, the Storage element state is controlled by either the global default Loadlevel value or the price level. ",
    ),
    (
        "Storage.duty",
        "Load shape to use for duty cycle dispatch simulations such as for solar ramp rate studies. Must be previously defined as a Loadshape object. \n\nTypically would have time intervals of 1-5 seconds. \n\nDesignate the number of points to solve using the Set Number=xxxx command. If there are fewer points in the actual shape, the shape is assumed to repeat.",
    ),
    (
        "Storage.dynadata",
        "String (in quotes or parentheses if necessary) that gets passed to the user-written dynamics model Edit function for defining the data required for that model.",
    ),
    (
        "Storage.dynadll",
        "Name of DLL containing user-written dynamics model, which computes the terminal currents for Dynamics-mode simulations, overriding the default model.  Set to \"none\" to negate previous setting. This DLL has a simpler interface than the UserModel DLL and is only used for Dynamics mode.",
    ),
    (
        "Storage.dynamiceq",
        "The name of the dynamic equation (DynamicExp) that will be used for defining the dynamic behavior of the generator. If not defined, the generator dynamics will follow the built-in dynamic equation.",
    ),
    (
        "Storage.dynout",
        "The name of the variables within the Dynamic equation that will be used to govern the Storage dynamics. This Storage model requires 1 output from the dynamic equation:\n\n    1. Current.\n\nThe output variables need to be defined in the same order.",
    ),
    (
        "Storage.effcurve",
        "An XYCurve object, previously defined, that describes the PER UNIT efficiency vs PER UNIT of rated kVA for the inverter. Power at the AC side of the inverter is discounted by the multiplier obtained from this curve.",
    ),
    (
        "Storage.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "Storage.kp",
        "It is the proportional gain for the PI controller within the inverter. Use it to modify the controller response in dynamics simulation mode.",
    ),
    (
        "Storage.kv",
        "Nominal rated (1.0 per unit) voltage, kV, for Storage element. For 2- and 3-phase Storage elements, specify phase-phase kV. Otherwise, specify actual kV across each branch of the Storage element. \n\nIf wye (star), specify phase-neutral kV. \n\nIf delta or phase-phase connected, specify phase-phase kV.",
    ),
    (
        "Storage.kva",
        "Indicates the inverter nameplate capability (in kVA). Used as the base for Dynamics mode and Harmonics mode values.",
    ),
    (
        "Storage.kvar",
        "Get/set the requested kvar value. Final kvar is subjected to the inverter ratings. Sets inverter to operate in constant kvar mode.",
    ),
    (
        "Storage.kvarmax",
        "Indicates the maximum reactive power GENERATION (un-signed numerical variable in kvar) for the inverter. Defaults to kVA rating of the inverter.",
    ),
    (
        "Storage.kvarmaxabs",
        "Indicates the maximum reactive power ABSORPTION (un-signed numerical variable in kvar) for the inverter. Defaults to kvarMax.",
    ),
    (
        "Storage.kvdc",
        "Indicates the rated voltage (kV) at the input of the inverter while the storage is discharging. The value is normally greater or equal to the kV base of the Storage device. It is used for dynamics simulation ONLY.",
    ),
    (
        "Storage.kw",
        "Get/set the requested kW value. Final kW is subjected to the inverter ratings. A positive value denotes power coming OUT of the element, which is the opposite of a Load element. A negative value indicates the Storage element is in Charging state. This value is modified internally depending on the dispatch mode.",
    ),
    (
        "Storage.kwhrated",
        "Rated Storage capacity in kWh. Default is 50.",
    ),
    (
        "Storage.kwhstored",
        "Present amount of energy stored, kWh. Default is same as kWhrated.",
    ),
    (
        "Storage.kwrated",
        "kW rating of power output. Base for Loadshapes when DispMode=Follow. Sets kVA property if it has not been specified yet. Defaults to 25.",
    ),
    (
        "Storage.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Storage.limitcurrent",
        "Limits current magnitude to Vminpu value for both 1-phase and 3-phase Storage similar to Generator Model 7. For 3-phase, limits the positive-sequence current but not the negative-sequence.",
    ),
    (
        "Storage.model",
        "Integer code (default=1) for the model to be used for power output variation with voltage. Valid values are:\n\n1:Storage element injects/absorbs a CONSTANT power.\n2:Storage element is modeled as a CONSTANT IMPEDANCE.\n3:Compute load injection from User-written Model.",
    ),
    (
        "Storage.pf",
        "Get/set the requested PF value. Final PF is subjected to the inverter ratings. Sets inverter to operate in constant PF mode. Nominally, the power factor for discharging (acting as a generator). Default is 1.0. \n\nEnter negative for leading power factor (when kW and kvar have opposite signs.)\n\nA positive power factor signifies kw and kvar at the same direction.",
    ),
    (
        "Storage.pfpriority",
        "If set to true, priority is given to power factor and WattPriority is neglected. It works only if operating in either constant PF or constant kvar modes. Defaults to False.",
    ),
    (
        "Storage.phases",
        "Number of Phases, this Storage element.  Power is evenly divided among phases.",
    ),
    (
        "Storage.pitol",
        "It is the tolerance (%) for the closed loop controller of the inverter. For dynamics simulation mode.",
    ),
    (
        "Storage.safemode",
        "(Read only) Indicates whether the inverter entered (Yes) or not (No) into Safe Mode.",
    ),
    (
        "Storage.safevoltage",
        "Indicates the voltage level (%) respect to the base voltage level for which the Inverter will operate. If this threshold is violated, the Inverter will enter safe mode (OFF). For dynamic simulation. By default is 80%.",
    ),
    (
        "Storage.spectrum",
        "Name of harmonic voltage or current spectrum for this Storage element. Current injection is assumed for inverter. Default value is \"default\", which is defined when the DSS starts.",
    ),
    (
        "Storage.state",
        "{IDLING | CHARGING | DISCHARGING}  Get/Set present operational state. In DISCHARGING mode, the Storage element acts as a generator and the kW property is positive. The element continues discharging at the scheduled output power level until the Storage reaches the reserve value. Then the state reverts to IDLING. In the CHARGING state, the Storage element behaves like a Load and the kW property is negative. The element continues to charge until the max Storage kWh is reached and then switches to IDLING state. In IDLING state, the element draws the idling losses plus the associated inverter losses.",
    ),
    (
        "Storage.timechargetrig",
        "Time of day in fractional hours (0230 = 2.5) at which Storage element will automatically go into charge state. Default is 2.0.  Enter a negative time value to disable this feature.",
    ),
    (
        "Storage.userdata",
        "String (in quotes or parentheses) that gets passed to user-written model for defining the data required for that model.",
    ),
    (
        "Storage.usermodel",
        "Name of DLL containing user-written model, which computes the terminal currents for both power flow and dynamics, overriding the default model.  Set to \"none\" to negate previous setting.",
    ),
    (
        "Storage.varfollowinverter",
        "Boolean variable (Yes|No) or (True|False). Defaults to False, which indicates that the reactive power generation/absorption does not respect the inverter status.When set to True, the reactive power generation/absorption will cease when the inverter status is off, due to DC kW dropping below %CutOut.  The reactive power generation/absorption will begin again when the DC kW is above %CutIn.  When set to False, the Storage will generate/absorb reactive power regardless of the status of the inverter.",
    ),
    (
        "Storage.vmaxpu",
        "Default = 1.10.  Maximum per unit voltage for which the Model is assumed to apply. Above this value, the load model reverts to a constant impedance model.",
    ),
    (
        "Storage.vminpu",
        "Default = 0.90.  Minimum per unit voltage for which the Model is assumed to apply. Below this value, the load model reverts to a constant impedance model.",
    ),
    (
        "Storage.wattpriority",
        "{Yes/No*/True/False} Set inverter to watt priority instead of the default var priority.",
    ),
    (
        "Storage.yearly",
        "Dispatch shape to use for yearly simulations.  Must be previously defined as a Loadshape object. If this is not specified, the Daily dispatch shape, if any, is repeated during Yearly solution modes. In the default dispatch mode, the Storage element uses this loadshape to trigger State changes.",
    ),
    (
        "StorageController.%kwband",
        "Bandwidth (% of Target kW/kamps) of the dead band around the kW/kamps target value. Default is 2% (+/-1%).No dispatch changes are attempted if the power in the monitored terminal stays within this band.",
    ),
    (
        "StorageController.%kwbandlow",
        "Bandwidth (% of kWTargetLow) of the dead band around the kW/kamps low target value. Default is 2% (+/-1%).No charging is attempted if the power in the monitored terminal stays within this band.",
    ),
    (
        "StorageController.%ratecharge",
        "Sets the kW charging rate in % of rated capacity for each element of the fleet. Applies to TIME control mode and anytime charging mode is entered due to a time trigger.",
    ),
    (
        "StorageController.%ratekw",
        "Sets the kW discharge rate in % of rated capacity for each element of the fleet. Applies to TIME control mode, SCHEDULE mode, or anytime discharging is triggered by time.",
    ),
    (
        "StorageController.%reserve",
        "Use this property to change the % reserve for each Storage element under control of this controller. This might be used, for example, to allow deeper discharges of Storage or in case of emergency operation to use the remainder of the Storage element.",
    ),
    ("StorageController.basefreq", "Base Frequency for ratings."),
    (
        "StorageController.daily",
        "Dispatch loadshape object, If any, for Daily solution mode.",
    ),
    (
        "StorageController.dispfactor",
        "Defaults to 1 (disabled). Set to any value between 0 and 1 to enable this parameter.\n\nUse this parameter to reduce the amount of power requested by the controller in each control iteration. It can be useful when maximum control iterations are exceeded due to numerical instability such as fleet being set to charging and idling in subsequent control iterations (check the Eventlog). ",
    ),
    (
        "StorageController.duty",
        "Dispatch loadshape object, If any, for Dutycycle solution mode.",
    ),
    (
        "StorageController.element",
        "Full object name of the circuit element, typically a line or transformer, which the control is monitoring. There is no default; Must be specified.In \"Local\" control mode, is the name of the load that will be managed by the storage device, which should be installed at the same bus.",
    ),
    (
        "StorageController.elementlist",
        "Array list of Storage elements to be controlled.  If not specified, all Storage elements in the circuit not presently dispatched by another controller are assumed dispatched by this controller.",
    ),
    (
        "StorageController.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "StorageController.eventlog",
        "{Yes/True | No/False} Default is No. Log control actions to Eventlog.",
    ),
    (
        "StorageController.inhibittime",
        "Hours (integer) to inhibit Discharging after going into Charge mode. Default is 5.",
    ),
    (
        "StorageController.kwactual",
        "(Read only). Actual kW output of all controlled Storage elements. ",
    ),
    (
        "StorageController.kwband",
        "Alternative way of specifying the bandwidth. (kW/kamps) of the dead band around the kW/kamps target value. Default is 2% of kWTarget (+/-1%).No dispatch changes are attempted if the power in the monitored terminal stays within this band.",
    ),
    (
        "StorageController.kwbandlow",
        "Alternative way of specifying the bandwidth. (kW/kamps) of the dead band around the kW/kamps low target value. Default is 2% of kWTargetLow (+/-1%).No charging is attempted if the power in the monitored terminal stays within this band.",
    ),
    (
        "StorageController.kwhactual",
        "(Read only). Actual kWh stored of all controlled Storage elements. ",
    ),
    (
        "StorageController.kwhtotal",
        "(Read only). Total rated kWh energy Storage capacity of Storage elements controlled by this controller.",
    ),
    (
        "StorageController.kwneed",
        "(Read only). KW needed to meet target.",
    ),
    (
        "StorageController.kwtarget",
        "kW/kamps target for Discharging. The Storage element fleet is dispatched to try to hold the power/current in band at least until the Storage is depleted. The selection of power or current depends on the Discharge mode (PeakShave->kW, I-PeakShave->kamps).",
    ),
    (
        "StorageController.kwtargetlow",
        "kW/kamps target for Charging. The Storage element fleet is dispatched to try to hold the power/current in band at least until the Storage is fully charged. The selection of power or current depends on the charge mode (PeakShavelow->kW, I-PeakShavelow->kamps).",
    ),
    (
        "StorageController.kwthreshold",
        "Threshold, kW, for Follow mode. kW has to be above this value for the Storage element to be dispatched on. Defaults to 75% of the kWTarget value. Must reset this property after setting kWTarget if you want a different value.",
    ),
    (
        "StorageController.kwtotal",
        "(Read only). Total rated kW power capacity of Storage elements controlled by this controller.",
    ),
    (
        "StorageController.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "StorageController.modecharge",
        "{Loadshape | Time* | PeakShaveLow | I-PeakShaveLow} Mode of operation for the CHARGE FUNCTION of this controller. \n\nIn Loadshape mode, both charging and discharging precisely follows the per unit loadshape. Storage is charged when the loadshape value is negative. \n\nIn Time mode, the Storage charging FUNCTION is triggered at the specified %RateCharge at the specified charge trigger time in fractional hours.\n\nIn PeakShaveLow mode, the charging operation will charge the Storage fleet when the power at a monitored element is below a specified KW target (kWTarget_low). The Storage will charge as much power as necessary to keep the power within the deadband around kWTarget_low.\n\nIn I-PeakShaveLow mode, the charging operation will charge the Storage fleet when the current (Amps) at a monitored element is below a specified amps target (kWTarget_low). The Storage will charge as much power as necessary to keep the amps within the deadband around kWTarget_low. When this control mode is active, the property kWTarget_low will be expressed in k-amps and all the other parameters will be adjusted to match the amps (current) control criteria.",
    ),
    (
        "StorageController.modedischarge",
        "{PeakShave* | Follow | Support | Loadshape | Time | Schedule | I-PeakShave} Mode of operation for the DISCHARGE FUNCTION of this controller. \n\nIn PeakShave mode (Default), the control attempts to discharge Storage to keep power in the monitored element below the kWTarget. \n\nIn Follow mode, the control is triggered by time and resets the kWTarget value to the present monitored element power. It then attempts to discharge Storage to keep power in the monitored element below the new kWTarget. See TimeDischargeTrigger.\n\nIn Support mode, the control operates oppositely of PeakShave mode: Storage is discharged to keep kW power output up near the target. \n\nIn Loadshape mode, both charging and discharging precisely follows the per unit loadshape. Storage is discharged when the loadshape value is positive. \n\nIn Time mode, the Storage discharge is turned on at the specified %RatekW at the specified discharge trigger time in fractional hours.\n\nIn Schedule mode, the Tup, TFlat, and Tdn properties specify the up ramp duration, flat duration, and down ramp duration for the schedule. The schedule start time is set by TimeDischargeTrigger and the rate of discharge for the flat part is determined by %RatekW.\n\nIn I-PeakShave mode, the control attempts to discharge Storage to keep current in the monitored element below the target given in k-amps (thousands of amps), when this control mode is active, the property kWTarget will be expressed in k-amps. ",
    ),
    (
        "StorageController.monphase",
        "Number of the phase being monitored or one of {AVG | MAX | MIN} for all phases. Default=MAX. Must be less than the number of phases. Used in PeakShave, Follow, Support and I-PeakShave discharging modes and in PeakShaveLow, I-PeakShaveLow charging modes. For modes based on active power measurements, the value used by the control is the monitored one multiplied by the number of phases of the monitored element.",
    ),
    (
        "StorageController.resetlevel",
        "The level of charge required for allowing the storage to discharge again after reaching the reserve storage level. After reaching this level, the storage control  will not allow the storage device to discharge, forcing the storage to charge. Once the storage reaches this level, the storage will be able to discharge again. This value is a number between 0.2 and 1",
    ),
    (
        "StorageController.seasons",
        "With this property the user can specify the number of targets to be used by the controller using the list given at \"SeasonTargets\"/\"SeasonTargetsLow\", which can be used to dynamically adjust the storage controller during a QSTS simulation. The default value is 1. This property needs to be defined before defining SeasonTargets/SeasonTargetsLow.",
    ),
    (
        "StorageController.seasontargets",
        "An array of doubles specifying the targets to be used during a QSTS simulation. These targets will take effect only if SeasonRating=true. The number of targets cannot exceed the number of seasons defined at the SeasonSignal.The difference between the targets defined at SeasonTargets and SeasonTargetsLow is that SeasonTargets applies to discharging modes, while SeasonTargetsLow applies to charging modes.",
    ),
    (
        "StorageController.seasontargetslow",
        "An array of doubles specifying the targets to be used during a QSTS simulation. These targets will take effect only if SeasonRating=true. The number of targets cannot exceed the number of seasons defined at the SeasonSignal.The difference between the targets defined at SeasonTargets and SeasonTargetsLow is that SeasonTargets applies to discharging modes, while SeasonTargetsLow applies to charging modes.",
    ),
    (
        "StorageController.tdn",
        "Duration, hrs, of downramp part for SCHEDULE mode. Default is 0.25.",
    ),
    (
        "StorageController.terminal",
        "Number of the terminal of the circuit element to which the StorageController control is connected. 1 or 2, typically.  Default is 1. Make sure to select the proper direction on the power for the respective dispatch mode.",
    ),
    (
        "StorageController.tflat",
        "Duration, hrs, of flat part for SCHEDULE mode. Default is 2.0.",
    ),
    (
        "StorageController.timechargetrigger",
        "Default time of day (hr) for initiating charging in Time control mode. Set this to a negative value to ignore. Default is 2.0.  (0200).When this value is >0 the Storage fleet is set to charging at this time regardless of other control criteria to make sure Storage is topped off for the next discharge cycle.",
    ),
    (
        "StorageController.timedischargetrigger",
        "Default time of day (hr) for initiating Discharging of the fleet. During Follow or Time mode discharging is triggered at a fixed time each day at this hour. If Follow mode, Storage will be discharged to attempt to hold the load at or below the power level at the time of triggering. In Time mode, the discharge is based on the %RatekW property value. Set this to a negative value to ignore. Default is 12.0 for Follow mode; otherwise it is -1 (ignored). ",
    ),
    (
        "StorageController.tup",
        "Duration, hrs, of upramp part for SCHEDULE mode. Default is 0.25.",
    ),
    (
        "StorageController.weights",
        "Array of proportional weights corresponding to each Storage element in the ElementList. The needed kW or kvar to get back to center band is dispatched to each Storage element according to these weights. Default is to set all weights to 1.0.",
    ),
    (
        "StorageController.yearly",
        "Dispatch loadshape object, If any, for Yearly solution Mode.",
    ),
    (
        "SwtControl.action",
        "{Open | Close}  After specified delay time, and if not locked, causes the controlled switch to open or close. ",
    ),
    ("SwtControl.basefreq", "Base Frequency for ratings."),
    (
        "SwtControl.delay",
        "Operating time delay (sec) of the switch. Defaults to 120.",
    ),
    (
        "SwtControl.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "SwtControl.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "SwtControl.lock",
        "{Yes | No} Delayed action. Sends CTRL_LOCK or CTRL_UNLOCK message to control queue. After delay time, controlled switch is locked in its present open / close state or unlocked. Switch will not respond to either manual (Action) or automatic (APIs) control or internal OpenDSS Reset when locked.",
    ),
    (
        "SwtControl.normal",
        "{Open | Closed] Normal state of the switch. If not Locked, the switch reverts to this state for reset, change of mode, etc. Defaults to first Action or State specified if not specifically declared.",
    ),
    (
        "SwtControl.reset",
        "{Yes | No} If Yes, forces Reset of switch to Normal state and removes Lock independently of any internal reset command for mode change, etc.",
    ),
    (
        "SwtControl.state",
        "{Open | Closed] Present state of the switch. Upon setting, immediately forces state of switch.",
    ),
    (
        "SwtControl.switchedobj",
        "Name of circuit element switch that the SwtControl operates. Specify the full object class and name.",
    ),
    (
        "SwtControl.switchedterm",
        "Terminal number of the controlled element switch. 1 or 2, typically.  Default is 1.",
    ),
    (
        "TCC_Curve.c_array",
        "Array of current (or voltage) values corresponding to time values (see help on T_Array).",
    ),
    (
        "TCC_Curve.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "TCC_Curve.npts",
        "Number of points to expect in time-current arrays.",
    ),
    (
        "TCC_Curve.t_array",
        "Array of time values in sec. Typical array syntax: \nt_array = (1, 2, 3, 4, ...)\n\nCan also substitute a file designation: \nt_array =  (file=filename)\n\nThe specified file has one value per line.",
    ),
    (
        "TSData.capradius",
        "Equivalent conductor radius for capacitance calcs. Specify this for bundled conductors. Defaults to same value as radius. Define Diam or Radius property first.",
    ),
    (
        "TSData.diacable",
        "Diameter over cable; same units as radius; no default.",
    ),
    (
        "TSData.diains",
        "Diameter over insulation layer; same units as radius; no default. Establishes outer radius for capacitance calculation.",
    ),
    (
        "TSData.diam",
        "Diameter; Alternative method for entering radius.",
    ),
    (
        "TSData.diashield",
        "Diameter over tape shield; same units as radius; no default.",
    ),
    (
        "TSData.emergamps",
        "Emergency ampacity, amperes. Defaults to 1.5 * Normal Amps if not specified.",
    ),
    (
        "TSData.epsr",
        "Insulation layer relative permittivity; default is 2.3.",
    ),
    (
        "TSData.gmrac",
        "GMR at 60 Hz. Defaults to .7788*radius if not specified.",
    ),
    (
        "TSData.gmrunits",
        "Units for GMR: {mi|kft|km|m|Ft|in|cm|mm} Default=none.",
    ),
    (
        "TSData.inslayer",
        "Insulation layer thickness; same units as radius; no default. With DiaIns, establishes inner radius for capacitance calculation.",
    ),
    (
        "TSData.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "TSData.normamps",
        "Normal ampacity, amperes. Defaults to Emergency amps/1.5 if not specified.",
    ),
    (
        "TSData.rac",
        "Resistance at 60 Hz per unit length. Defaults to 1.02*Rdc if not specified.",
    ),
    (
        "TSData.radius",
        "Outside radius of conductor. Defaults to GMR/0.7788 if not specified.",
    ),
    (
        "TSData.radunits",
        "Units for outside radius: {mi|kft|km|m|Ft|in|cm|mm} Default=none.",
    ),
    (
        "TSData.ratings",
        "An array of ratings to be used when the seasonal ratings flag is True. It can be used to insert\nmultiple ratings to change during a QSTS simulation to evaluate different ratings in lines.",
    ),
    (
        "TSData.rdc",
        "dc Resistance, ohms per unit length (see Runits). Defaults to Rac/1.02 if not specified.",
    ),
    (
        "TSData.runits",
        "Length units for resistance: ohms per {mi|kft|km|m|Ft|in|cm|mm} Default=none.",
    ),
    (
        "TSData.seasons",
        "Defines the number of ratings to be defined for the wire, to be used only when defining seasonal ratings using the \"Ratings\" property.",
    ),
    ("TSData.tapelap", "Tape Lap in percent; default 20.0"),
    (
        "TSData.tapelayer",
        "Tape shield thickness; same units as radius; no default.",
    ),
    (
        "TShape.action",
        "{DblSave | SngSave} After defining temperature curve data... Setting action=DblSave or SngSave will cause the present \"Temp\" values to be written to either a packed file of double or single. The filename is the Tshape name. ",
    ),
    (
        "TShape.csvfile",
        "Switch input of  temperature curve data to a csv file containing (hour, Temp) points, or simply (Temp) values for fixed time interval data, one per line. NOTE: This action may reset the number of points to a lower value.",
    ),
    (
        "TShape.dblfile",
        "Switch input of  temperature curve data to a binary file of doubles containing (hour, Temp) points, or simply (Temp) values for fixed time interval data, packed one after another. NOTE: This action may reset the number of points to a lower value.",
    ),
    (
        "TShape.hour",
        "Array of hour values. Only necessary to define this property for variable interval data. If the data are fixed interval, do not use this property. You can also use the syntax: \nhour = (file=filename)     !for text file one value per line\nhour = (dblfile=filename)  !for packed file of doubles\nhour = (sngfile=filename)  !for packed file of singles ",
    ),
    (
        "TShape.interval",
        "Time interval for fixed interval data, hrs. Default = 1. If Interval = 0 then time data (in hours) may be at irregular intervals and time value must be specified using either the Hour property or input files. Then values are interpolated when Interval=0, but not for fixed interval data.  \n\nSee also \"sinterval\" and \"minterval\".",
    ),
    (
        "TShape.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "TShape.mean",
        "Mean of the temperature curve values.  This is computed on demand the first time a value is needed.  However, you may set it to another value independently. Used for Monte Carlo load simulations.",
    ),
    (
        "TShape.minterval",
        "Specify fixed interval in MINUTES. Alternate way to specify Interval property.",
    ),
    (
        "TShape.npts",
        "Max number of points to expect in temperature shape vectors. This gets reset to the number of Temperature values found if less than specified.",
    ),
    (
        "TShape.sinterval",
        "Specify fixed interval in SECONDS. Alternate way to specify Interval property.",
    ),
    (
        "TShape.sngfile",
        "Switch input of  temperature curve data to a binary file of singles containing (hour, Temp) points, or simply (Temp) values for fixed time interval data, packed one after another. NOTE: This action may reset the number of points to a lower value.",
    ),
    (
        "TShape.stddev",
        "Standard deviation of the temperatures.  This is computed on demand the first time a value is needed.  However, you may set it to another value independently.Is overwritten if you subsequently read in a curve\n\nUsed for Monte Carlo load simulations.",
    ),
    (
        "TShape.temp",
        "Array of temperature values.  Units should be compatible with the object using the data. You can also use the syntax: \nTemp = (file=filename)     !for text file one value per line\nTemp = (dblfile=filename)  !for packed file of doubles\nTemp = (sngfile=filename)  !for packed file of singles \n\nNote: this property will reset Npts if the  number of values in the files are fewer.",
    ),
    (
        "Transformer.%imag",
        "Percent magnetizing current. Default=0.0. Magnetizing branch is in parallel with windings in each phase. Also, see \"ppm_antifloat\".",
    ),
    (
        "Transformer.%loadloss",
        "Percent load loss at full load. The %R of the High and Low windings (1 and 2) are adjusted to agree at rated kVA loading.",
    ),
    (
        "Transformer.%noloadloss",
        "Percent no load losses at rated excitatation voltage. Default is 0. Converts to a resistance in parallel with the magnetizing impedance in each winding.",
    ),
    (
        "Transformer.%r",
        "Percent resistance this winding.  (half of total for a 2-winding).",
    ),
    (
        "Transformer.%rs",
        "Use this property to specify all the winding %resistances using an array. Example:\n\nNew Transformer.T1 buses=\"Hibus, lowbus\" ~ %Rs=(0.2  0.3)",
    ),
    (
        "Transformer.bank",
        "Name of the bank this transformer is part of, for CIM, MultiSpeak, and other interfaces.",
    ),
    ("Transformer.basefreq", "Base Frequency for ratings."),
    ("Transformer.bus", "Bus connection spec for this winding."),
    (
        "Transformer.buses",
        "Use this to specify all the bus connections at once using an array. Example:\n\nNew Transformer.T1 buses=\"Hibus, lowbus\"",
    ),
    (
        "Transformer.conn",
        "Connection of this winding {wye*, Delta, LN, LL}. Default is \"wye\" with the neutral solidly grounded. ",
    ),
    (
        "Transformer.conns",
        "Use this to specify all the Winding connections at once using an array. Example:\n\nNew Transformer.T1 buses=\"Hibus, lowbus\" ~ conns=(delta, wye)",
    ),
    (
        "Transformer.core",
        "{Shell*|5-leg|3-Leg|1-phase|core-1-phase|4-leg} Core Type. Used for GIC analysis",
    ),
    ("Transformer.emergamps", "Maximum or emerg current."),
    (
        "Transformer.emerghkva",
        "Emergency (contingency)  kVA rating of H winding (winding 1).  Usually 140% - 150% of maximum nameplate rating, depending on load shape. Defaults to 150% of kVA rating of Winding 1.",
    ),
    (
        "Transformer.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    ("Transformer.faultrate", "Failure rate per year."),
    (
        "Transformer.flrise",
        "Temperature rise, deg C, for full load.  Default is 65.",
    ),
    (
        "Transformer.hsrise",
        "Hot spot temperature rise, deg C.  Default is 15.",
    ),
    (
        "Transformer.kv",
        "For 2-or 3-phase, enter phase-phase kV rating.  Otherwise, kV rating of the actual winding",
    ),
    (
        "Transformer.kva",
        "Base kVA rating of the winding. Side effect: forces change of max normal and emerg kVA ratings.If 2-winding transformer, forces other winding to same value. When winding 1 is defined, all other windings are defaulted to the same rating and the first two winding resistances are defaulted to the %loadloss value.",
    ),
    (
        "Transformer.kvas",
        "Use this to specify the kVA ratings of all windings at once using an array.",
    ),
    (
        "Transformer.kvs",
        "Use this to specify the kV ratings of all windings at once using an array. Example:\n\nNew Transformer.T1 buses=\"Hibus, lowbus\" \n~ conns=(delta, wye)\n~ kvs=(115, 12.47)\n\nSee kV= property for voltage rules.",
    ),
    (
        "Transformer.leadlag",
        "{Lead | Lag (default) | ANSI (default) | Euro } Designation in mixed Delta-wye connections the relationship between HV to LV winding. Default is ANSI 30 deg lag, e.g., Dy1 of Yd1 vector group. To get typical European Dy11 connection, specify either \"lead\" or \"Euro\"",
    ),
    (
        "Transformer.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Transformer.m",
        "m Exponent for thermal properties in IEEE C57.  Typically 0.9 - 1.0",
    ),
    (
        "Transformer.maxtap",
        "Max per unit tap for the active winding.  Default is 1.10",
    ),
    (
        "Transformer.mintap",
        "Min per unit tap for the active winding.  Default is 0.90",
    ),
    (
        "Transformer.n",
        "n Exponent for thermal properties in IEEE C57.  Typically 0.8.",
    ),
    ("Transformer.normamps", "Normal rated current."),
    (
        "Transformer.normhkva",
        "Normal maximum kVA rating of H winding (winding 1).  Usually 100% - 110% of maximum nameplate rating, depending on load shape. Defaults to 110% of kVA rating of Winding 1.",
    ),
    (
        "Transformer.numtaps",
        "Total number of taps between min and max tap.  Default is 32 (16 raise and 16 lower taps about the neutral position). The neutral position is not counted.",
    ),
    (
        "Transformer.pctperm",
        "Percent of failures that become permanent.",
    ),
    (
        "Transformer.phases",
        "Number of phases this transformer. Default is 3.",
    ),
    (
        "Transformer.ppm_antifloat",
        "Default=1 ppm.  Parts per million of transformer winding VA rating connected to ground to protect against accidentally floating a winding without a reference. If positive then the effect is adding a very large reactance to ground.  If negative, then a capacitor.",
    ),
    (
        "Transformer.ratings",
        "An array of ratings to be used when the seasonal ratings flag is True. It can be used to insert\nmultiple ratings to change during a QSTS simulation to evaluate different ratings in transformers. Is given in kVA",
    ),
    (
        "Transformer.rdcohms",
        "Winding dc resistance in OHMS. Useful for GIC analysis. From transformer test report. Defaults to 85% of %R property",
    ),
    ("Transformer.repair", "Hours to repair."),
    (
        "Transformer.rneut",
        "Default = -1. Neutral resistance of wye (star)-connected winding in actual ohms. If entered as a negative value, the neutral is assumed to be open, or floating. To solidly ground the neutral, connect the neutral conductor to Node 0 in the Bus property spec for this winding. For example: Bus=MyBusName.1.2.3.0, which is generally the default connection.",
    ),
    (
        "Transformer.seasons",
        "Defines the number of ratings to be defined for the transfomer, to be used only when defining seasonal ratings using the \"Ratings\" property.",
    ),
    (
        "Transformer.sub",
        "={Yes|No}  Designates whether this transformer is to be considered a substation.Default is No.",
    ),
    (
        "Transformer.subname",
        "Substation Name. Optional. Default is null. If specified, printed on plots",
    ),
    ("Transformer.tap", "Per unit tap that this winding is on."),
    (
        "Transformer.taps",
        "Use this to specify the p.u. tap of all windings at once using an array.",
    ),
    (
        "Transformer.thermal",
        "Thermal time constant of the transformer in hours.  Typically about 2.",
    ),
    (
        "Transformer.wdg",
        "Set this = to the number of the winding you wish to define.  Then set the values for this winding.  Repeat for each winding.  Alternatively, use the array collections (buses, kVAs, etc.) to define the windings.  Note: reactances are BETWEEN pairs of windings; they are not the property of a single winding.",
    ),
    (
        "Transformer.wdgcurrents",
        "(Read only) Makes winding currents available via return on query (? Transformer.TX.WdgCurrents). Order: Phase 1, Wdg 1, Wdg 2, ..., Phase 2 ...\n\nWARNING: If the transformer has open terminal(s), results may be wrong, i.e. avoid using this in those situations. For more information, see https://github.com/dss-extensions/dss-extensions/issues/24",
    ),
    (
        "Transformer.windings",
        "Number of windings, this transformers. (Also is the number of terminals) Default is 2. This property triggers memory allocation for the Transformer and will cause other properties to revert to default values.",
    ),
    (
        "Transformer.x12",
        "Alternative to XHL for specifying the percent reactance from winding 1 to winding 2.  Use for 2- or 3-winding transformers. Percent on the kVA base of winding 1. ",
    ),
    (
        "Transformer.x13",
        "Alternative to XHT for specifying the percent reactance from winding 1 to winding 3.  Use for 3-winding transformers only. Percent on the kVA base of winding 1. ",
    ),
    (
        "Transformer.x23",
        "Alternative to XLT for specifying the percent reactance from winding 2 to winding 3.Use for 3-winding transformers only. Percent on the kVA base of winding 1.  ",
    ),
    (
        "Transformer.xfmrcode",
        "Name of a library entry for transformer properties. The named XfmrCode must already be defined.",
    ),
    (
        "Transformer.xhl",
        "Use this to specify the percent reactance, H-L (winding 1 to winding 2).  Use for 2- or 3-winding transformers. On the kVA base of winding 1. See also X12.",
    ),
    (
        "Transformer.xht",
        "Use this to specify the percent reactance, H-T (winding 1 to winding 3).  Use for 3-winding transformers only. On the kVA base of winding 1. See also X13.",
    ),
    (
        "Transformer.xlt",
        "Use this to specify the percent reactance, L-T (winding 2 to winding 3).  Use for 3-winding transformers only. On the kVA base of winding 1.  See also X23.",
    ),
    (
        "Transformer.xneut",
        "Neutral reactance of wye(star)-connected winding in actual ohms.  May be + or -.",
    ),
    (
        "Transformer.xrconst",
        "={Yes|No} Default is NO. Signifies whether or not the X/R is assumed contant for harmonic studies.",
    ),
    (
        "Transformer.xscarray",
        "Use this to specify the percent reactance between all pairs of windings as an array. All values are on the kVA base of winding 1.  The order of the values is as follows:\n\n(x12 13 14... 23 24.. 34 ..)  \n\nThere will be n(n-1)/2 values, where n=number of windings.",
    ),
    ("UPFC.basefreq", "Base Frequency for ratings."),
    (
        "UPFC.bus1",
        "Name of bus to which the input terminal (1) is connected.\nbus1=busname.1.3\nbus1=busname.1.2.3",
    ),
    (
        "UPFC.bus2",
        "Name of bus to which the output terminal (2) is connected.\nbus2=busname.1.2\nbus2=busname.1.2.3",
    ),
    (
        "UPFC.climit",
        "Current Limit for the UPFC, if the current passing through the UPFC is higher than this value the UPFC turns off. This value is specified in Amps (Default 265 A)",
    ),
    (
        "UPFC.element",
        "The name of the PD element monitored when operating with reactive power compensation. Normally, it should be the PD element immediately upstream the UPFC. The element must be defined including the class, e.g. Line.myline.",
    ),
    (
        "UPFC.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "UPFC.frequency",
        "UPFC working frequency.  Defaults to system default base frequency.",
    ),
    (
        "UPFC.kvarlimit",
        "Maximum amount of reactive power (kvar) that can be absorbed by the UPFC (Default = 5)",
    ),
    (
        "UPFC.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "UPFC.losscurve",
        "Name of the XYCurve for describing the losses behavior as a function of the voltage at the input of the UPFC",
    ),
    (
        "UPFC.mode",
        "Integer used to define the control mode of the UPFC: \n\n0 = Off, \n1 = Voltage regulator, \n2 = Phase angle regulator, \n3 = Dual mode\n4 = It is a control mode where the user can set two different set points to create a secure GAP, these references must be defined in the parameters RefkV and RefkV2. The only restriction when setting these values is that RefkV must be higher than RefkV2. \n5 = In this mode the user can define the same GAP using two set points as in control mode 4. The only difference between mode 5 and mode 4 is that in mode 5, the UPFC controller performs dual control actions just as in control mode 3",
    ),
    ("UPFC.pf", "Power factor target at the input terminal."),
    (
        "UPFC.phases",
        "Number of phases.  Defaults to 1 phase (2 terminals, 1 conductor per terminal).",
    ),
    (
        "UPFC.refkv",
        "Base Voltage expected at the output of the UPFC\n\n\"refkv=0.24\"",
    ),
    (
        "UPFC.refkv2",
        "Base Voltage expected at the output of the UPFC for control modes 4 and 5.\n\nThis reference must be lower than refkv, see control modes 4 and 5 for details",
    ),
    (
        "UPFC.spectrum",
        "Name of harmonic spectrum for this source.  Default is \"defaultUPFC\", which is defined when the DSS starts.",
    ),
    (
        "UPFC.tol1",
        "Tolerance in pu for the series PI controller\nTol1=0.02 is the format used to define 2% tolerance (Default=2%)",
    ),
    (
        "UPFC.vhlimit",
        "High limit for the voltage at the input of the UPFC, if the voltage is above this value the UPFC turns off. This value is specified in Volts (default 300 V)",
    ),
    (
        "UPFC.vllimit",
        "low limit for the voltage at the input of the UPFC, if voltage is below this value the UPFC turns off. This value is specified in Volts (default 125 V)",
    ),
    (
        "UPFC.vpqmax",
        "Maximum voltage (in volts) delivered by the series voltage source (Default = 24 V)",
    ),
    (
        "UPFC.xs",
        "Reactance of the series transformer of the UPFC, ohms (default=0.7540 ... 2 mH)",
    ),
    ("UPFCControl.basefreq", "Base Frequency for ratings."),
    (
        "UPFCControl.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "UPFCControl.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "UPFCControl.upfclist",
        "The list of all the UPFC devices to be controlled by this controller, If left empty, this control will apply for all UPFCs in the model.",
    ),
    ("VCCS.basefreq", "Base Frequency for ratings."),
    (
        "VCCS.bp1",
        "XYCurve defining the input piece-wise linear block.",
    ),
    (
        "VCCS.bp2",
        "XYCurve defining the output piece-wise linear block.",
    ),
    (
        "VCCS.bus1",
        "Name of bus to which source is connected.\nbus1=busname\nbus1=busname.1.2.3",
    ),
    (
        "VCCS.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "VCCS.filter",
        "XYCurve defining the digital filter coefficients (x numerator, y denominator).",
    ),
    (
        "VCCS.fsample",
        "Sample frequency [Hz} for the digital filter.",
    ),
    (
        "VCCS.imaxpu",
        "Maximum output current in per-unit of rated; defaults to 1.1",
    ),
    (
        "VCCS.irmstau",
        "Time constant in producing Irms from the PLL; defaults to 0.0015",
    ),
    (
        "VCCS.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    ("VCCS.phases", "Number of phases.  Defaults to 1."),
    (
        "VCCS.ppct",
        "Steady-state operating output, in percent of rated.",
    ),
    ("VCCS.prated", "Total rated power, in Watts."),
    (
        "VCCS.rmsmode",
        "True if only Hz is used to represent a phase-locked loop (PLL), ignoring the BP1, BP2 and time-domain transformations. Default is no.",
    ),
    (
        "VCCS.spectrum",
        "Harmonic spectrum assumed for this source.  Default is \"default\".",
    ),
    ("VCCS.vrated", "Rated line-to-line voltage, in Volts"),
    (
        "VCCS.vrmstau",
        "Time constant in sensing Vrms for the PLL; defaults to 0.0015",
    ),
    ("VSConverter.basefreq", "Base Frequency for ratings."),
    (
        "VSConverter.bus1",
        "Name of converter bus, containing both AC and DC conductors. Bus2 is always ground.",
    ),
    (
        "VSConverter.d0",
        "Fixed or initial value of the power angle in degrees. Default is 0.",
    ),
    (
        "VSConverter.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "VSConverter.iacmax",
        "Maximum value of AC line current, per-unit of nominal. Default is 2.",
    ),
    (
        "VSConverter.idcmax",
        "Maximum value of DC current, per-unit of nominal. Default is 2.",
    ),
    (
        "VSConverter.kvac",
        "Nominal AC line-neutral voltage in kV. Must be specified > 0.",
    ),
    (
        "VSConverter.kvdc",
        "Nominal DC voltage in kV. Must be specified > 0.",
    ),
    (
        "VSConverter.kw",
        "Nominal converter power in kW. Must be specified > 0.",
    ),
    (
        "VSConverter.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "VSConverter.m0",
        "Fixed or initial value of the modulation index. Default is 0.5.",
    ),
    (
        "VSConverter.mmax",
        "Maximum value of modulation index. Default is 0.9.",
    ),
    (
        "VSConverter.mmin",
        "Minimum value of modulation index. Default is 0.1.",
    ),
    (
        "VSConverter.ndc",
        "Number of DC conductors. Default is 1. DC conductors numbered after AC phases.",
    ),
    (
        "VSConverter.pacref",
        "Reference total AC real power, Watts. Default is 0.\nApplies to PacVac and PacQac control modes, influencing d.",
    ),
    (
        "VSConverter.phases",
        "Number of AC plus DC conductors. Default is 4. AC phases numbered before DC conductors.",
    ),
    (
        "VSConverter.qacref",
        "Reference total AC reactive power, Vars. Default is 0.\nApplies to PacQac and VdcQac control modes, influencing m.",
    ),
    (
        "VSConverter.rac",
        "AC resistance (ohms) for the converter transformer, plus any series reactors. Default is 0.\nMust be 0 for Vac control mode.",
    ),
    (
        "VSConverter.spectrum",
        "Name of harmonic spectrum for this device.",
    ),
    (
        "VSConverter.vacref",
        "Reference AC line-to-neutral voltage, RMS Volts. Default is 0.\nApplies to PacVac and VdcVac control modes, influencing m.",
    ),
    (
        "VSConverter.vdcref",
        "Reference DC voltage, Volts. Default is 0.\nApplies to VdcVac control mode, influencing d.",
    ),
    (
        "VSConverter.vscmode",
        "Control Mode (Fixed|PacVac|PacQac|VdcVac|VdcQac). Default is Fixed.",
    ),
    (
        "VSConverter.xac",
        "AC reactance (ohms) for the converter transformer, plus any series reactors. Default is 0.\nMust be 0 for Vac control mode. Must be >0 for PacVac, PacQac or VacVdc control mode.",
    ),
    (
        "Vsource.angle",
        "Phase angle in degrees of first phase: e.g.,Angle=10.3",
    ),
    ("Vsource.basefreq", "Base Frequency for ratings."),
    (
        "Vsource.basekv",
        "Base Source kV, usually phase-phase (L-L) unless you are making a positive-sequence model or 1-phase modelin which case, it will be phase-neutral (L-N) kV.",
    ),
    (
        "Vsource.basemva",
        "Default value is 100. Base used to convert values specified with puZ1, puZ0, and puZ2 properties to ohms on kV base specified by BasekV property.",
    ),
    (
        "Vsource.bus1",
        "Name of bus to which the main terminal (1) is connected.\nbus1=busname\nbus1=busname.1.2.3\n\nThe VSOURCE object is a two-terminal voltage source (thevenin equivalent). Bus2 defaults to Bus1 with all phases connected to ground (node 0) unless previously specified. This is a Yg connection. If you want something different, define the Bus2 property explicitly.",
    ),
    (
        "Vsource.bus2",
        "Name of bus to which 2nd terminal is connected.\nbus2=busname\nbus2=busname.1.2.3\n\nDefault is Bus1.0.0.0 (grounded wye connection)",
    ),
    (
        "Vsource.daily",
        "LOADSHAPE object to use for the per-unit voltage for DAILY-mode simulations. Set the Mult property of the LOADSHAPE to the pu curve. Qmult is not used. If UseActual=Yes then the Mult curve should be actual L-N kV.\n\nMust be previously defined as a LOADSHAPE object. \n\nSets Yearly curve if it is not already defined.   Set to NONE to reset to no loadshape for Yearly mode. The default is no variation.",
    ),
    (
        "Vsource.duty",
        "LOADSHAPE object to use for the per-unit voltage for DUTYCYCLE-mode simulations. Set the Mult property of the LOADSHAPE to the pu curve. Qmult is not used. If UseActual=Yes then the Mult curve should be actual L-N kV.\n\nMust be previously defined as a LOADSHAPE object. \n\nDefaults to Daily load shape when Daily is defined.   Set to NONE to reset to no loadshape for Yearly mode. The default is no variation.",
    ),
    (
        "Vsource.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "Vsource.frequency",
        "Source frequency.  Defaults to system default base frequency.",
    ),
    (
        "Vsource.isc1",
        "Alternate method of defining the source impedance. \nsingle-phase short circuit current, amps.  Default is 10500.",
    ),
    (
        "Vsource.isc3",
        "Alternate method of defining the source impedance. \n3-phase short circuit current, amps.  Default is 10000.",
    ),
    (
        "Vsource.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "Vsource.model",
        "{Thevenin* | Ideal}  Specifies whether the Vsource is to be considered a Thevenin short circuit model or a quasi-ideal voltage source. If Thevenin, the Vsource uses the impedances defined for all calculations. If \"Ideal\", the model uses a small impedance on the diagonal of the impedance matrix for the fundamental base frequency power flow only. Then switches to actual Thevenin model for other frequencies. ",
    ),
    (
        "Vsource.mvasc1",
        "MVA Short Circuit, 1-phase fault. Default = 2100. The \"single-phase impedance\", Zs, is determined by squaring the base kV and dividing by this value. Then Z0 is determined by Z0 = 3Zs - 2Z1.  For 1-phase sources, Zs is used directly. Use X0R0 to define X/R ratio for 1-phase source.",
    ),
    (
        "Vsource.mvasc3",
        "MVA Short circuit, 3-phase fault. Default = 2000. Z1 is determined by squaring the base kv and dividing by this value. For single-phase source, this value is not used.",
    ),
    ("Vsource.phases", "Number of phases.  Defaults to 3."),
    (
        "Vsource.pu",
        "Per unit of the base voltage that the source is actually operating at.\n\"pu=1.05\"",
    ),
    (
        "Vsource.puz0",
        "2-element array: e.g., [1  2]. An alternate way to specify Z0. See Z0 property. Per-unit zero-sequence impedance on base of Vsource BasekV and BaseMVA.",
    ),
    (
        "Vsource.puz1",
        "2-element array: e.g., [1  2]. An alternate way to specify Z1. See Z1 property. Per-unit positive-sequence impedance on base of Vsource BasekV and BaseMVA.",
    ),
    (
        "Vsource.puz2",
        "2-element array: e.g., [1  2]. An alternate way to specify Z2. See Z2 property. Per-unit negative-sequence impedance on base of Vsource BasekV and BaseMVA.",
    ),
    (
        "Vsource.puzideal",
        "2-element array: e.g., [1  2]. The pu impedance to use for the quasi-ideal voltage source model. Should be a very small impedances. Default is [1e-6, 0.001]. Per-unit impedance on base of Vsource BasekV and BaseMVA. If too small, solution may not work. Be sure to check the voltage values and powers.",
    ),
    (
        "Vsource.r0",
        "Alternate method of defining the source impedance. \nZero-sequence resistance, ohms.  Default is 1.9.",
    ),
    (
        "Vsource.r1",
        "Alternate method of defining the source impedance. \nPositive-sequence resistance, ohms.  Default is 1.65.",
    ),
    (
        "Vsource.scantype",
        "{pos*| zero | none} Maintain specified sequence for harmonic solution. Default is positive sequence. Otherwise, angle between phases rotates with harmonic.",
    ),
    (
        "Vsource.sequence",
        "{pos*| neg | zero} Set the phase angles for the specified symmetrical component sequence for non-harmonic solution modes. Default is positive sequence. ",
    ),
    (
        "Vsource.spectrum",
        "Name of harmonic spectrum for this source.  Default is \"defaultvsource\", which is defined when the DSS starts.",
    ),
    (
        "Vsource.x0",
        "Alternate method of defining the source impedance. \nZero-sequence reactance, ohms.  Default is 5.7.",
    ),
    ("Vsource.x0r0", "Zero-sequence X/R ratio.Default = 3."),
    (
        "Vsource.x1",
        "Alternate method of defining the source impedance. \nPositive-sequence reactance, ohms.  Default is 6.6.",
    ),
    ("Vsource.x1r1", "Positive-sequence  X/R ratio. Default = 4."),
    (
        "Vsource.yearly",
        "LOADSHAPE object to use for the per-unit voltage for YEARLY-mode simulations. Set the Mult property of the LOADSHAPE to the pu curve. Qmult is not used. If UseActual=Yes then the Mult curve should be actual L-N kV.\n\nMust be previously defined as a LOADSHAPE object. \n\nIs set to the Daily load shape when Daily is defined.  The daily load shape is repeated in this case. Set to NONE to reset to no loadshape for Yearly mode. The default is no variation.",
    ),
    (
        "Vsource.z0",
        "Zero-sequence equivalent source impedance, ohms, as a 2-element array representing a complex number. Example: \n\nZ0=[3, 4]  ! represents 3 + j4 \n\nUsed to define the impedance matrix of the VSOURCE if Z1 is also specified. \n\nNote: Z0 defaults to Z1 if it is not specifically defined. ",
    ),
    (
        "Vsource.z1",
        "Positive-sequence equivalent source impedance, ohms, as a 2-element array representing a complex number. Example: \n\nZ1=[1, 2]  ! represents 1 + j2 \n\nIf defined, Z1, Z2, and Z0 are used to define the impedance matrix of the VSOURCE. Z1 MUST BE DEFINED TO USE THIS OPTION FOR DEFINING THE MATRIX.\n\nSide Effect: Sets Z2 and Z0 to same values unless they were previously defined.",
    ),
    (
        "Vsource.z2",
        "Negative-sequence equivalent source impedance, ohms, as a 2-element array representing a complex number. Example: \n\nZ2=[1, 2]  ! represents 1 + j2 \n\nUsed to define the impedance matrix of the VSOURCE if Z1 is also specified. \n\nNote: Z2 defaults to Z1 if it is not specifically defined. If Z2 is not equal to Z1, the impedance matrix is asymmetrical.",
    ),
    ("WindGen. ", " "),
    (
        "WindGen.balanced",
        "{Yes | No*} Default is No.  For Model=7, force balanced current only for 3-phase WindGens. Force zero- and negative-sequence to zero.",
    ),
    ("WindGen.basefreq", "Base Frequency for ratings."),
    (
        "WindGen.bus1",
        "Bus to which the WindGen is connected.  May include specific node specification.",
    ),
    (
        "WindGen.class",
        "An arbitrary integer number representing the class of WindGen so that WindGen values may be segregated by class.",
    ),
    ("WindGen.conn", "={wye|LN|delta|LL}.  Default is wye."),
    (
        "WindGen.d",
        "Damping constant.  Usual range is 0 to 4. Default is 1.0.  Adjust to get damping",
    ),
    (
        "WindGen.daily",
        "Dispatch shape to use for daily-mode simulations.  Must be previously defined as a Loadshape object of 24 hrs, typically.Set to NONE to reset to no loadshape. ",
    ),
    (
        "WindGen.debugtrace",
        "{Yes | No }  Default is no.  Turn this on to capture the progress of the WindGen model for each iteration.  Creates a separate file for each WindGen named \"GEN_name.csv\".",
    ),
    (
        "WindGen.duty",
        "Load shape to use for duty cycle dispatch simulations such as for wind or solar generation. Must be previously defined as a Loadshape object. Typically would have time intervals less than 1 hr -- perhaps, in seconds. Set to NONE to reset to no loadshape. Designate the number of points to solve using the Set Number=xxxx command. If there are fewer points in the actual shape, the shape is assumed to repeat.",
    ),
    (
        "WindGen.dutystart",
        "Starting time offset [hours] into the duty cycle shape for this WindGen, defaults to 0",
    ),
    (
        "WindGen.enabled",
        "{Yes|No or True|False} Indicates whether this element is enabled.",
    ),
    (
        "WindGen.forceon",
        "{Yes | No}  Forces WindGen ON despite requirements of other dispatch modes. Stays ON until this property is set to NO, or an internal algorithm cancels the forced ON state.",
    ),
    (
        "WindGen.h",
        "Per unit mass constant of the machine.  MW-sec/MVA.  Default is 1.0.",
    ),
    (
        "WindGen.kv",
        "Nominal rated (1.0 per unit) voltage, kV, for WindGen. For 2- and 3-phase WindGens, specify phase-phase kV. Otherwise, for phases=1 or phases>3, specify actual kV across each branch of the WindGen. If wye (star), specify phase-neutral kV. If delta or phase-phase connected, specify phase-phase kV.",
    ),
    (
        "WindGen.kva",
        "kVA rating of electrical machine. Defaults to 1.2* kW if not specified. Applied to machine or inverter definition for Dynamics mode solutions. ",
    ),
    (
        "WindGen.kvar",
        "Specify the base kvar.  Alternative to specifying the power factor.  Side effect:  the power factor value is altered to agree based on present value of kW.",
    ),
    (
        "WindGen.kw",
        "Total base kW for the WindGen.  A positive value denotes power coming OUT of the element, \nwhich is the opposite of a load. This value is modified depending on the dispatch mode. Unaffected by the global load multiplier and growth curves. If you want there to be more generation, you must add more WindGens or change this value.",
    ),
    (
        "WindGen.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "WindGen.maxkvar",
        "Maximum kvar limit for Model = 3.  Defaults to twice the specified load kvar.  Always reset this if you change PF or kvar properties.",
    ),
    (
        "WindGen.minkvar",
        "Minimum kvar limit for Model = 3. Enter a negative number if WindGen can absorb vars. Defaults to negative of Maxkvar.  Always reset this if you change PF or kvar properties.",
    ),
    (
        "WindGen.model",
        "Integer code for the model to use for generation variation with voltage. Valid values are:\n\n1:WindGen injects a constant kW at specified power factor.\n2:WindGen is modeled as a constant admittance.\n3:Const kW, constant kV.  Voltage-regulated model.\n4:Const kW, Fixed Q (Q never varies)\n5:Const kW, Fixed Q(as a constant reactance)\n6:Compute load injection from User-written Model.(see usage of Xd, Xdp)",
    ),
    (
        "WindGen.mva",
        "MVA rating of electrical machine.  Alternative to using kVA=.",
    ),
    (
        "WindGen.pf",
        "WindGen power factor. Default is 0.80. Enter negative for leading powerfactor (when kW and kvar have opposite signs.)\nA positive power factor for a WindGen signifies that the WindGen produces vars \nas is typical for a synchronous WindGen.  Induction machines would be \ngenerally specified with a negative power factor.",
    ),
    (
        "WindGen.phases",
        "Number of Phases, this WindGen.  Power is evenly divided among phases.",
    ),
    (
        "WindGen.pvfactor",
        "Deceleration factor for P-V WindGen model (Model=3).  Default is 0.1. If the circuit converges easily, you may want to use a higher number such as 1.0. Use a lower number if solution diverges. Use Debugtrace=yes to create a file that will trace the convergence of a WindGen model.",
    ),
    (
        "WindGen.shaftdata",
        "String (in quotes or parentheses) that gets passed to user-written shaft dynamic model for defining the data for that model.",
    ),
    (
        "WindGen.shaftmodel",
        "Name of user-written DLL containing a Shaft model, which models the prime mover and determines the power on the shaft for Dynamics studies. Models additional mass elements other than the single-mass model in the DSS default model. Set to \"none\" to negate previous setting.",
    ),
    (
        "WindGen.spectrum",
        "Name of harmonic voltage or current spectrum for this WindGen. Voltage behind Xd\" for machine - default. Current injection for inverter. Default value is \"default\", which is defined when the DSS starts.",
    ),
    (
        "WindGen.status",
        "={Fixed | Variable*}.  If Fixed, then dispatch multipliers do not apply. The WindGen is always at full power when it is ON.  Default is Variable  (follows curves or windspeed).",
    ),
    (
        "WindGen.userdata",
        "String (in quotes or parentheses) that gets passed to user-written model for defining the data required for that model.",
    ),
    (
        "WindGen.usermodel",
        "Name of DLL containing user-written model, which computes the terminal currents for Dynamics studies, overriding the default model.  Set to \"none\" to negate previous setting.",
    ),
    (
        "WindGen.vmaxpu",
        "Default = 1.10.  Maximum per unit voltage for which the Model is assumed to apply. Above this value, the Windgen model reverts to a constant impedance model.",
    ),
    (
        "WindGen.vminpu",
        "Default = 0.90.  Minimum per unit voltage for which the Model is assumed to apply. Below this value, the Windgen model reverts to a constant impedance model. For model 7, the current is limited to the value computed for constant power at Vminpu.",
    ),
    (
        "WindGen.vpu",
        "Per Unit voltage set point for Model = 3  (Regulated voltage model).  Default is 1.0 pu. ",
    ),
    (
        "WindGen.xd",
        "Per unit synchronous reactance of machine. Presently used only for Thevenin impedance for power flow calcs of user models (model=6). Typically use a value 0.4 to 1.0. Default is 1.0",
    ),
    (
        "WindGen.xdp",
        "Per unit transient reactance of the machine.  Used for Dynamics mode and Fault studies.  Default is 0.27.For user models, this value is used for the Thevenin/Norton impedance for Dynamics Mode.",
    ),
    (
        "WindGen.xdpp",
        "Per unit subtransient reactance of the machine.  Used for Harmonics. Default is 0.20.",
    ),
    (
        "WindGen.xrdp",
        "Default is 20. X/R ratio for Xdp property for FaultStudy and Dynamic modes.",
    ),
    (
        "WindGen.yearly",
        "Dispatch shape to use for yearly-mode simulations.  Must be previously defined as a Loadshape object. If this is not specified, a constant value is assumed (no variation). Set to NONE to reset to no loadshape. Nominally for 8760 simulations.  If there are fewer points in the designated shape than the number of points in the solution, the curve is repeated.",
    ),
    (
        "WireData.capradius",
        "Equivalent conductor radius for capacitance calcs. Specify this for bundled conductors. Defaults to same value as radius. Define Diam or Radius property first.",
    ),
    (
        "WireData.diam",
        "Diameter; Alternative method for entering radius.",
    ),
    (
        "WireData.emergamps",
        "Emergency ampacity, amperes. Defaults to 1.5 * Normal Amps if not specified.",
    ),
    (
        "WireData.gmrac",
        "GMR at 60 Hz. Defaults to .7788*radius if not specified.",
    ),
    (
        "WireData.gmrunits",
        "Units for GMR: {mi|kft|km|m|Ft|in|cm|mm} Default=none.",
    ),
    (
        "WireData.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "WireData.normamps",
        "Normal ampacity, amperes. Defaults to Emergency amps/1.5 if not specified.",
    ),
    (
        "WireData.rac",
        "Resistance at 60 Hz per unit length. Defaults to 1.02*Rdc if not specified.",
    ),
    (
        "WireData.radius",
        "Outside radius of conductor. Defaults to GMR/0.7788 if not specified.",
    ),
    (
        "WireData.radunits",
        "Units for outside radius: {mi|kft|km|m|Ft|in|cm|mm} Default=none.",
    ),
    (
        "WireData.ratings",
        "An array of ratings to be used when the seasonal ratings flag is True. It can be used to insert\nmultiple ratings to change during a QSTS simulation to evaluate different ratings in lines.",
    ),
    (
        "WireData.rdc",
        "dc Resistance, ohms per unit length (see Runits). Defaults to Rac/1.02 if not specified.",
    ),
    (
        "WireData.runits",
        "Length units for resistance: ohms per {mi|kft|km|m|Ft|in|cm|mm} Default=none.",
    ),
    (
        "WireData.seasons",
        "Defines the number of ratings to be defined for the wire, to be used only when defining seasonal ratings using the \"Ratings\" property.",
    ),
    (
        "XYcurve.csvfile",
        "Switch input of  X-Y curve data to a CSV file containing X, Y points one per line. NOTE: This action may reset the number of points to a lower value.",
    ),
    (
        "XYcurve.dblfile",
        "Switch input of  X-Y  curve data to a binary file of DOUBLES containing X, Y points packed one after another. NOTE: This action may reset the number of points to a lower value.",
    ),
    (
        "XYcurve.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "XYcurve.npts",
        "Max number of points to expect in curve. This could get reset to the actual number of points defined if less than specified.",
    ),
    (
        "XYcurve.points",
        "One way to enter the points in a curve. Enter x and y values as one array in the order [x1, y1, x2, y2, ...]. For example:\n\nPoints=[1,100 2,200 3, 300] \n\nValues separated by commas or white space. Zero fills arrays if insufficient number of values.",
    ),
    (
        "XYcurve.sngfile",
        "Switch input of  X-Y curve data to a binary file of SINGLES containing X, Y points packed one after another. NOTE: This action may reset the number of points to a lower value.",
    ),
    (
        "XYcurve.x",
        "Enter a value and then retrieve the interpolated Y value from the Y property. On input shifted then scaled to original curve. Scaled then shifted on output.",
    ),
    (
        "XYcurve.xarray",
        "Alternate way to enter X values. Enter an array of X values corresponding to the Y values.  You can also use the syntax: \nXarray = (file=filename)     !for text file one value per line\nXarray = (dblfile=filename)  !for packed file of doubles\nXarray = (sngfile=filename)  !for packed file of singles \n\nNote: this property will reset Npts to a smaller value if the  number of values in the files are fewer.",
    ),
    (
        "XYcurve.xscale",
        "Scale X property values (in/out) by this factor. Default = 1.0. Does not change original definition of arrays.",
    ),
    (
        "XYcurve.xshift",
        "Shift X property values (in/out) by this amount of offset. Default = 0. Does not change original definition of arrays.",
    ),
    (
        "XYcurve.y",
        "Enter a value and then retrieve the interpolated X value from the X property. On input shifted then scaled to original curve. Scaled then shifted on output.",
    ),
    (
        "XYcurve.yarray",
        "Alternate way to enter Y values. Enter an array of Y values corresponding to the X values.  You can also use the syntax: \nYarray = (file=filename)     !for text file one value per line\nYarray = (dblfile=filename)  !for packed file of doubles\nYarray = (sngfile=filename)  !for packed file of singles \n\nNote: this property will reset Npts to a smaller value if the  number of values in the files are fewer.",
    ),
    (
        "XYcurve.yscale",
        "Scale Y property values (in/out) by this factor. Default = 1.0. Does not change original definition of arrays.",
    ),
    (
        "XYcurve.yshift",
        "Shift Y property values (in/out) by this amount of offset. Default = 0. Does not change original definition of arrays.",
    ),
    (
        "XfmrCode.%imag",
        "Percent magnetizing current. Default=0.0. Magnetizing branch is in parallel with windings in each phase. Also, see \"ppm_antifloat\".",
    ),
    (
        "XfmrCode.%loadloss",
        "Percent load loss at full load. The %R of the High and Low windings (1 and 2) are adjusted to agree at rated kVA loading.",
    ),
    (
        "XfmrCode.%noloadloss",
        "Percent no load losses at rated excitation voltage. Default is 0. Converts to a resistance in parallel with the magnetizing impedance in each winding.",
    ),
    (
        "XfmrCode.%r",
        "Percent resistance this winding.  (half of total for a 2-winding).",
    ),
    (
        "XfmrCode.%rs",
        "Use this property to specify all the winding %resistances using an array. Example:\n\nNew Transformer.T1 buses=\"Hibus, lowbus\" ~ %Rs=(0.2  0.3)",
    ),
    (
        "XfmrCode.conn",
        "Connection of this winding. Default is \"wye\" with the neutral solidly grounded.",
    ),
    (
        "XfmrCode.conns",
        "Use this to specify all the Winding connections at once using an array. Example:\n\nNew Transformer.T1 buses=\"Hibus, lowbus\"\n~ conns=(delta, wye)",
    ),
    (
        "XfmrCode.emerghkva",
        "Emergency (contingency)  kVA rating of H winding (winding 1).  Usually 140% - 150% of maximum nameplate rating, depending on load shape. Defaults to 150% of kVA rating of Winding 1.",
    ),
    (
        "XfmrCode.flrise",
        "Temperature rise, deg C, for full load.  Default is 65.",
    ),
    (
        "XfmrCode.hsrise",
        "Hot spot temperature rise, deg C.  Default is 15.",
    ),
    (
        "XfmrCode.kv",
        "For 2-or 3-phase, enter phase-phase kV rating.  Otherwise, kV rating of the actual winding",
    ),
    (
        "XfmrCode.kva",
        "Base kVA rating of the winding. Side effect: forces change of max normal and emerg kva ratings.If 2-winding transformer, forces other winding to same value. When winding 1 is defined, all other windings are defaulted to the same rating and the first two winding resistances are defaulted to the %loadloss value.",
    ),
    (
        "XfmrCode.kvas",
        "Use this to specify the kVA ratings of all windings at once using an array.",
    ),
    (
        "XfmrCode.kvs",
        "Use this to specify the kV ratings of all windings at once using an array. Example:\n\nNew Transformer.T1 buses=\"Hibus, lowbus\" \n~ conns=(delta, wye)\n~ kvs=(115, 12.47)\n\nSee kV= property for voltage rules.",
    ),
    (
        "XfmrCode.like",
        "Make like another object, e.g.:\n\nNew Capacitor.C2 like=c1  ...",
    ),
    (
        "XfmrCode.m",
        "m Exponent for thermal properties in IEEE C57.  Typically 0.9 - 1.0",
    ),
    (
        "XfmrCode.maxtap",
        "Max per unit tap for the active winding.  Default is 1.10",
    ),
    (
        "XfmrCode.mintap",
        "Min per unit tap for the active winding.  Default is 0.90",
    ),
    (
        "XfmrCode.n",
        "n Exponent for thermal properties in IEEE C57.  Typically 0.8.",
    ),
    (
        "XfmrCode.normhkva",
        "Normal maximum kVA rating of H winding (winding 1).  Usually 100% - 110% of maximum nameplate rating, depending on load shape. Defaults to 110% of kVA rating of Winding 1.",
    ),
    (
        "XfmrCode.numtaps",
        "Total number of taps between min and max tap.  Default is 32.",
    ),
    (
        "XfmrCode.phases",
        "Number of phases this transformer. Default is 3.",
    ),
    (
        "XfmrCode.ppm_antifloat",
        "Default=1 ppm.  Parts per million of transformer winding VA rating connected to ground to protect against accidentally floating a winding without a reference. If positive then the effect is adding a very large reactance to ground.  If negative, then a capacitor.",
    ),
    (
        "XfmrCode.ratings",
        "An array of ratings to be used when the seasonal ratings flag is True. It can be used to insert\nmultiple ratings to change during a QSTS simulation to evaluate different ratings in transformers.",
    ),
    (
        "XfmrCode.rdcohms",
        "Winding dc resistance in OHMS. Useful for GIC analysis. From transformer test report. Defaults to 85% of %R property",
    ),
    (
        "XfmrCode.rneut",
        "Default = -1. Neutral resistance of wye (star)-connected winding in actual ohms.If entered as a negative value, the neutral is assumed to be open, or floating.",
    ),
    (
        "XfmrCode.seasons",
        "Defines the number of ratings to be defined for the transfomer, to be used only when defining seasonal ratings using the \"Ratings\" property.",
    ),
    (
        "XfmrCode.tap",
        "Per unit tap that this winding is normally on.",
    ),
    (
        "XfmrCode.taps",
        "Use this to specify the normal p.u. tap of all windings at once using an array.",
    ),
    (
        "XfmrCode.thermal",
        "Thermal time constant of the transformer in hours.  Typically about 2.",
    ),
    (
        "XfmrCode.wdg",
        "Set this = to the number of the winding you wish to define.  Then set the values for this winding.  Repeat for each winding.  Alternatively, use the array collections (buses, kvas, etc.) to define the windings.  Note: reactances are BETWEEN pairs of windings; they are not the property of a single winding.",
    ),
    (
        "XfmrCode.windings",
        "Number of windings, this transformers. (Also is the number of terminals) Default is 2. This property triggers memory allocation for the Transformer and will cause other properties to revert to default values.",
    ),
    (
        "XfmrCode.x12",
        "Alternative to XHL for specifying the percent reactance from winding 1 to winding 2.  Use for 2- or 3-winding transformers. Percent on the kVA base of winding 1. ",
    ),
    (
        "XfmrCode.x13",
        "Alternative to XHT for specifying the percent reactance from winding 1 to winding 3.  Use for 3-winding transformers only. Percent on the kVA base of winding 1. ",
    ),
    (
        "XfmrCode.x23",
        "Alternative to XLT for specifying the percent reactance from winding 2 to winding 3.Use for 3-winding transformers only. Percent on the kVA base of winding 1.  ",
    ),
    (
        "XfmrCode.xhl",
        "Use this to specify the percent reactance, H-L (winding 1 to winding 2).  Use for 2- or 3-winding transformers. On the kva base of winding 1.",
    ),
    (
        "XfmrCode.xht",
        "Use this to specify the percent reactance, H-T (winding 1 to winding 3).  Use for 3-winding transformers only. On the kVA base of winding 1.",
    ),
    (
        "XfmrCode.xlt",
        "Use this to specify the percent reactance, L-T (winding 2 to winding 3).  Use for 3-winding transformers only. On the kVA base of winding 1.",
    ),
    (
        "XfmrCode.xneut",
        "Neutral reactance of wye(star)-connected winding in actual ohms.  May be + or -.",
    ),
    (
        "XfmrCode.xscarray",
        "Use this to specify the percent reactance between all pairs of windings as an array. All values are on the kVA base of winding 1.  The order of the values is as follows:\n\n(x12 13 14... 23 24.. 34 ..)  \n\nThere will be n(n-1)/2 values, where n=number of windings.",
    ),
];

/// Pascal `DSSGlobals.DSSHelp` (`DSSGlobals.pas:717-727`) over the loaded
/// catalog: the help string for `key`, or **the key itself** on a miss (the
/// gettext `Translate` returns empty → `Result := s`). `GetPropertyHelp`'s
/// ClassParents fallback (`DSSClass.pas:2191-2197`) is provably dead against
/// this catalog — it contains no parent-class-prefixed key (asserted at
/// generation) — so own-key-or-miss is the complete lookup.
pub fn dss_help(key: &str) -> &str {
    match HELP_CATALOG.binary_search_by(|(k, _)| (*k).cmp(key)) {
        Ok(i) => HELP_CATALOG[i].1,
        Err(_) => key,
    }
}
