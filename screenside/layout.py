"""Turn a human choice ("put the external screen on the left") into the
absolute logical-monitor coordinates that mutter expects.

Mutter rejects arrangements whose monitors overlap, and leaves the pointer
unable to cross where they merely touch at a corner, so positions are computed
as exact adjacency along one axis with a deliberate alignment on the other.
"""

from __future__ import annotations

from .mutter import LogicalMonitor, State

LEFT, RIGHT, ABOVE, BELOW = "left", "right", "above", "below"
SIDES = (LEFT, RIGHT, ABOVE, BELOW)

SIDE_LABELS = {
    LEFT: "Left",
    RIGHT: "Right",
    ABOVE: "Above",
    BELOW: "Below",
}

# Where the pointer leaves the built-in screen to reach the external one.
SIDE_DESCRIPTIONS = {
    LEFT: "Pointer exits the left edge of the laptop and enters the right edge of the external screen.",
    RIGHT: "Pointer exits the right edge of the laptop and enters the left edge of the external screen.",
    ABOVE: "Pointer exits the top edge of the laptop and enters the bottom edge of the external screen.",
    BELOW: "Pointer exits the bottom edge of the laptop and enters the top edge of the external screen.",
}

START, CENTER = "start", "center"
ALIGNMENTS = (CENTER, START)
ALIGNMENT_LABELS = {CENTER: "Centred", START: "Top / left aligned"}


class LayoutError(RuntimeError):
    pass


def _ordered(state: State, side: str) -> list[str]:
    """Connectors in the order they appear along the arrangement axis."""
    builtin = state.builtin
    externals = [m.connector for m in state.externals]
    if not externals:
        raise LayoutError("No external screen is connected.")

    if builtin is not None:
        anchor = builtin.connector
    else:
        # Desktop, or a laptop whose panel is off: anchor on the primary and
        # treat the remaining screens as the ones being moved.
        primary = next((lm for lm in state.logical_monitors if lm.primary), None)
        if primary is None or not primary.connectors:
            raise LayoutError("No primary screen to arrange around.")
        anchor = primary.connectors[0]
        externals = [c for c in externals if c != anchor]
        if not externals:
            raise LayoutError("Only one screen is connected.")

    if side in (LEFT, ABOVE):
        return [*externals, anchor]
    return [anchor, *externals]


def compute(state: State, side: str, alignment: str = CENTER) -> list[LogicalMonitor]:
    if side not in SIDES:
        raise LayoutError(f"Unknown side {side!r}")

    order = _ordered(state, side)
    extents = {c: state.extent(c) for c in order}
    if any(w <= 0 or h <= 0 for w, h in extents.values()):
        raise LayoutError("A screen reported no usable resolution.")

    horizontal = side in (LEFT, RIGHT)
    cross = max(
        (extents[c][1] if horizontal else extents[c][0]) for c in order
    )

    result: list[LogicalMonitor] = []
    cursor = 0
    for connector in order:
        width, height = extents[connector]
        span, thickness = (width, height) if horizontal else (height, width)

        if alignment == CENTER:
            offset = (cross - thickness) // 2
        else:
            offset = 0

        x, y = (cursor, offset) if horizontal else (offset, cursor)
        cursor += span

        existing = state.logical_for(connector)
        result.append(
            LogicalMonitor(
                x=x,
                y=y,
                scale=existing.scale if existing else 1.0,
                transform=existing.transform if existing else 0,
                primary=existing.primary if existing else False,
                connectors=[connector],
            )
        )

    if not any(lm.primary for lm in result):
        result[0].primary = True
    return result


def current_side(state: State) -> str | None:
    """Best guess at the arrangement in force, or None if it is not a simple
    two-screen side-by-side layout."""
    builtin = state.builtin
    externals = state.externals
    if builtin is None or len(externals) != 1 or len(state.logical_monitors) != 2:
        return None

    b = state.logical_for(builtin.connector)
    e = state.logical_for(externals[0].connector)
    if b is None or e is None:
        return None

    bw, bh = state.extent(builtin.connector)
    ew, eh = state.extent(externals[0].connector)

    # Compare overlap on each axis to decide whether the split is horizontal
    # or vertical, then which side the external sits on.
    x_overlap = min(b.x + bw, e.x + ew) - max(b.x, e.x)
    y_overlap = min(b.y + bh, e.y + eh) - max(b.y, e.y)

    if x_overlap <= 0 and y_overlap > 0:
        return LEFT if e.x < b.x else RIGHT
    if y_overlap <= 0 and x_overlap > 0:
        return ABOVE if e.y < b.y else BELOW
    return None


def current_alignment(state: State) -> str | None:
    side = current_side(state)
    if side is None:
        return None
    order = _ordered(state, side)
    horizontal = side in (LEFT, RIGHT)
    offsets = []
    extents = {c: state.extent(c) for c in order}
    cross = max((extents[c][1] if horizontal else extents[c][0]) for c in order)
    for connector in order:
        lm = state.logical_for(connector)
        width, height = extents[connector]
        thickness = height if horizontal else width
        actual = (lm.y if horizontal else lm.x) if lm else 0
        offsets.append((actual, (cross - thickness) // 2))
    if all(a == 0 for a, _ in offsets):
        return START
    if all(a == c for a, c in offsets):
        return CENTER
    return None
