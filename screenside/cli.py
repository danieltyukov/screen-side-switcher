"""Command line entry point. Useful on its own and for keyboard shortcuts."""

from __future__ import annotations

import argparse
import sys

from . import __version__
from .layout import (
    ALIGNMENTS,
    LEFT,
    RIGHT,
    SIDE_DESCRIPTIONS,
    SIDES,
    CENTER,
    LayoutError,
    compute,
    current_alignment,
    current_side,
)
from .mutter import DisplayConfig, MutterError


def _describe(state) -> str:
    side = current_side(state)
    lines = []
    for lm in sorted(state.logical_monitors, key=lambda l: (l.x, l.y)):
        connector = lm.connectors[0]
        monitor = state.monitor(connector)
        w, h = state.extent(connector)
        flag = " (primary)" if lm.primary else ""
        label = monitor.label if monitor else connector
        lines.append(
            f"  {connector:<10} {label:<24} {w}x{h} at ({lm.x},{lm.y}) scale {lm.scale:g}{flag}"
        )
    head = (
        f"External screen is {side} of the built-in screen."
        if side
        else "Arrangement is not a simple side-by-side layout."
    )
    align = current_alignment(state)
    if align:
        head += f"  Alignment: {align}."
    return head + "\n" + "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="screen-side",
        description="Choose which side of the laptop screen the external monitor sits on.",
    )
    parser.add_argument(
        "side",
        nargs="?",
        default="status",
        choices=[*SIDES, "status", "toggle"],
        help="Where to put the external screen. 'toggle' flips left/right.",
    )
    parser.add_argument(
        "--align",
        choices=ALIGNMENTS,
        default=None,
        help="How to line the screens up on the other axis (default: keep current, else centred).",
    )
    parser.add_argument(
        "--temporary",
        action="store_true",
        help="Do not save the arrangement; it reverts on the next reconnect.",
    )
    parser.add_argument("--version", action="version", version=__version__)
    args = parser.parse_args(argv)

    try:
        config = DisplayConfig()
        state = config.get_state()
    except MutterError as exc:
        print(f"screen-side: {exc}", file=sys.stderr)
        return 1

    if args.side == "status":
        print(_describe(state))
        return 0

    side = args.side
    if side == "toggle":
        side = RIGHT if current_side(state) == LEFT else LEFT

    alignment = args.align or current_alignment(state) or CENTER

    try:
        monitors = compute(state, side, alignment)
        config.apply(state, monitors, persistent=not args.temporary)
    except (LayoutError, MutterError) as exc:
        print(f"screen-side: {exc}", file=sys.stderr)
        return 1

    print(f"External screen moved {side}. {SIDE_DESCRIPTIONS[side]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
