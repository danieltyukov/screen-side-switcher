import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import App from './App';
import { createMock, type MockOptions } from './backend/mock';

async function setup(options: MockOptions = { screens: 2 }, before?: (b: ReturnType<typeof createMock>) => Promise<unknown>) {
  const backend = createMock(options);
  await before?.(backend);
  const user = userEvent.setup();
  render(<App backend={backend} />);
  await screen.findByRole('tablist');
  return { backend, user };
}

const group = (name: RegExp) => screen.getByRole('group', { name });

describe('App: arrange', () => {
  it('moves the only external screen and says where the pointer crosses', async () => {
    const { user, backend } = await setup();
    expect(within(group(/Side/)).getByRole('button', { name: 'Left' })).toHaveAttribute('aria-pressed', 'true');
    await user.click(within(group(/Side/)).getByRole('button', { name: 'Right' }));
    await waitFor(() =>
      expect(within(group(/Side/)).getByRole('button', { name: 'Right' })).toHaveAttribute('aria-pressed', 'true'),
    );
    expect(screen.getByText(/leaves the right edge of the built-in screen/)).toBeInTheDocument();
    expect((await backend.getState()).screens.find((s) => s.id === 'HDMI-1')?.x).toBe(1280);
  });

  it('offers screen chips only with two or more external screens', async () => {
    await setup({ screens: 2 });
    expect(screen.queryByRole('radiogroup', { name: /Screen to move/ })).toBeNull();
  });

  it('moves one chosen screen of several', async () => {
    const { user, backend } = await setup({ screens: 3 });
    const chips = screen.getByRole('radiogroup', { name: /Screen to move/ });
    expect(within(chips).getByRole('radio', { name: 'All screens' })).toBeChecked();
    await user.click(within(chips).getByRole('radio', { name: 'LG HDR 4K' }));
    await user.click(within(group(/Side/)).getByRole('button', { name: 'Left' }));
    await waitFor(async () =>
      expect((await backend.getState()).arrangement?.placements).toEqual([
        { screen: 'HDMI-1', side: 'left' },
        { screen: 'DP-1', side: 'left' },
      ]),
    );
  });

  it('labels alignment by axis and shows custom', async () => {
    const { user } = await setup({ screens: 2 });
    const align = group(/Line up/);
    expect(within(align).getByRole('button', { name: 'Top' })).toBeInTheDocument();
    await user.click(within(group(/Side/)).getByRole('button', { name: 'Above' }));
    await waitFor(() => expect(within(group(/Line up/)).getByRole('button', { name: 'Left' })).toBeInTheDocument());
    await user.click(within(group(/Line up/)).getByRole('button', { name: 'Right' }));
    await waitFor(() =>
      expect(within(group(/Line up/)).getByRole('button', { name: 'Right' })).toHaveAttribute('aria-pressed', 'true'),
    );
  });

  it('makes a screen primary only where the backend can', async () => {
    const { user, backend } = await setup({ screens: 2 });
    await user.click(screen.getByRole('button', { name: 'Make primary' }));
    await waitFor(async () =>
      expect((await backend.getState()).screens.find((s) => s.id === 'HDMI-1')?.primary).toBe(true),
    );
  });

  it('hides Make primary on wlroots', async () => {
    await setup({ screens: 2, backend: 'wlroots' });
    expect(screen.queryByRole('button', { name: 'Make primary' })).toBeNull();
  });

  it('shows a refusal in a banner and keeps the real state', async () => {
    const { user } = await setup({ screens: 3 });
    const chips = screen.getByRole('radiogroup', { name: /Screen to move/ });
    await user.click(within(chips).getByRole('radio', { name: 'LG HDR 4K' }));
    await user.click(within(group(/Side/)).getByRole('button', { name: 'Right' }));
    await user.click(within(group(/Line up/)).getByRole('button', { name: 'Centred' }));
    await user.click(within(group(/Side/)).getByRole('button', { name: 'Above' }));
    expect(await screen.findByRole('alert')).toHaveTextContent(/would overlap/);
    expect(within(group(/Side/)).getByRole('button', { name: 'Right' })).toHaveAttribute('aria-pressed', 'true');
    await user.click(screen.getByRole('button', { name: 'Dismiss' }));
    expect(screen.queryByRole('alert')).toBeNull();
  });

  it('refreshes when the screens change outside the window', async () => {
    const { backend } = await setup({ screens: 2 });
    backend.simulateHotplug();
    expect(await screen.findByRole('button', { name: /LG HDR 4K/ })).toBeInTheDocument();
  });

  it('shows the problem and a way to report it when there is no backend', async () => {
    await setup({ error: 'This is a Wayland session that is neither GNOME nor KDE. Install wlr-randr.' });
    expect(screen.getByRole('alert')).toHaveTextContent(/Install wlr-randr/);
    expect(screen.getByRole('button', { name: 'Copy diagnostics' })).toBeInTheDocument();
    expect(screen.queryByRole('group', { name: /Side/ })).toBeNull();
  });
});

