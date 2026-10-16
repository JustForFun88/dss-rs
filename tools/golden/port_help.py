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
}
