"""Thin wrapper around the org.gnome.Mutter.DisplayConfig D-Bus interface.

This is the only supported way to rearrange monitors on a Wayland GNOME
session; the X11-era tools (xrandr) cannot do it. The interface is documented
in mutter's data/dbus-interfaces/org.gnome.Mutter.DisplayConfig.xml.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from gi.repository import Gio, GLib

BUS_NAME = "org.gnome.Mutter.DisplayConfig"
OBJECT_PATH = "/org/gnome/Mutter/DisplayConfig"

# ApplyMonitorsConfig "method" argument.
METHOD_VERIFY = 0
METHOD_TEMPORARY = 1
METHOD_PERSISTENT = 2

# Global "layout-mode" property. It decides the coordinate space that logical
# monitor positions live in, so every width/height calculation depends on it.
LAYOUT_MODE_LOGICAL = 1  # positions are in scaled (logical) pixels
LAYOUT_MODE_PHYSICAL = 2  # positions are in raw device pixels

# Transforms that swap width and height.
_ROTATED = (1, 3, 5, 7)


class MutterError(RuntimeError):
    """Raised when mutter refuses a configuration or is unreachable."""


@dataclass
class Mode:
    id: str
    width: int
    height: int
    refresh: float
    preferred_scale: float
    supported_scales: list[float]
    is_current: bool
    is_preferred: bool


@dataclass
class Monitor:
    connector: str
    vendor: str
    product: str
    serial: str
    display_name: str
    is_builtin: bool
    modes: list[Mode] = field(default_factory=list)

    @property
    def current_mode(self) -> Mode | None:
        return next((m for m in self.modes if m.is_current), None)

    @property
    def preferred_mode(self) -> Mode | None:
        return next((m for m in self.modes if m.is_preferred), None)

    @property
    def label(self) -> str:
        return self.display_name or self.product or self.connector


@dataclass
class LogicalMonitor:
    x: int
    y: int
    scale: float
    transform: int
    primary: bool
    connectors: list[str]


@dataclass
class State:
    serial: int
    monitors: list[Monitor]
    logical_monitors: list[LogicalMonitor]
    layout_mode: int

    def monitor(self, connector: str) -> Monitor | None:
        return next((m for m in self.monitors if m.connector == connector), None)

    def logical_for(self, connector: str) -> LogicalMonitor | None:
        return next(
            (lm for lm in self.logical_monitors if connector in lm.connectors), None
        )

    @property
    def builtin(self) -> Monitor | None:
        return next((m for m in self.monitors if m.is_builtin), None)

    @property
    def externals(self) -> list[Monitor]:
        return [m for m in self.monitors if not m.is_builtin]

    def extent(self, connector: str) -> tuple[int, int]:
        """Width and height this monitor occupies in the active coordinate space."""
        monitor = self.monitor(connector)
        logical = self.logical_for(connector)
        if monitor is None or logical is None:
            return (0, 0)
        mode = monitor.current_mode or monitor.preferred_mode
        if mode is None:
            return (0, 0)
        width, height = mode.width, mode.height
        if logical.transform in _ROTATED:
            width, height = height, width
        if self.layout_mode == LAYOUT_MODE_LOGICAL and logical.scale:
            # Round like mutter does, otherwise adjacent monitors leave a
            # one-pixel seam that the pointer cannot cross.
            width = round(width / logical.scale)
            height = round(height / logical.scale)
        return (width, height)


def _is_builtin(connector: str) -> bool:
    c = connector.upper()
    return c.startswith(("EDP", "LVDS", "DSI")) or c.startswith("EDP-")


class DisplayConfig:
    def __init__(self) -> None:
        try:
            self._bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        except GLib.Error as exc:  # pragma: no cover - depends on session
            raise MutterError(f"cannot reach the session bus: {exc.message}") from exc

    def _call(self, method: str, args: GLib.Variant | None) -> GLib.Variant:
        try:
            return self._bus.call_sync(
                BUS_NAME,
                OBJECT_PATH,
                BUS_NAME,
                method,
                args,
                None,
                Gio.DBusCallFlags.NONE,
                -1,
                None,
            )
        except GLib.Error as exc:
            raise MutterError(exc.message) from exc

    def get_state(self) -> State:
        serial, raw_monitors, raw_logical, props = self._call(
            "GetCurrentState", None
        ).unpack()

        monitors: list[Monitor] = []
        for (connector, vendor, product, serial_no), raw_modes, mprops in raw_monitors:
            modes = [
                Mode(
                    id=mid,
                    width=width,
                    height=height,
                    refresh=refresh,
                    preferred_scale=pref_scale,
                    supported_scales=list(scales),
                    is_current=bool(mode_props.get("is-current", False)),
                    is_preferred=bool(mode_props.get("is-preferred", False)),
                )
                for (
                    mid,
                    width,
                    height,
                    refresh,
                    pref_scale,
                    scales,
                    mode_props,
                ) in raw_modes
            ]
            monitors.append(
                Monitor(
                    connector=connector,
                    vendor=vendor,
                    product=product,
                    serial=serial_no,
                    display_name=str(mprops.get("display-name", "") or ""),
                    is_builtin=bool(
                        mprops.get("is-builtin", _is_builtin(connector))
                    ),
                    modes=modes,
                )
            )

        logical = [
            LogicalMonitor(
                x=x,
                y=y,
                scale=scale,
                transform=transform,
                primary=primary,
                connectors=[m[0] for m in mons],
            )
            for x, y, scale, transform, primary, mons, _lprops in raw_logical
        ]

        return State(
            serial=serial,
            monitors=monitors,
            logical_monitors=logical,
            layout_mode=int(props.get("layout-mode", LAYOUT_MODE_LOGICAL)),
        )

    def apply(
        self,
        state: State,
        logical_monitors: list[LogicalMonitor],
        persistent: bool = True,
    ) -> None:
        """Push a new arrangement. Raises MutterError if mutter rejects it."""
        entries = []
        for lm in logical_monitors:
            monitors = []
            for connector in lm.connectors:
                monitor = state.monitor(connector)
                if monitor is None:
                    raise MutterError(f"unknown connector {connector}")
                mode = monitor.current_mode or monitor.preferred_mode
                if mode is None:
                    raise MutterError(f"{connector} reports no usable mode")
                monitors.append((connector, mode.id, {}))
            entries.append(
                (
                    int(lm.x),
                    int(lm.y),
                    float(lm.scale),
                    int(lm.transform),
                    bool(lm.primary),
                    monitors,
                )
            )

        args = GLib.Variant(
            "(uua(iiduba(ssa{sv}))a{sv})",
            (
                state.serial,
                METHOD_PERSISTENT if persistent else METHOD_TEMPORARY,
                entries,
                {},
            ),
        )
        self._call("ApplyMonitorsConfig", args)

    def connect_changed(self, callback) -> int:
        """Invoke callback whenever mutter reports a monitor hotplug or
        reconfiguration, so a UI can refresh itself."""
        return self._bus.signal_subscribe(
            BUS_NAME,
            BUS_NAME,
            "MonitorsChanged",
            OBJECT_PATH,
            None,
            Gio.DBusSignalFlags.NONE,
            lambda *_args: callback(),
            None,
        )
