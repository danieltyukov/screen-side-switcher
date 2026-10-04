import type { Side } from './backend/types';

/*
 * Small line icons drawn on a 16-unit grid in currentColor, so they follow
 * the text colour in every state and theme. Decorative: the buttons that
 * hold them carry the words.
 */

const ROTATION: Record<Side, number> = { left: 0, above: 90, right: 180, below: 270 };

export function SideIcon({ side }: { side: Side }) {
  return (
    <svg className="icon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" focusable="false">
      <g transform={`rotate(${ROTATION[side]} 8 8)`}>
        <rect x="8.5" y="4" width="6" height="8" rx="1" fill="none" stroke="currentColor" strokeWidth="1.4" />
        <path d="M6.5 8 H1.5 M3.8 5.7 L1.5 8 L3.8 10.3" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
      </g>
    </svg>
  );
}

export function Mark({ size = 22 }: { size?: number }) {
  return (
    <svg viewBox="0 0 32 32" width={size} height={size} aria-hidden="true" focusable="false" className="mark">
      <rect x="2" y="8" width="13" height="10" rx="2" fill="none" stroke="currentColor" strokeWidth="2" />
      <rect x="17" y="6" width="13" height="12" rx="2" fill="none" stroke="currentColor" strokeWidth="2" />
      <path d="M17 23 H30" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
      <path d="M15.9 9 V17" stroke="var(--pointer)" strokeWidth="2.6" strokeLinecap="round" />
    </svg>
  );
}
