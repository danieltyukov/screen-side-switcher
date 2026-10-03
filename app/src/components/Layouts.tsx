import { useState, type FormEvent } from 'react';
import type { AppState, Backend } from '../backend/types';
import './Layouts.css';

type Act = (work: () => Promise<AppState>, done?: string) => Promise<void>;

export function Layouts({ state, backend, act, busy }: { state: AppState; backend: Backend; act: Act; busy: boolean }) {
  const [name, setName] = useState('');
  const [auto, setAuto] = useState(false);
  const [confirming, setConfirming] = useState<string | null>(null);
  const canSave = state.arrangement !== null;

  const save = async (event: FormEvent) => {
    event.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) return;
    const replacing = state.layouts.some((l) => l.name.toLowerCase() === trimmed.toLowerCase());
    await act(() => backend.saveLayout(trimmed, auto), `${replacing ? 'Replaced' : 'Saved'} ${trimmed}`);
    setName('');
    setAuto(false);
  };

  return (
    <div className="layouts">
      {state.layouts.length === 0 ? (
        <p className="empty">
          No layouts yet. Arrange the screens the way this desk needs them, then save the arrangement here to get it
          back next time.
        </p>
      ) : (
        <ul className="layout-list">
          {state.layouts.map((l) => (
            <li key={l.name} className={l.matches ? 'layout' : 'layout other'}>
              <div className="layout-text">
                <span className="layout-name">
                  {l.name}
                  {state.activeLayout === l.name && <span className="badge">In use</span>}
                </span>
                <span className="layout-summary">{l.matches ? l.summary : 'For other screens'}</span>
              </div>
              {confirming === l.name ? (
                <div className="layout-actions">
                  <span>Delete {l.name}?</span>
                  <button
                    type="button"
                    className="danger"
                    aria-label={`Delete ${l.name}`}
                    onClick={async () => {
                      setConfirming(null);
                      await act(() => backend.forgetLayout(l.name), `Deleted ${l.name}`);
                    }}
                  >
                    Delete
                  </button>
                  <button type="button" className="quiet" onClick={() => setConfirming(null)}>
                    Cancel
                  </button>
                </div>
              ) : (
                <div className="layout-actions">
                  {l.matches && (
                    <button
                      type="button"
                      className="secondary"
                      disabled={busy}
                      onClick={() => act(() => backend.applyLayout(l.name), `Applied ${l.name}`)}
                    >
                      Apply
                    </button>
                  )}
                  <button
                    type="button"
                    role="switch"
                    className="switch"
                    aria-checked={l.auto}
                    aria-label="Apply automatically"
                    title="Apply automatically when these screens connect"
                    disabled={busy}
                    onClick={() => act(() => backend.setLayoutAuto(l.name, !l.auto))}
                  >
                    <span className="switch-track" aria-hidden="true" />
                    <span className="switch-text">Auto</span>
                  </button>
                  <button type="button" className="quiet" onClick={() => setConfirming(l.name)}>
                    Delete
                  </button>
                </div>
              )}
            </li>
          ))}
        </ul>
      )}

      <form className="save" onSubmit={save}>
        <h2>Save current layout</h2>
        {!canSave && <p className="hint">Connect a second screen to save a layout.</p>}
        <label className="field">
          <span>Name</span>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            maxLength={64}
            required
            placeholder="Office, Home desk"
            disabled={!canSave}
          />
        </label>
        <label className="check">
          <input type="checkbox" checked={auto} onChange={(e) => setAuto(e.target.checked)} disabled={!canSave} />
          <span>Apply automatically when these screens connect</span>
        </label>
        <button type="submit" className="primary" disabled={!canSave || busy || !name.trim()}>
          Save
        </button>
      </form>
    </div>
  );
}
