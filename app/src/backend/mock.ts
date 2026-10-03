import type {
  Align,
  AppState,
  Arrangement,
  Backend,
  Capabilities,
  LayoutInfo,
  Placement,
  Platform,
  ScreenInfo,
  Settings,
  ShortcutSupport,
  Side,
} from './types';

/*
 * An in-memory backend for the browser: development, unit tests and the
 * Playwright run that also takes the screenshots.
 *
 * It carries a small copy of the chain maths so the preview moves like the
 * real thing. The real maths is crates/core/src/layout.rs; this copy only
 * needs to be close enough to draw and to refuse overlaps.
 */

export type MockKind = 'gnome' | 'kde' | 'wlroots' | 'x11' | 'windows' | 'macos';

export interface MockOptions {
  screens?: 1 | 2 | 3;
  backend?: MockKind;
  /** No backend at all: this message and no screens. */
  error?: string;
  /** Screens that overlap at the origin, as X11 often leaves a new monitor. */
  custom?: boolean;
  /** A problem shown next to working screens. */
  warning?: string;
  layoutsError?: string;
}

export interface MockBackend extends Backend {
  /** Connects or disconnects a third screen, as plugging in a cable would. */
  simulateHotplug(): void;
}

interface Physical {
  id: string;
  connector: string;
  name: string;
  builtin: boolean;
  width: number;
  height: number;
  scale: number;
}

const LAPTOP: Physical = { id: 'eDP-1', connector: 'eDP-1', name: 'Built-in display', builtin: true, width: 1280, height: 800, scale: 1.5 };
const DELL: Physical = { id: 'HDMI-1', connector: 'HDMI-1', name: 'DELL U2723QE', builtin: false, width: 2560, height: 1440, scale: 1 };
const LG: Physical = { id: 'DP-1', connector: 'DP-1', name: 'LG HDR 4K', builtin: false, width: 1920, height: 1080, scale: 1.25 };

const CAPABILITIES: Record<MockKind, Capabilities> = {
  gnome: { primary: true, temporary: true, verify: true, remembers: true, origin: 'top_left' },
  kde: { primary: true, temporary: false, verify: false, remembers: true, origin: 'top_left' },
  wlroots: { primary: false, temporary: false, verify: true, remembers: false, origin: 'top_left' },
  x11: { primary: true, temporary: false, verify: false, remembers: false, origin: 'top_left' },
  windows: { primary: true, temporary: true, verify: true, remembers: true, origin: 'primary' },
  macos: { primary: true, temporary: true, verify: false, remembers: true, origin: 'primary' },
};

const SIDES: Side[] = ['left', 'right', 'above', 'below'];
const MIRROR: Record<Side, Side> = { left: 'right', right: 'left', above: 'below', below: 'above' };

interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

function offset(align: Align, anchorLength: number, length: number): number {
  if (align === 'start') return 0;
  if (align === 'end') return anchorLength - length;
  return Math.floor((anchorLength - length) / 2);
}

function overlaps(a: Rect, b: Rect): boolean {
  return a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;
}

