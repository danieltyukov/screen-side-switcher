import { describe, expect, it, vi } from 'vitest';
import { createMock } from './mock';
import type { AppState, ScreenInfo } from './types';

function screen(state: AppState, id: string): ScreenInfo {
  const s = state.screens.find((x) => x.id === id);
  if (!s) throw new Error(`no screen ${id}`);
  return s;
}

describe('mock backend', () => {
  it('starts with a laptop and a monitor on the left', async () => {
    const state = await createMock({ screens: 2 }).getState();
    expect(state.screens.map((s) => s.id)).toEqual(['HDMI-1', 'eDP-1']);
    expect(screen(state, 'eDP-1')).toMatchObject({ builtin: true, primary: true, width: 1280, height: 800 });
    expect(state.arrangement?.placements).toEqual([{ screen: 'HDMI-1', side: 'left' }]);
    expect(state.screens.map((s) => s.number)).toEqual([1, 2]);
  });

  it('moves every external screen and centres them', async () => {
    const state = await createMock({ screens: 2 }).moveScreen(null, 'right');
    expect(screen(state, 'HDMI-1')).toMatchObject({ x: 1280, y: 0 });
    expect(screen(state, 'eDP-1')).toMatchObject({ x: 0, y: 320 });
  });

  it('lines up bottom edges with end alignment', async () => {
    const mock = createMock({ screens: 2 });
    await mock.moveScreen(null, 'right');
    const state = await mock.setAlign('end');
    const bottom = (s: ScreenInfo) => s.y + s.height;
    expect(bottom(screen(state, 'HDMI-1'))).toBe(bottom(screen(state, 'eDP-1')));
    expect(state.arrangement?.align).toBe('end');
  });

  it('moves one screen of three to the far end of a side', async () => {
    const state = await createMock({ screens: 3 }).moveScreen('DP-1', 'left');
    expect(state.arrangement?.placements).toEqual([
      { screen: 'HDMI-1', side: 'left' },
      { screen: 'DP-1', side: 'left' },
    ]);
    expect(screen(state, 'DP-1').x).toBeLessThan(screen(state, 'HDMI-1').x);
  });

  it('refuses an arrangement whose screens would overlap', async () => {
    const mock = createMock({ screens: 3 });
    await mock.moveScreen('DP-1', 'right');
    await mock.setAlign('center');
    await expect(mock.moveScreen('DP-1', 'above')).rejects.toThrow(/would overlap/);
    const after = await mock.getState();
    expect(after.arrangement?.placements.find((p) => p.screen === 'DP-1')?.side).toBe('right');
  });

  it('toggles every side', async () => {
    const mock = createMock({ screens: 3 });
    const state = await mock.toggle();
    expect(state.arrangement?.placements).toEqual([
      { screen: 'HDMI-1', side: 'right' },
      { screen: 'DP-1', side: 'below' },
    ]);
    expect(state.arrangement?.align).toBe('end');
  });

  it('sets the primary screen where the backend can', async () => {
    const state = await createMock({ screens: 2 }).setPrimary('HDMI-1');
    expect(screen(state, 'HDMI-1').primary).toBe(true);
    expect(screen(state, 'eDP-1').primary).toBe(false);
    const wl = createMock({ screens: 2, backend: 'wlroots' });
    expect((await wl.getState()).capabilities?.primary).toBe(false);
    await expect(wl.setPrimary('HDMI-1')).rejects.toThrow(/no primary screen/);
  });

  it('puts the primary screen at the origin on Windows', async () => {
    const state = await createMock({ screens: 2, backend: 'windows' }).getState();
    expect(screen(state, 'eDP-1')).toMatchObject({ x: 0, y: 0 });
    expect(screen(state, 'HDMI-1').x).toBe(-2560);
    expect(state.platform).toBe('windows');
    expect(state.shortcut.kind).toBe('app');
  });

  it('saves, recognises, applies and forgets layouts', async () => {
    const mock = createMock({ screens: 2 });
    let state = await mock.saveLayout('office', true);
    expect(state.layouts).toEqual([
      expect.objectContaining({ name: 'office', auto: true, matches: true }),
    ]);
    expect(state.activeLayout).toBe('office');
    state = await mock.moveScreen(null, 'right');
    expect(state.activeLayout).toBeNull();
    state = await mock.applyLayout('office');
    expect(state.activeLayout).toBe('office');
    expect(screen(state, 'HDMI-1').x).toBe(0);
    state = await mock.setLayoutAuto('office', false);
    expect(state.layouts[0]?.auto).toBe(false);
    state = await mock.forgetLayout('office');
    expect(state.layouts).toEqual([]);
  });

  it('keeps settings', async () => {
    const mock = createMock({ screens: 2 });
    const state = await mock.updateSettings({ background: false, shortcut: 'Super+Alt+S' });
    expect(state.settings).toMatchObject({ background: false, shortcut: 'Super+Alt+S' });
  });

  it('tells listeners about a hotplug', async () => {
    const mock = createMock({ screens: 2 });
    const listener = vi.fn();
    const stop = mock.onChange(listener);
    mock.simulateHotplug();
    expect(listener).toHaveBeenCalledTimes(1);
    expect((await mock.getState()).screens.map((s) => s.id)).toContain('DP-1');
    stop();
    mock.simulateHotplug();
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it('can stand in for a session with no backend', async () => {
    const state = await createMock({ error: 'No backend' }).getState();
    expect(state.error).toBe('No backend');
    expect(state.screens).toEqual([]);
    expect(state.backend).toBeNull();
  });

  it('has nothing to arrange with one screen', async () => {
    const state = await createMock({ screens: 1 }).getState();
    expect(state.arrangement).toBeNull();
    expect(state.screens).toHaveLength(1);
  });
});
