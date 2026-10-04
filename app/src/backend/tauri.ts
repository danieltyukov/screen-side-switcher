import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { openUrl } from '@tauri-apps/plugin-opener';
import type { AppState, Backend } from './types';

/*
 * The Tauri side of the seam. Command names live here and in
 * app/src-tauri/src/commands.rs and nowhere else. A rejected command carries
 * the user-facing text from Rust.
 */
export function createTauriBackend(): Backend {
  const call = (command: string, args?: Record<string, unknown>) => invoke<AppState>(command, args);
  return {
    getState: () => call('get_state'),
    moveScreen: (screen, side) => call('move_screen', { screen, side }),
    toggle: () => call('toggle'),
    setAlign: (align) => call('set_align', { align }),
    setPrimary: (screen) => call('set_primary', { screen }),
    saveLayout: (name, auto) => call('save_layout', { name, auto }),
    applyLayout: (name) => call('apply_layout', { name }),
    forgetLayout: (name) => call('forget_layout', { name }),
    setLayoutAuto: (name, auto) => call('set_layout_auto', { name, auto }),
    updateSettings: (patch) => call('update_settings', { patch }),
    installShortcut: () => call('install_shortcut'),
    removeShortcut: () => call('remove_shortcut'),
    diagnostics: () => invoke<string>('diagnostics'),
    openUrl: (url) => openUrl(url),
    copyText: (text) => writeText(text),
    onChange: (listener) => {
      let stop: (() => void) | null = null;
      let stopped = false;
      void listen('state-changed', () => listener()).then((unlisten) => {
        if (stopped) unlisten();
        else stop = unlisten;
      });
      return () => {
        stopped = true;
        stop?.();
      };
    },
  };
}
