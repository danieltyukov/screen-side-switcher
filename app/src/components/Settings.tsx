import { useState, type KeyboardEvent } from 'react';
import type { AppState, Backend, Settings as SettingsShape } from '../backend/types';
import { humanKeys } from '../text';
import './Settings.css';

type Act = (work: () => Promise<AppState>, done?: string) => Promise<void>;

export const LINKS = {
  site: 'https://danieltyukov.github.io/screen-side-switcher/',
  install: 'https://danieltyukov.github.io/screen-side-switcher/#install',
  issues: 'https://github.com/danieltyukov/screen-side-switcher/issues/new/choose',
  discussions: 'https://github.com/danieltyukov/screen-side-switcher/discussions',
};

/** A key press as an accelerator: "Ctrl+Alt+K". Null until it has a key and a modifier. */
export function accelerator(event: Pick<KeyboardEvent, 'key' | 'code' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey'>): string | null {
  if (['Control', 'Alt', 'Shift', 'Meta', 'OS', 'AltGraph'].includes(event.key)) return null;
  const parts: string[] = [];
  if (event.ctrlKey) parts.push('Ctrl');
  if (event.altKey) parts.push('Alt');
  if (event.shiftKey) parts.push('Shift');
  if (event.metaKey) parts.push('Super');
  if (parts.length === 0 || (parts.length === 1 && parts[0] === 'Shift')) return null;
  const code = event.code;
  const key = code.startsWith('Key')
    ? code.slice(3)
    : code.startsWith('Digit')
      ? code.slice(5)
      : /^F\d{1,2}$/.test(code)
        ? code
        : code.startsWith('Arrow')
          ? code.slice(5)
          : event.key.length === 1
            ? event.key.toUpperCase()
            : null;
  return key ? [...parts, key].join('+') : null;
}

function Switch({
  label,
  description,
  checked,
  disabled,
  onChange,
}: {
  label: string;
  description?: string;
  checked: boolean;
  disabled: boolean;
  onChange: (value: boolean) => void;
}) {
  const id = label.toLowerCase().replace(/\W+/g, '-');
  return (
    <div className="setting">
      <div className="setting-text">
        <span id={`${id}-label`} className="setting-label">
          {label}
        </span>
        {description && (
          <span id={`${id}-desc`} className="setting-desc">
            {description}
          </span>
        )}
      </div>
      <button
        type="button"
        role="switch"
        className="switch"
        aria-checked={checked}
        aria-labelledby={`${id}-label`}
        aria-describedby={description ? `${id}-desc` : undefined}
        disabled={disabled}
        onClick={() => onChange(!checked)}
      >
        <span className="switch-track" aria-hidden="true" />
      </button>
    </div>
  );
}

