import { useEffect, useState } from 'react';
import type { Align, AppState, Backend, Side } from '../backend/types';
import { SideIcon } from '../icons';
import { alignLabel, anchorText, crossing, headline, isHorizontal, nameOf } from '../text';
import { Preview } from './Preview';
import './Arrange.css';

type Act = (work: () => Promise<AppState>) => Promise<void>;

const SIDES: { side: Side; label: string }[] = [
  { side: 'left', label: 'Left' },
  { side: 'right', label: 'Right' },
  { side: 'above', label: 'Above' },
  { side: 'below', label: 'Below' },
];
const ALIGNS: Align[] = ['start', 'center', 'end'];

/** The screen that moves by default: the only external one, else all of them. */
function defaultSelection(state: AppState): string | null {
  const placements = state.arrangement?.placements ?? [];
  return placements.length === 1 ? placements[0]!.screen : null;
}

export function Arrange({ state, backend, act, busy }: { state: AppState; backend: Backend; act: Act; busy: boolean }) {
  const [selected, setSelected] = useState<string | null>(() => defaultSelection(state));
  const arr = state.arrangement;
  const caps = state.capabilities;

  // Keep the selection valid when screens come and go.
  useEffect(() => {
    const ids = arr?.placements.map((p) => p.screen) ?? [];
    if (selected !== null && !ids.includes(selected)) setSelected(defaultSelection(state));
    else if (selected === null && ids.length === 1) setSelected(ids[0]!);
  }, [arr, selected, state]);

  const externals = arr?.placements.map((p) => p.screen) ?? [];
  const custom = state.custom;
  const placementOf = (id: string) => arr?.placements.find((p) => p.screen === id);
  const currentSide = custom
    ? undefined
    : selected
    ? placementOf(selected)?.side
    : arr && arr.placements.every((p) => p.side === arr.placements[0]?.side)
      ? arr.placements[0]?.side
      : undefined;
  const axisSide = (selected ? placementOf(selected)?.side : arr?.placements[0]?.side) ?? 'left';
  const horizontal = isHorizontal(axisSide);
  const focus = selected ?? (externals.length === 1 ? externals[0] : undefined);
  const focusSide = focus ? placementOf(focus)?.side : undefined;
  const selectedScreen = selected ? state.screens.find((s) => s.id === selected) : undefined;

  return (
    <div className="arrange">
      <figure className="arrange-figure">
        <Preview screens={state.screens} arrangement={arr} selected={selected} onSelect={setSelected} />
        <figcaption>
          {custom
            ? 'The screens overlap, so they are not side by side. Pick a side to arrange them.'
            : focus && focusSide
              ? crossing(focusSide, nameOf(state, focus), anchorText(state))
              : headline(state)}
        </figcaption>
      </figure>

      {arr && (
        <div className="controls">
          {externals.length > 1 && (
            <div className="row">
              <span className="row-label" id="chips-label">
                Screen
              </span>
              <div className="chips" role="radiogroup" aria-label="Screen to move">
                {[null, ...externals].map((id) => (
                  <label key={id ?? 'all'} className="chip">
                    <input
                      type="radio"
                      name="screen"
                      checked={selected === id}
                      onChange={() => setSelected(id)}
                    />
                    <span>{id === null ? 'All screens' : nameOf(state, id)}</span>
                  </label>
                ))}
              </div>
            </div>
          )}

          <div className="row">
            <span className="row-label" id="side-label">
              Side
            </span>
            <div className="segmented" role="group" aria-labelledby="side-label">
              {SIDES.map(({ side, label }) => (
                <button
                  key={side}
                  type="button"
                  aria-pressed={currentSide === side}
                  disabled={busy}
                  onClick={() => act(() => backend.moveScreen(selected, side))}
                >
                  <SideIcon side={side} />
                  {label}
                </button>
              ))}
            </div>
          </div>

          <div className="row">
            <span className="row-label" id="align-label">
              Line up
            </span>
            <div className="segmented" role="group" aria-labelledby="align-label">
              {ALIGNS.map((align) => (
                <button
                  key={align}
                  type="button"
                  aria-pressed={!custom && arr.aligned && arr.align === align}
                  disabled={busy}
                  onClick={() => act(() => backend.setAlign(align))}
                >
                  {alignLabel(align, horizontal)}
                </button>
              ))}
            </div>
            {!custom && !arr.aligned && <p className="hint">The screens line up in no named way. Pick one to line them up.</p>}
          </div>

          {caps?.primary && selectedScreen && (
            <div className="row">
              <span className="row-label">Primary</span>
              {selectedScreen.primary ? (
                <p className="row-text">{selectedScreen.name} is the primary screen.</p>
              ) : (
                <button
                  type="button"
                  className="secondary"
                  disabled={busy}
                  onClick={() => act(() => backend.setPrimary(selectedScreen.id))}
                >
                  Make primary
                </button>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
