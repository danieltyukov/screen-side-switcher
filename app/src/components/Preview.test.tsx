import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { createMock } from '../backend/mock';
import { Preview } from './Preview';

describe('Preview', () => {
  it('draws every enabled screen as a button and marks the primary', async () => {
    const state = await createMock({ screens: 3 }).getState();
    render(<Preview screens={state.screens} arrangement={state.arrangement} selected={null} onSelect={() => {}} />);
    const buttons = screen.getAllByRole('button');
    expect(buttons).toHaveLength(3);
    expect(screen.getByRole('button', { name: /Built-in display, primary/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /DELL U2723QE, left of the built-in screen/ })).toBeInTheDocument();
  });

  it('selects by click and by keyboard', async () => {
    const state = await createMock({ screens: 3 }).getState();
    const onSelect = vi.fn();
    render(<Preview screens={state.screens} arrangement={state.arrangement} selected="DP-1" onSelect={onSelect} />);
    expect(screen.getByRole('button', { name: /LG HDR 4K/ })).toHaveAttribute('aria-pressed', 'true');
    await userEvent.click(screen.getByRole('button', { name: /DELL U2723QE/ }));
    expect(onSelect).toHaveBeenLastCalledWith('HDMI-1');
    screen.getByRole('button', { name: /LG HDR 4K/ }).focus();
    await userEvent.keyboard('{Enter}');
    expect(onSelect).toHaveBeenLastCalledWith('DP-1');
    await userEvent.click(screen.getByRole('button', { name: /Built-in display/ }));
    expect(onSelect).toHaveBeenLastCalledWith(null);
  });

  it('lights the edge the pointer crosses', async () => {
    const state = await createMock({ screens: 2 }).getState();
    const { container } = render(
      <Preview screens={state.screens} arrangement={state.arrangement} selected="HDMI-1" onSelect={() => {}} />,
    );
    expect(container.querySelector('[data-edge="HDMI-1"]')).not.toBeNull();
    expect(container.querySelector('[data-pointer]')).not.toBeNull();
  });

  it('lists switched-off screens under the drawing', () => {
    render(
      <Preview
        screens={[
          { number: 1, id: 'HDMI-1', connector: 'HDMI-1', name: 'Philips 27', builtin: false, enabled: true, primary: true, x: 0, y: 0, width: 2560, height: 1440, scale: 1 },
          { number: 2, id: 'eDP-1', connector: 'eDP-1', name: 'Built-in display', builtin: true, enabled: false, primary: false, x: 0, y: 0, width: 0, height: 0, scale: 1 },
        ]}
        arrangement={null}
        selected={null}
        onSelect={() => {}}
      />,
    );
    expect(screen.getByText('Built-in display is off.')).toBeInTheDocument();
    expect(screen.getAllByRole('button')).toHaveLength(1);
  });
});