describe('App: tabs', () => {
  it('switches tabs by click and arrow keys', async () => {
    const { user } = await setup();
    const tabs = screen.getAllByRole('tab');
    expect(tabs.map((t) => t.textContent)).toEqual(['Arrange', 'Layouts', 'Settings']);
    await user.click(screen.getByRole('tab', { name: 'Layouts' }));
    expect(screen.getByRole('tab', { name: 'Layouts' })).toHaveAttribute('aria-selected', 'true');
    await user.keyboard('{ArrowRight}');
    expect(screen.getByRole('tab', { name: 'Settings' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('tab', { name: 'Settings' })).toHaveFocus();
    await user.keyboard('{Home}');
    expect(screen.getByRole('tab', { name: 'Arrange' })).toHaveAttribute('aria-selected', 'true');
  });
});

describe('App: layouts', () => {
  it('saves, applies, switches auto and deletes with an inline confirmation', async () => {
    const { user, backend } = await setup();
    await user.click(screen.getByRole('tab', { name: 'Layouts' }));
    expect(screen.getByText(/No layouts yet/)).toBeInTheDocument();
    await user.type(screen.getByLabelText('Name'), 'office');
    await user.click(screen.getByLabelText(/Apply automatically when these screens connect/));
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved office'));
    const item = screen.getByRole('listitem');
    expect(within(item).getByText('In use')).toBeInTheDocument();
    const auto = within(item).getByRole('switch', { name: /Apply automatically/ });
    expect(auto).toHaveAttribute('aria-checked', 'true');
    await user.click(auto);
    await waitFor(() => expect(within(screen.getByRole('listitem')).getByRole('switch')).toHaveAttribute('aria-checked', 'false'));

    await backend.moveScreen(null, 'right');
    backend.simulateHotplug();
    backend.simulateHotplug();
    await user.click(await within(screen.getByRole('listitem')).findByRole('button', { name: 'Apply' }));
    await waitFor(async () => expect((await backend.getState()).activeLayout).toBe('office'));

    await user.click(within(screen.getByRole('listitem')).getByRole('button', { name: 'Delete' }));
    expect(screen.getByText('Delete office?')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.getByRole('listitem')).toBeInTheDocument();
    await user.click(within(screen.getByRole('listitem')).getByRole('button', { name: 'Delete' }));
    await user.click(screen.getByRole('button', { name: 'Delete office' }));
    await waitFor(() => expect(screen.queryByRole('listitem')).toBeNull());
  });

  it('greys out layouts for other screens', async () => {
    const { user, backend } = await setup();
    await backend.saveLayout('home', false);
    backend.simulateHotplug();
    await user.click(screen.getByRole('tab', { name: 'Layouts' }));
    const item = await screen.findByRole('listitem');
    expect(within(item).getByText('For other screens')).toBeInTheDocument();
    expect(within(item).queryByRole('button', { name: 'Apply' })).toBeNull();
  });

  it('says when a save replaced a layout', async () => {
    const { user } = await setup({ screens: 2 }, (b) => b.saveLayout('office', false));
    await user.click(screen.getByRole('tab', { name: 'Layouts' }));
    await user.type(screen.getByLabelText('Name'), 'Office');
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Replaced Office'));
  });
});

describe('App: settings', () => {
  it('switches background and autostart', async () => {
    const { user, backend } = await setup();
    await user.click(screen.getByRole('tab', { name: 'Settings' }));
    const background = screen.getByRole('switch', { name: /Keep running in the background/ });
    expect(background).toHaveAttribute('aria-checked', 'false');
    await user.click(background);
    await waitFor(async () => expect((await backend.getState()).settings.background).toBe(true));
    await user.click(screen.getByRole('switch', { name: /Start at login/ }));
    await waitFor(async () => expect((await backend.getState()).settings.autoStart).toBe(true));
    expect(screen.getByText(/AppIndicator/)).toBeInTheDocument();
  });

  it('records an app shortcut on Windows', async () => {
    const { user, backend } = await setup({ screens: 2, backend: 'windows' });
    await user.click(screen.getByRole('tab', { name: 'Settings' }));
    await user.click(screen.getByRole('button', { name: 'Record shortcut' }));
    await user.keyboard('{Control>}{Alt>}k{/Alt}{/Control}');
    expect(screen.getByText('Ctrl+Alt+K')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Save shortcut' }));
    await waitFor(async () => expect((await backend.getState()).settings.shortcut).toBe('Ctrl+Alt+K'));
    await user.click(screen.getByRole('button', { name: 'Clear' }));
    await waitFor(async () => expect((await backend.getState()).settings.shortcut).toBeNull());
  });

  it('sets up the GNOME shortcut', async () => {
    const { user } = await setup({ screens: 2, backend: 'gnome' });
    await user.click(screen.getByRole('tab', { name: 'Settings' }));
    expect(screen.getByText(/Super\+Alt\+S/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Set up shortcut' }));
    expect(await screen.findByRole('button', { name: 'Remove shortcut' })).toBeInTheDocument();
  });

  it('shows instructions where the desktop owns shortcuts', async () => {
    const { user } = await setup({ screens: 2, backend: 'wlroots' });
    await user.click(screen.getByRole('tab', { name: 'Settings' }));
    expect(screen.getByText(/bindsym Mod4\+Mod1\+s/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Copy' })).toBeInTheDocument();
  });

  it('copies diagnostics and shows the command line path', async () => {
    const { user } = await setup();
    await user.click(screen.getByRole('tab', { name: 'Settings' }));
    expect(screen.getByText('/usr/bin/screen-side')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Copy diagnostics' }));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Diagnostics copied'));
    expect(screen.getByText('Screen Side 2.0.0')).toBeInTheDocument();
  });
});
