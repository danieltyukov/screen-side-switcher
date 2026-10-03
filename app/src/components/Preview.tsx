import type { KeyboardEvent } from 'react';
import type { Arrangement, ScreenInfo } from '../backend/types';
import { relation } from '../text';
import './Preview.css';

/*
 * The arrangement drawn to scale, the way the screens stand on the desk:
 * monitors on a stand, the laptop on its hinge. The edge the pointer
 * crosses is the only thing drawn in the pointer yellow.
 *
 * Coordinates are the backend's own (logical pixels, physical pixels or
 * points); the viewBox takes care of scale, so nothing here converts units.
 */

interface Props {
  screens: ScreenInfo[];
  arrangement: Arrangement | null;
  /** A screen id, or null for "all screens". */
  selected: string | null;
  onSelect: (id: string | null) => void;
}

interface Edge {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  /** Which way the pointer travels across it, as a unit vector. */
  dx: number;
  dy: number;
}

function sharedEdge(from: ScreenInfo, to: ScreenInfo): Edge | null {
  const overlapY = [Math.max(from.y, to.y), Math.min(from.y + from.height, to.y + to.height)] as const;
  const overlapX = [Math.max(from.x, to.x), Math.min(from.x + from.width, to.x + to.width)] as const;
  if (overlapY[1] > overlapY[0]) {
    if (from.x + from.width === to.x) return { x1: to.x, y1: overlapY[0], x2: to.x, y2: overlapY[1], dx: 1, dy: 0 };
    if (to.x + to.width === from.x) return { x1: from.x, y1: overlapY[0], x2: from.x, y2: overlapY[1], dx: -1, dy: 0 };
  }
  if (overlapX[1] > overlapX[0]) {
    if (from.y + from.height === to.y) return { x1: overlapX[0], y1: to.y, x2: overlapX[1], y2: to.y, dx: 0, dy: 1 };
    if (to.y + to.height === from.y) return { x1: overlapX[0], y1: from.y, x2: overlapX[1], y2: from.y, dx: 0, dy: -1 };
  }
  return null;
}

/** The edge the pointer crosses to reach `target`: from the anchor if they touch, else from its nearest neighbour. */
function edgeInto(target: ScreenInfo, anchor: ScreenInfo, all: ScreenInfo[]): Edge | null {
  const direct = sharedEdge(anchor, target);
  if (direct) return direct;
  let best: Edge | null = null;
  let length = 0;
  for (const other of all) {
    if (other.id === target.id) continue;
    const e = sharedEdge(other, target);
    const l = e ? Math.hypot(e.x2 - e.x1, e.y2 - e.y1) : 0;
    if (e && l > length) {
      best = e;
      length = l;
    }
  }
  return best;
}

function hasScreenBelow(s: ScreenInfo, all: ScreenInfo[]): boolean {
  return all.some(
    (o) => o.id !== s.id && o.y >= s.y + s.height && o.y <= s.y + s.height * 1.2 && o.x < s.x + s.width && s.x < o.x + o.width,
  );
}

/** The longest label that fits: the name, "Built-in", then the connector. */
function fit(s: ScreenInfo, fontSize: number): string {
  const room = s.width * 0.86;
  const wide = (t: string) => t.length * fontSize * 0.56;
  const candidates = [s.name, s.builtin ? 'Built-in' : '', s.connector].filter(Boolean);
  return candidates.find((t) => wide(t) <= room) ?? '';
}

