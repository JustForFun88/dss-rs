"""Help texts the engine states differently from the pinned wheel's catalog.

`gen_help_catalog.py` applies them after the r4133 supplement, so a
regeneration reproduces the committed `help_catalog.rs`. Each entry states a
rule the engine enforces where the wheel's text contradicts it.
"""

PORT_HELP: dict[str, str] = {
    # Only cond=N selects a conductor, there is no default, and conductor data
    # without one is refused.
    "LineGeometry.cond": (
        "Number of the conductor that the following wire, cncable, tscable, x, h and units "
        "apply to, 1 to NConds. There is no default: no conductor is selected after New, "
        "like= or nconds=, so conductor data needs cond=N first."
    ),
    # The class reads one exact word of these eight, in any case.
    "LineGeometry.units": (
        "Units for x and h, one of mi, kft, km, m, ft, in, cm or mm, written whole. Initial "
        'default is "ft", but defaults to last unit defined.'
    ),
    # The other unit properties read one exact word of their list, in any case.
    "Line.units": (
        "Length units, one of none, mi, kft, km, m, ft, in, cm or mm, written whole. "
        "Default is None - assumes length units match impedance units."
    ),
    "LineCode.units": (
        "One of (ohms per ...) none, mi, kft, km, m, me, ft, in, cm or mm, written whole, "
        "me reading as m. Default is none, which assumes units agree with length units given "
        "in Line object."
    ),
    "LineSpacing.units": (
        "Units for x and h, one of mi, kft, km, m, ft, in, cm or mm, written whole. Initial "
        'default is "ft", but defaults to last unit defined.'
    ),
    "ShowOption.lineconstants": (
        "Creates two report files for the line constants (impedances) of every LINEGEOMETRY "
        "element currently defined. One file shows the main report with the matrices. The other "
        "file contains corresponding LINECODE definitions that you may use in subsequent "
        "simulations.  Syntax:\n\nShow LIneConstants [frequency] "
        "[none|mi|km|kft|m|me|ft|in|cm|mm] [rho]\n\nSpecify the frequency, length units (one "
        "of the listed words, written whole) and earth resistivity (meter-ohms). Examples:\n\n"
        "Show Lineconstants 60 kft 100\nShow Linecon 50 km 1000"
    ),
}