function ShortcutSection({ state, backend, act, busy }: { state: AppState; backend: Backend; act: Act; busy: boolean }) {
  const [recording, setRecording] = useState(false);
  const [pending, setPending] = useState<string | null>(null);
  const support = state.shortcut;

  if (support.kind === 'gnome') {
    return (
      <div className="shortcut">
        <p>
          {support.installed
            ? `${humanKeys(support.keys)} swaps the sides from anywhere. Change the keys in GNOME Settings, Keyboard, Custom Shortcuts.`
            : `Set up a GNOME custom shortcut so ${humanKeys(support.keys)} swaps the sides from anywhere.`}
        </p>
        {support.installed ? (
          <button type="button" className="secondary" disabled={busy} onClick={() => act(() => backend.removeShortcut(), 'Shortcut removed')}>
            Remove shortcut
          </button>
        ) : (
          <button type="button" className="primary" disabled={busy} onClick={() => act(() => backend.installShortcut(), 'Shortcut set')}>
            Set up shortcut
          </button>
        )}
      </div>
    );
  }

  if (support.kind === 'manual') {
    return (
      <div className="shortcut">
        <p>This desktop keeps its own shortcuts, so add one there:</p>
        <pre>{support.instructions.trim()}</pre>
        <button type="button" className="secondary" onClick={() => backend.copyText(support.instructions.trim())}>
          Copy
        </button>
      </div>
    );
  }

  const current = state.settings.shortcut;
  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (!recording) return;
    event.preventDefault();
    if (event.key === 'Escape') {
      setRecording(false);
      return;
    }
    const keys = accelerator(event);
    if (keys) {
      setPending(keys);
      setRecording(false);
    }
  };

  return (
    <div className="shortcut">
      <p>Swap the sides from anywhere while Screen Side runs.</p>
      <div className="shortcut-row">
        <button
          type="button"
          className={recording ? 'recorder recording' : 'recorder'}
          onClick={() => setRecording(true)}
          onKeyDown={onKeyDown}
          onBlur={() => setRecording(false)}
          aria-label={recording ? 'Press the keys' : 'Record shortcut'}
        >
          {recording ? 'Press the keys' : (pending ?? current ?? 'Record shortcut')}
        </button>
        {pending && pending !== current && (
          <button
            type="button"
            className="primary"
            disabled={busy}
            onClick={async () => {
              await act(() => backend.updateSettings({ shortcut: pending }), `Shortcut set to ${pending}`);
              setPending(null);
            }}
          >
            Save shortcut
          </button>
        )}
        {(current || pending) && (
          <button
            type="button"
            className="quiet"
            disabled={busy}
            onClick={async () => {
              setPending(null);
              await act(() => backend.updateSettings({ shortcut: null }), 'Shortcut cleared');
            }}
          >
            Clear
          </button>
        )}
      </div>
    </div>
  );
}

export function Settings({ state, backend, act, busy }: { state: AppState; backend: Backend; act: Act; busy: boolean }) {
  const set = (patch: Partial<SettingsShape>) => act(() => backend.updateSettings(patch));
  const s = state.settings;

  return (
    <div className="settings">
      <section aria-labelledby="general-heading">
        <h2 id="general-heading">General</h2>
        <Switch
          label="Keep running in the background"
          description={
            state.platform === 'linux'
              ? 'Shows a tray icon and puts saved layouts back when screens connect. On GNOME the tray icon needs the AppIndicator extension.'
              : 'Shows a tray icon and puts saved layouts back when screens connect.'
          }
          checked={s.background}
          disabled={busy}
          onChange={(background) => set({ background })}
        />
        <Switch label="Start at login" checked={s.autoStart} disabled={busy} onChange={(autoStart) => set({ autoStart })} />
        <Switch
          label="Apply saved layouts automatically"
          description="For layouts marked Auto, when their screens connect."
          checked={s.autoApply}
          disabled={busy}
          onChange={(autoApply) => set({ autoApply })}
        />
      </section>

      <section aria-labelledby="shortcut-heading">
        <h2 id="shortcut-heading">Keyboard shortcut</h2>
        <ShortcutSection state={state} backend={backend} act={act} busy={busy} />
      </section>

      <section aria-labelledby="cli-heading">
        <h2 id="cli-heading">Command line</h2>
        {state.cliPath ? (
          <p>
            <code>{state.cliPath}</code>
          </p>
        ) : (
          <p>
            Not installed.{' '}
            <button type="button" className="link" onClick={() => backend.openUrl(LINKS.install)}>
              How to install it
            </button>
          </p>
        )}
      </section>

      <section aria-labelledby="help-heading">
        <h2 id="help-heading">Help</h2>
        <div className="help-row">
          <button
            type="button"
            className="secondary"
            onClick={() =>
              act(async () => {
                await backend.copyText(await backend.diagnostics());
                return backend.getState();
              }, 'Diagnostics copied')
            }
          >
            Copy diagnostics
          </button>
          <span className="setting-desc">Paste them into a bug report.</span>
        </div>
        <p className="about">Screen Side {state.version}</p>
        <p className="links">
          <button type="button" className="link" onClick={() => backend.openUrl(LINKS.site)}>
            Website
          </button>
          <button type="button" className="link" onClick={() => backend.openUrl(LINKS.issues)}>
            Report a problem
          </button>
          <button type="button" className="link" onClick={() => backend.openUrl(LINKS.discussions)}>
            Ideas and questions
          </button>
        </p>
      </section>
    </div>
  );
}
