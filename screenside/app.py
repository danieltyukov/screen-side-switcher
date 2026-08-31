"""GTK4/libadwaita front end: pick a side, the arrangement changes at once."""

from __future__ import annotations

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")

from gi.repository import Adw, Gdk, GLib, Gtk  # noqa: E402

from . import APP_ID, __version__  # noqa: E402
from .layout import (  # noqa: E402
    ABOVE,
    ALIGNMENT_LABELS,
    ALIGNMENTS,
    BELOW,
    CENTER,
    LEFT,
    RIGHT,
    SIDE_DESCRIPTIONS,
    SIDE_LABELS,
    SIDES,
    LayoutError,
    compute,
    current_alignment,
    current_side,
)
from .mutter import DisplayConfig, MutterError  # noqa: E402

SIDE_ICONS = {
    LEFT: "go-previous-symbolic",
    RIGHT: "go-next-symbolic",
    ABOVE: "go-up-symbolic",
    BELOW: "go-down-symbolic",
}


class Preview(Gtk.DrawingArea):
    """Scale drawing of the current arrangement, so the choice is obvious
    before and after clicking."""

    def __init__(self) -> None:
        super().__init__()
        self.set_content_height(150)
        self.boxes: list[tuple[int, int, int, int, str, bool]] = []
        self.set_draw_func(self._draw)

    def update(self, state) -> None:
        self.boxes = []
        for lm in state.logical_monitors:
            connector = lm.connectors[0]
            w, h = state.extent(connector)
            monitor = state.monitor(connector)
            label = connector
            if monitor and monitor.is_builtin:
                label = "Laptop"
            elif monitor:
                label = (monitor.product or monitor.connector).strip()
            self.boxes.append((lm.x, lm.y, w, h, label, bool(lm.primary)))
        self.queue_draw()

    def _draw(self, _area, cr, width, height) -> None:
        if not self.boxes:
            return
        pad = 10
        min_x = min(b[0] for b in self.boxes)
        min_y = min(b[1] for b in self.boxes)
        max_x = max(b[0] + b[2] for b in self.boxes)
        max_y = max(b[1] + b[3] for b in self.boxes)
        span_x = max(max_x - min_x, 1)
        span_y = max(max_y - min_y, 1)
        scale = min((width - 2 * pad) / span_x, (height - 2 * pad) / span_y)
        off_x = (width - span_x * scale) / 2
        off_y = (height - span_y * scale) / 2

        style = self.get_style_context()
        accent = style.lookup_color("accent_bg_color")[1] or Gdk.RGBA()
        fg = style.lookup_color("window_fg_color")[1] or Gdk.RGBA()

        for x, y, w, h, label, primary in self.boxes:
            rx = off_x + (x - min_x) * scale
            ry = off_y + (y - min_y) * scale
            rw = max(w * scale, 2)
            rh = max(h * scale, 2)

            if primary:
                cr.set_source_rgba(accent.red, accent.green, accent.blue, 0.30)
            else:
                cr.set_source_rgba(fg.red, fg.green, fg.blue, 0.10)
            cr.rectangle(rx + 1, ry + 1, rw - 2, rh - 2)
            cr.fill()

            cr.set_source_rgba(fg.red, fg.green, fg.blue, 0.55)
            cr.set_line_width(1.5)
            cr.rectangle(rx + 1, ry + 1, rw - 2, rh - 2)
            cr.stroke()

            cr.set_source_rgba(fg.red, fg.green, fg.blue, 0.9)
            cr.select_font_face("Cantarell")
            cr.set_font_size(11)
            extents = cr.text_extents(label)
            if extents.width < rw - 8:
                cr.move_to(
                    rx + (rw - extents.width) / 2,
                    ry + rh / 2 + extents.height / 2,
                )
                cr.show_text(label)


