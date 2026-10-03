import { useCallback, useEffect, useState } from 'react';
import type { AppState, Backend } from './backend/types';
import { Arrange } from './components/Arrange';
import { Banner } from './components/Banner';
import { Layouts } from './components/Layouts';
import { LINKS, Settings } from './components/Settings';
import { Tabs, type Tab } from './components/Tabs';
import './App.css';

/** Tauri rejects with the error text itself; everything else throws an Error. */
function message(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

export default function App({ backend }: { backend: Backend }) {
  const [state, setState] = useState<AppState | null>(null);
  const [tab, setTab] = useState<Tab>('arrange');
  const [problem, setProblem] = useState<string | null>(null);
  const [status, setStatus] = useState('');
  const [busy, setBusy] = useState(false);

  const reload = useCallback(async () => {
    try {
      setState(await backend.getState());
    } catch (error) {
      setProblem(message(error));
    }
  }, [backend]);

  useEffect(() => {
    void reload();
    return backend.onChange(() => void reload());
  }, [backend, reload]);

  // Every change goes through here: one call at a time, the new state on
  // success, and on refusal the reason in the banner and the real state back.
  const act = useCallback(
    async (work: () => Promise<AppState>, done?: string) => {
      setBusy(true);
      setProblem(null);
      setStatus('');
      try {
        setState(await work());
        if (done) setStatus(done);
      } catch (error) {
        setProblem(message(error));
        await reload();
      } finally {
        setBusy(false);
      }
    },
    [reload],
  );

  const changeTab = (next: Tab) => {
    setTab(next);
    setStatus('');
  };

  return (
    <div className="app">
      <h1 className="visually-hidden">Screen Side</h1>
      <header className="app-head">
        <Tabs current={tab} onChange={changeTab} />
      </header>
      {problem && <Banner message={problem} onDismiss={() => setProblem(null)} />}
      <main role="tabpanel" id={`panel-${tab}`} aria-labelledby={`tab-${tab}`} tabIndex={-1} className="panel">
        {!state ? (
          <p className="loading">Reading the screens.</p>
        ) : tab === 'arrange' ? (
          state.error && state.screens.length === 0 ? (
            <div className="no-backend">
              <div role="alert" className="no-backend-message">
                <p>{state.error}</p>
              </div>
              <p>If this looks wrong, copy the diagnostics into a bug report.</p>
              <div className="help-row">
                <button
                  type="button"
                  className="primary"
                  onClick={() =>
                    act(async () => {
                      await backend.copyText(await backend.diagnostics());
                      return backend.getState();
                    }, 'Diagnostics copied')
                  }
                >
                  Copy diagnostics
                </button>
                <button type="button" className="link" onClick={() => backend.openUrl(LINKS.issues)}>
                  Report a problem
                </button>
              </div>
            </div>
          ) : (
            <Arrange state={state} backend={backend} act={act} busy={busy} />
          )
        ) : tab === 'layouts' ? (
          <Layouts state={state} backend={backend} act={act} busy={busy} />
        ) : (
          <Settings state={state} backend={backend} act={act} busy={busy} />
        )}
      </main>
      <p className="status" role="status" aria-live="polite">
        {status}
      </p>
    </div>
  );
}