/** Positions for every screen, or an error in the same words as the CLI. */
export function compute(screens: Physical[], arr: Arrangement, origin: Capabilities['origin']): Map<string, Rect> {
  const byId = new Map(screens.map((s) => [s.id, s]));
  const anchor = byId.get(arr.anchor);
  if (!anchor) throw new Error(`${arr.anchor} is not connected or is switched off.`);
  const placed: [Physical, Rect][] = [[anchor, { x: 0, y: 0, width: anchor.width, height: anchor.height }]];
  for (const side of SIDES) {
    let cursor = 0;
    for (const p of arr.placements.filter((q) => q.side === side)) {
      const s = byId.get(p.screen);
      if (!s) throw new Error(`${p.screen} is not connected or is switched off.`);
      const { width: w, height: h } = s;
      let rect: Rect;
      if (side === 'left') {
        cursor += w;
        rect = { x: -cursor, y: offset(arr.align, anchor.height, h), width: w, height: h };
      } else if (side === 'right') {
        rect = { x: anchor.width + cursor, y: offset(arr.align, anchor.height, h), width: w, height: h };
        cursor += w;
      } else if (side === 'above') {
        cursor += h;
        rect = { x: offset(arr.align, anchor.width, w), y: -cursor, width: w, height: h };
      } else {
        rect = { x: offset(arr.align, anchor.width, w), y: anchor.height + cursor, width: w, height: h };
        cursor += h;
      }
      placed.push([s, rect]);
    }
  }
  for (let i = 0; i < placed.length; i++) {
    for (let j = i + 1; j < placed.length; j++) {
      const [a, ra] = placed[i]!;
      const [b, rb] = placed[j]!;
      if (overlaps(ra, rb)) {
        throw new Error(`${a.name} and ${b.name} would overlap. Put one of them on another side, or use another alignment.`);
      }
    }
  }
  let dx: number;
  let dy: number;
  if (origin === 'primary') {
    const primary = placed.find(([s]) => s.id === arr.primary)?.[1] ?? { x: 0, y: 0 };
    dx = -primary.x;
    dy = -primary.y;
  } else {
    dx = -Math.min(...placed.map(([, r]) => r.x));
    dy = -Math.min(...placed.map(([, r]) => r.y));
  }
  return new Map(placed.map(([s, r]) => [s.id, { ...r, x: r.x + dx, y: r.y + dy }]));
}

interface SavedLayout {
  name: string;
  auto: boolean;
  screens: string;
  arrangement: Arrangement;
}

function sameArrangement(a: Arrangement, b: Arrangement): boolean {
  return (
    a.anchor === b.anchor &&
    a.align === b.align &&
    a.primary === b.primary &&
    JSON.stringify(a.placements) === JSON.stringify(b.placements)
  );
}

function summary(arr: Arrangement, count: number): string {
  const counts = new Map<Side, number>();
  for (const p of arr.placements) counts.set(p.side, (counts.get(p.side) ?? 0) + 1);
  const sides = [...counts].map(([side, n]) => `${n} ${side}`).join(', ');
  const horizontal = (arr.placements[0]?.side ?? 'left') === 'left' || arr.placements[0]?.side === 'right';
  const label = arr.align === 'center' ? 'centred' : arr.align === 'start' ? (horizontal ? 'top' : 'left') : horizontal ? 'bottom' : 'right';
  return `${count} screens: ${sides}; ${label}`;
}