class Window(Adw.ApplicationWindow):
    def __init__(self, app: Adw.Application) -> None:
        super().__init__(application=app, title="Screen Side")
        self.set_default_size(460, 560)
        self.set_resizable(True)

        self._config: DisplayConfig | None = None
        self._state = None
        self._updating = False
        self._side_buttons: dict[str, Gtk.ToggleButton] = {}
        self._align_buttons: dict[str, Gtk.ToggleButton] = {}

        toolbar = Adw.ToolbarView()
        header = Adw.HeaderBar()
        toolbar.add_top_bar(header)

        self.banner = Adw.Banner(revealed=False)
        toolbar.add_top_bar(self.banner)

        body = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=18)
        body.set_margin_top(18)
        body.set_margin_bottom(18)
        body.set_margin_start(18)
        body.set_margin_end(18)

        frame = Gtk.Frame()
        frame.add_css_class("view")
        self.preview = Preview()
        frame.set_child(self.preview)
        body.append(frame)

        self.summary = Gtk.Label(wrap=True, justify=Gtk.Justification.CENTER)
        self.summary.add_css_class("dim-label")
        body.append(self.summary)

        body.append(self._side_group())
        body.append(self._align_group())

        toolbar.set_content(body)
        self.set_content(toolbar)

        try:
            self._config = DisplayConfig()
            self._config.connect_changed(
                lambda: GLib.idle_add(self.refresh)
            )
        except MutterError as exc:
            self._error(str(exc))

        self.refresh()

    def _side_group(self) -> Gtk.Widget:
        group = Adw.PreferencesGroup(title="External screen position")
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=0, homogeneous=True)
        box.add_css_class("linked")
        for side in SIDES:
            button = Gtk.ToggleButton()
            inner = Gtk.Box(
                orientation=Gtk.Orientation.VERTICAL, spacing=6
            )
            inner.set_margin_top(10)
            inner.set_margin_bottom(10)
            icon = Gtk.Image.new_from_icon_name(SIDE_ICONS[side])
            icon.set_pixel_size(22)
            inner.append(icon)
            inner.append(Gtk.Label(label=SIDE_LABELS[side]))
            button.set_child(inner)
            button.set_tooltip_text(SIDE_DESCRIPTIONS[side])
            button.connect("toggled", self._on_side_toggled, side)
            self._side_buttons[side] = button
            box.append(button)
        group.add(box)
        return group

    def _align_group(self) -> Gtk.Widget:
        group = Adw.PreferencesGroup(
            title="Alignment",
            description="Where the screens line up on the other axis. This decides "
            "how much of the edge the pointer can cross when the screens are "
            "different heights.",
        )
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=0, homogeneous=True)
        box.add_css_class("linked")
        for alignment in ALIGNMENTS:
            button = Gtk.ToggleButton(label=ALIGNMENT_LABELS[alignment])
            button.connect("toggled", self._on_align_toggled, alignment)
            self._align_buttons[alignment] = button
            box.append(button)
        group.add(box)
        return group

    def _error(self, message: str) -> None:
        self.banner.set_title(message)
        self.banner.set_revealed(True)

    def refresh(self) -> bool:
        if self._config is None:
            return False
        try:
            self._state = self._config.get_state()
        except MutterError as exc:
            self._error(str(exc))
            return False

        self.banner.set_revealed(False)
        self.preview.update(self._state)

        side = current_side(self._state)
        alignment = current_alignment(self._state)

        self._updating = True
        for name, button in self._side_buttons.items():
            button.set_active(name == side)
        for name, button in self._align_buttons.items():
            button.set_active(name == alignment)
        self._updating = False

        externals = self._state.externals
        if not externals:
            self.summary.set_text(
                "No external screen is connected. Plug one in to arrange it."
            )
            sensitive = False
        elif side is None:
            self.summary.set_text(
                "The current arrangement is not a simple side-by-side layout. "
                "Pick a side below to set one."
            )
            sensitive = True
        else:
            self.summary.set_text(SIDE_DESCRIPTIONS[side])
            sensitive = True

        for button in self._side_buttons.values():
            button.set_sensitive(sensitive)
        for button in self._align_buttons.values():
            button.set_sensitive(sensitive)
        return False

    def _apply(self, side: str, alignment: str) -> None:
        if self._config is None or self._state is None:
            return
        try:
            monitors = compute(self._state, side, alignment)
            self._config.apply(self._state, monitors, persistent=True)
        except (LayoutError, MutterError) as exc:
            self._error(str(exc))
            self.refresh()
            return
        GLib.timeout_add(250, self.refresh)

    def _on_side_toggled(self, button: Gtk.ToggleButton, side: str) -> None:
        if self._updating or not button.get_active() or self._state is None:
            return
        alignment = current_alignment(self._state) or CENTER
        self._apply(side, alignment)

    def _on_align_toggled(self, button: Gtk.ToggleButton, alignment: str) -> None:
        if self._updating or not button.get_active() or self._state is None:
            return
        side = current_side(self._state)
        if side is None:
            return
        self._apply(side, alignment)


class Application(Adw.Application):
    def __init__(self) -> None:
        super().__init__(application_id=APP_ID)
        self._window: Window | None = None

    def do_activate(self) -> None:
        if self._window is None:
            self._window = Window(self)
        self._window.present()


def main(argv: list[str] | None = None) -> int:
    return Application().run(argv or [])


if __name__ == "__main__":
    raise SystemExit(main())
