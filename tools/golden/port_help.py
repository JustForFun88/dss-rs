"""Help texts the engine states differently from the pinned wheel's catalog.

`gen_help_catalog.py` applies them after the r4133 supplement, so a
regeneration reproduces the committed `help_catalog.rs`. Each entry states a
rule the engine enforces where the wheel's text contradicts it.
"""

PORT_HELP: dict[str, str] = {
    # Only cond=N selects a conductor, and conductor data without one is refused.
    "LineGeometry.cond": (
        "Number of the conductor that the following wire, cncable, tscable, x, h and units "
        "apply to, 1 to NConds. No conductor is selected after New, like= or nconds=, so "
        "conductor data needs cond=N first."
    ),
}