export function Preview({ screens, arrangement, selected, onSelect }: Props) {
  const on = screens.filter((s) => s.enabled);
  const off = screens.filter((s) => !s.enabled);
  const anchor = on.find((s) => s.id === arrangement?.anchor) ?? on.find((s) => s.builtin) ?? on[0];

  if (on.length === 0) {
    return <p className="preview-empty">No screen is switched on.</p>;
  }

  const minX = Math.min(...on.map((s) => s.x));
  const minY = Math.min(...on.map((s) => s.y));
  const maxX = Math.max(...on.map((s) => s.x + s.width));
  const maxY = Math.max(...on.map((s) => s.y + s.height));
  const span = Math.max(maxX - minX, maxY - minY);
  const unit = span / 60;
  const pad = unit * 4;
  const standRoom = Math.max(...on.map((s) => s.height)) * 0.16;
  const viewBox = `${minX - pad} ${minY - pad} ${maxX - minX + 2 * pad} ${maxY - minY + 2 * pad + standRoom}`;

  const sideOf = (id: string) => arrangement?.placements.find((p) => p.screen === id)?.side;
  const anchorWords = anchor?.builtin ? 'the built-in screen' : (anchor?.name ?? 'the main screen');

  // The selected screen, or with "all" and a single other screen, that one.
  const focus =
    selected ?? (arrangement && arrangement.placements.length === 1 ? arrangement.placements[0]!.screen : null);
  const lit = (focus ? [focus] : (arrangement?.placements.map((p) => p.screen) ?? []))
    .map((id) => on.find((s) => s.id === id))
    .filter((s): s is ScreenInfo => !!s && !!anchor && s.id !== anchor.id)
    .map((s) => ({ id: s.id, edge: edgeInto(s, anchor!, on) }))
    .filter((e): e is { id: string; edge: Edge } => e.edge !== null);
  const pointerEdge = focus ? lit.find((l) => l.id === focus)?.edge : undefined;

  const select = (s: ScreenInfo) => onSelect(anchor && s.id === anchor.id ? null : s.id);
  const onKey = (event: KeyboardEvent, s: ScreenInfo) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      select(s);
    }
  };

  const fontSize = unit * 2.4;

  return (
    <div className="preview">
      <svg viewBox={viewBox} preserveAspectRatio="xMidYMid meet" className="preview-svg">
        <title>The screens as they are arranged</title>
        {on.map((s) => {
          const side = sideOf(s.id);
          const label =
            `${s.name}` +
            (side ? `, ${relation(side)} ${anchorWords}` : '') +
            (s.primary ? ', primary' : '');
          const bezel = Math.max(unit * 0.55, Math.min(s.width, s.height) * 0.025);
          const isSelected = selected === s.id;
          const text = fit(s, fontSize);
          const stand = !hasScreenBelow(s, on);
          return (
            <g
              key={s.id}
              role="button"
              tabIndex={0}
              aria-label={label}
              aria-pressed={isSelected}
              className="screen"
              data-selected={isSelected || undefined}
              style={{ transform: `translate(${s.x}px, ${s.y}px)` }}
              onClick={() => select(s)}
              onKeyDown={(e) => onKey(e, s)}
            >
              {stand &&
                (s.builtin ? (
                  <path
                    className="base"
                    d={`M ${-unit} ${s.height} H ${s.width + unit} L ${s.width - unit * 0.5} ${s.height + unit * 1.3} H ${unit * 0.5} Z`}
                  />
                ) : (
                  <path
                    className="base"
                    d={`M ${s.width / 2 - unit} ${s.height} h ${unit * 2} v ${standRoom * 0.55} h ${unit * 3} v ${unit * 0.9} h ${-unit * 8} v ${-unit * 0.9} h ${unit * 3} Z`}
                  />
                ))}
              <rect className="bezel" width={s.width} height={s.height} rx={bezel * 1.2} />
              <rect
                className={s.primary ? 'glass primary' : 'glass'}
                x={bezel}
                y={bezel}
                width={s.width - 2 * bezel}
                height={s.height - 2 * bezel}
                rx={bezel * 0.5}
              />
              {text && (
                <text className="name" x={s.width / 2} y={s.height / 2} fontSize={fontSize} textAnchor="middle" dominantBaseline="middle">
                  {text}
                </text>
              )}
              {s.primary && (
                <text className="tag" x={s.width / 2} y={s.height / 2 + fontSize * 1.5} fontSize={fontSize * 0.8} textAnchor="middle" dominantBaseline="middle">
                  Primary
                </text>
              )}
            </g>
          );
        })}
        {lit.map(({ id, edge }) => (
          <line
            key={id}
            data-edge={id}
            className="edge"
            x1={edge.x1}
            y1={edge.y1}
            x2={edge.x2}
            y2={edge.y2}
            strokeWidth={unit * 0.7}
          />
        ))}
        {pointerEdge && (
          <g
            data-pointer=""
            className="pointer"
            style={{
              transform: `translate(${(pointerEdge.x1 + pointerEdge.x2) / 2 - pointerEdge.dx * unit * 2.4}px, ${(pointerEdge.y1 + pointerEdge.y2) / 2 - pointerEdge.dy * unit * 2.4}px)`,
              ['--travel-x' as string]: `${pointerEdge.dx * unit * 4.2}px`,
              ['--travel-y' as string]: `${pointerEdge.dy * unit * 4.2}px`,
            }}
          >
            <path
              className="cursor"
              transform={`scale(${unit * 0.24})`}
              d="M0 0 L0 15 L4 11.5 L6.6 17.4 L9 16.4 L6.4 10.6 L11.4 10.6 Z"
              strokeWidth={1.4}
            />
          </g>
        )}
      </svg>
      {off.length > 0 && (
        <ul className="preview-off">
          {off.map((s) => (
            <li key={s.id}>{s.name} is off.</li>
          ))}
        </ul>
      )}
    </div>
  );
}