export function createMock(options: MockOptions = {}): MockBackend {
  const kind: MockKind = options.backend ?? 'gnome';
  const caps = CAPABILITIES[kind];
  const platform: Platform = kind === 'windows' ? 'windows' : kind === 'macos' ? 'macos' : 'linux';
  const count = options.screens ?? 2;
  let screens: Physical[] = [LAPTOP, DELL, LG].slice(0, count);
  let arrangement: Arrangement | null =
    count === 1
      ? null
      : {
          anchor: 'eDP-1',
          placements:
            count === 3
              ? [
                  { screen: 'HDMI-1', side: 'left' },
                  { screen: 'DP-1', side: 'above' },
                ]
              : [{ screen: 'HDMI-1', side: 'left' }],
          align: count === 3 ? 'start' : 'center',
          aligned: true,
          primary: 'eDP-1',
        };
  let layouts: SavedLayout[] = [];
  let custom = options.custom === true && count >= 2;
  if (custom && arrangement) {
    arrangement = { ...arrangement, placements: arrangement.placements.map((p) => ({ ...p, side: 'right' })), align: 'center' };
  }
  let settings: Settings = {
    background: platform !== 'linux',
    autoStart: false,
    autoApply: true,
    shortcut: null,
  };
  let gnomeShortcut = false;
  const listeners = new Set<() => void>();

  const fingerprint = () => screens.map((s) => s.id).sort().join(',');

  const shortcut = (): ShortcutSupport => {
    if (kind === 'windows' || kind === 'macos' || kind === 'x11') return { kind: 'app' };
    if (kind === 'gnome') return { kind: 'gnome', installed: gnomeShortcut, keys: '<Super><Alt>s' };
    const line =
      kind === 'wlroots'
        ? "bindsym Mod4+Mod1+s exec '/usr/bin/screen-side' 'toggle'"
        : "'/usr/bin/screen-side' 'toggle'";
    return { kind: 'manual', instructions: `Add this to your desktop's shortcuts:\n\n    ${line}\n` };
  };

  const snapshot = (): AppState => {
    if (options.error) {
      return {
        version: '2.0.0',
        platform,
        backend: null,
        capabilities: null,
        screens: [],
        arrangement: null,
        custom: false,
        layouts: [],
        layoutsError: null,
        activeLayout: null,
        settings: { ...settings },
        shortcut: shortcut(),
        cliPath: null,
        error: options.error,
      };
    }
    const positions =
      arrangement && !custom
        ? compute(screens, arrangement, caps.origin)
        : new Map(screens.map((s) => [s.id, { x: 0, y: 0, width: s.width, height: s.height }]));
    const primary = caps.primary ? (arrangement?.primary ?? screens[0]?.id) : null;
    const list: ScreenInfo[] = screens
      .map((s) => {
        const r = positions.get(s.id)!;
        return {
          number: 0,
          id: s.id,
          connector: s.connector,
          name: s.name,
          builtin: s.builtin,
          enabled: true,
          primary: s.id === primary,
          x: r.x,
          y: r.y,
          width: s.width,
          height: s.height,
          scale: s.scale,
        };
      })
      .sort((a, b) => a.x - b.x || a.y - b.y)
      .map((s, i) => ({ ...s, number: i + 1 }));
    const here = fingerprint();
    const infos: LayoutInfo[] = [...layouts]
      .sort((a, b) => Number(b.screens === here) - Number(a.screens === here))
      .map((l) => ({
        name: l.name,
        auto: l.auto,
        matches: l.screens === here,
        summary: summary(l.arrangement, l.screens.split(',').length),
      }));
    const active =
      arrangement === null || custom
        ? null
        : (layouts.find((l) => l.screens === here && sameArrangement(l.arrangement, arrangement!))?.name ?? null);
    return {
      version: '2.0.0',
      platform,
      backend: kind,
      capabilities: { ...caps },
      screens: list,
      arrangement: arrangement && structuredClone(arrangement),
      custom,
      layouts: options.layoutsError ? [] : infos,
      layoutsError: options.layoutsError ?? null,
      activeLayout: active,
      settings: { ...settings },
      shortcut: shortcut(),
      cliPath: platform === 'windows' ? null : '/usr/bin/screen-side',
      error: options.warning ?? null,
    };
  };

  /** Applies `next` if the maths accepts it, like the real backends. */
  const commit = (next: Arrangement): AppState => {
    compute(screens, next, caps.origin);
    arrangement = next;
    custom = false;
    return snapshot();
  };

  const need = (): Arrangement => {
    if (!arrangement) throw new Error('Only one screen is switched on, so there is nothing to arrange.');
    return arrangement;
  };

  const run = async (work: () => AppState): Promise<AppState> => work();

  return {
    getState: () => run(snapshot),

    moveScreen: (screen, side) =>
      run(() => {
        const arr = need();
        if (screen === null) {
          const ranked = arr.placements
            .map((p, i) => ({ p, i, rank: arr.placements.slice(0, i).filter((q) => q.side === p.side).length }))
            .sort((a, b) => a.rank - b.rank || a.i - b.i);
          return commit({ ...arr, aligned: true, placements: ranked.map(({ p }) => ({ screen: p.screen, side })) });
        }
        if (screen === arr.anchor) {
          const only = arr.placements.length === 1 ? arr.placements[0] : undefined;
          if (!only) throw new Error('Pick an external screen to move. The built-in screen stays where it is.');
          return commit({ ...arr, aligned: true, placements: [{ screen: only.screen, side: MIRROR[side] }] });
        }
        const current = arr.placements.find((p) => p.screen === screen);
        if (!current) throw new Error(`${screen} is not connected or is switched off.`);
        if (current.side === side) return snapshot();
        const placements: Placement[] = [...arr.placements.filter((p) => p.screen !== screen), { screen, side }];
        return commit({ ...arr, aligned: true, placements });
      }),

    toggle: () =>
      run(() => {
        const arr = need();
        // Both axes in use: a point reflection, so start and end swap too.
        const horizontal = arr.placements.some((p) => p.side === 'left' || p.side === 'right');
        const vertical = arr.placements.some((p) => p.side === 'above' || p.side === 'below');
        const flip: Record<Align, Align> = { start: 'end', center: 'center', end: 'start' };
        const align = horizontal && vertical ? flip[arr.align] : arr.align;
        return commit({ ...arr, align, aligned: true, placements: arr.placements.map((p) => ({ screen: p.screen, side: MIRROR[p.side] })) });
      }),

    setAlign: (align) => run(() => commit({ ...need(), align, aligned: true })),

    setPrimary: (screen) =>
      run(() => {
        if (!caps.primary) throw new Error(`The ${kind} backend has no primary screen to set.`);
        return commit({ ...need(), primary: screen });
      }),

    saveLayout: (name, auto) =>
      run(() => {
        const trimmed = name.trim();
        if (!trimmed || trimmed.length > 64) throw new Error('A layout name needs 1 to 64 characters and no control characters.');
        const saved: SavedLayout = { name: trimmed, auto, screens: fingerprint(), arrangement: structuredClone(need()) };
        const at = layouts.findIndex((l) => l.name.toLowerCase() === trimmed.toLowerCase());
        layouts = at >= 0 ? layouts.map((l, i) => (i === at ? saved : l)) : [...layouts, saved];
        return snapshot();
      }),

    applyLayout: (name) =>
      run(() => {
        const saved = layouts.find((l) => l.name.toLowerCase() === name.toLowerCase());
        if (!saved) throw new Error(`There is no saved layout called '${name}'.`);
        if (saved.screens !== fingerprint()) throw new Error(`'${saved.name}' was saved for other screens than the ones connected now.`);
        return commit(structuredClone(saved.arrangement));
      }),

    forgetLayout: (name) =>
      run(() => {
        layouts = layouts.filter((l) => l.name.toLowerCase() !== name.toLowerCase());
        return snapshot();
      }),

    setLayoutAuto: (name, auto) =>
      run(() => {
        layouts = layouts.map((l) => (l.name.toLowerCase() === name.toLowerCase() ? { ...l, auto } : l));
        return snapshot();
      }),

    updateSettings: (patch) =>
      run(() => {
        settings = { ...settings, ...patch };
        return snapshot();
      }),

    installShortcut: () =>
      run(() => {
        gnomeShortcut = true;
        return snapshot();
      }),

    removeShortcut: () =>
      run(() => {
        gnomeShortcut = false;
        return snapshot();
      }),

    diagnostics: async () =>
      [
        'Screen Side 2.0.0 (browser preview)',
        `Backend: ${options.error ? 'none' : kind} (mock)`,
        ...screens.map((s) => `  ${s.connector} (${s.name}): ${s.width}x${s.height}`),
      ].join('\n'),

    openUrl: async () => {},
    copyText: async () => {},

    onChange: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },

    simulateHotplug: () => {
      if (screens.some((s) => s.id === LG.id)) {
        screens = screens.filter((s) => s.id !== LG.id);
        if (arrangement) {
          arrangement = { ...arrangement, placements: arrangement.placements.filter((p) => p.screen !== LG.id) };
        }
      } else {
        screens = [...screens, LG];
        if (arrangement) {
          arrangement = { ...arrangement, placements: [...arrangement.placements, { screen: LG.id, side: 'right' }] };
        } else {
          arrangement = { anchor: 'eDP-1', placements: [{ screen: LG.id, side: 'right' }], align: 'center', aligned: true, primary: 'eDP-1' };
        }
      }
      for (const listener of listeners) listener();
    },
  };
}
