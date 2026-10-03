/*
 * The one seam between the interface and everything underneath it.
 *
 * Components know this file and nothing else: no Tauri import, no command
 * names. The Tauri shell implements `Backend` (tauri.ts); a browser gets the
 * mock (mock.ts). That keeps the whole interface runnable and testable
 * without a display backend, and it mirrors `AppState` in
 * app/src-tauri/src/view.rs field for field.
 */

export type Side = 'left' | 'right' | 'above' | 'below';
export type Align = 'start' | 'center' | 'end';
export type Platform = 'linux' | 'windows' | 'macos';

export interface ScreenInfo {
  /** 1-based, in the order `screen-side status` lists them. */
  number: number;
  id: string;
  connector: string;
  name: string;
  builtin: boolean;
  /** Part of the desktop right now. A closed lid is connected but off. */
  enabled: boolean;
  primary: boolean;
  x: number;
  y: number;
  width: number;
  height: number;
  scale: number;
}

export interface Capabilities {
  primary: boolean;
  temporary: boolean;
  verify: boolean;
  remembers: boolean;
  origin: 'top_left' | 'primary';
}

export interface Placement {
  screen: string;
  side: Side;
}

export interface Arrangement {
  anchor: string;
  /** Nearest first within each side. */
  placements: Placement[];
  align: Align;
  /** False when the screens line up in no named way ("custom"). */
  aligned: boolean;
  primary: string;
}

export interface LayoutInfo {
  name: string;
  auto: boolean;
  /** Saved for the screens connected now. */
  matches: boolean;
  summary: string;
}

export interface Settings {
  background: boolean;
  autoStart: boolean;
  autoApply: boolean;
  /** Accelerator such as "Super+Alt+S", or null for none. */
  shortcut: string | null;
}

export type ShortcutSupport =
  | { kind: 'app' }
  | { kind: 'gnome'; installed: boolean; keys: string }
  | { kind: 'manual'; instructions: string };

export interface AppState {
  version: string;
  platform: Platform;
  backend: string | null;
  capabilities: Capabilities | null;
  screens: ScreenInfo[];
  arrangement: Arrangement | null;
  /** Two or more screens are on but overlap, so no side describes them;
   * `arrangement` is then where the side buttons start from. */
  custom: boolean;
  layouts: LayoutInfo[];
  /** layouts.json could not be read; shown in place of the list. */
  layoutsError: string | null;
  activeLayout: string | null;
  settings: Settings;
  shortcut: ShortcutSupport;
  cliPath: string | null;
  error: string | null;
}

export interface Backend {
  getState(): Promise<AppState>;
  /** `screen` null moves every external screen. */
  moveScreen(screen: string | null, side: Side): Promise<AppState>;
  toggle(): Promise<AppState>;
  setAlign(align: Align): Promise<AppState>;
  setPrimary(screen: string): Promise<AppState>;
  saveLayout(name: string, auto: boolean): Promise<AppState>;
  applyLayout(name: string): Promise<AppState>;
  forgetLayout(name: string): Promise<AppState>;
  setLayoutAuto(name: string, auto: boolean): Promise<AppState>;
  updateSettings(patch: Partial<Settings>): Promise<AppState>;
  installShortcut(): Promise<AppState>;
  removeShortcut(): Promise<AppState>;
  diagnostics(): Promise<string>;
  openUrl(url: string): Promise<void>;
  copyText(text: string): Promise<void>;
  /** Called when the screens change outside the window. Returns an unsubscribe. */
  onChange(listener: () => void): () => void;
}
