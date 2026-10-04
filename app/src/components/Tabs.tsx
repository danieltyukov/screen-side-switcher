import { useRef, type KeyboardEvent } from 'react';
import './Tabs.css';

export type Tab = 'arrange' | 'layouts' | 'settings';

const TABS: { id: Tab; label: string }[] = [
  { id: 'arrange', label: 'Arrange' },
  { id: 'layouts', label: 'Layouts' },
  { id: 'settings', label: 'Settings' },
];

/** WAI-ARIA tabs: one tab stop, arrow keys and Home/End move between tabs. */
export function Tabs({ current, onChange }: { current: Tab; onChange: (tab: Tab) => void }) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);

  const move = (event: KeyboardEvent, index: number) => {
    const last = TABS.length - 1;
    const next =
      event.key === 'ArrowRight' ? (index === last ? 0 : index + 1)
      : event.key === 'ArrowLeft' ? (index === 0 ? last : index - 1)
      : event.key === 'Home' ? 0
      : event.key === 'End' ? last
      : null;
    if (next === null) return;
    event.preventDefault();
    onChange(TABS[next]!.id);
    refs.current[next]?.focus();
  };

  return (
    <div className="tabs" role="tablist" aria-label="Screen Side">
      {TABS.map((tab, i) => (
        <button
          key={tab.id}
          ref={(el) => {
            refs.current[i] = el;
          }}
          type="button"
          role="tab"
          id={`tab-${tab.id}`}
          aria-selected={current === tab.id}
          aria-controls={`panel-${tab.id}`}
          tabIndex={current === tab.id ? 0 : -1}
          onClick={() => onChange(tab.id)}
          onKeyDown={(e) => move(e, i)}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}
