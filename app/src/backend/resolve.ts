import { createMock, type MockKind } from './mock';
import type { Backend } from './types';

/*
 * The backend for a browser tab: development, Playwright and screenshots.
 * Under Tauri, main.tsx loads tauri.ts instead and never calls this.
 *
 * ?screens=1|2|3, ?backend=gnome|kde|wlroots|x11|windows|macos,
 * ?error=<message> and ?theme=light|dark set up what the page shows.
 */
export function resolveBackend(search: string = location.search): Backend {
  const params = new URLSearchParams(search);
  const screens = Number(params.get('screens') ?? 2);
  const theme = params.get('theme');
  if (theme === 'light' || theme === 'dark') document.documentElement.dataset.theme = theme;
  const backend = params.get('backend') as MockKind | null;
  const error = params.get('error');
  return createMock({
    screens: screens === 1 || screens === 3 ? screens : 2,
    ...(backend ? { backend } : {}),
    ...(error ? { error } : {}),
  });
}
