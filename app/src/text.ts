import type { Align, AppState, Side } from './backend/types';

/*
 * The sentences the interface shows, in the same words the command line
 * uses, so the two never describe one arrangement differently.
 */

const EDGES: Record<Side, [string, string]> = {
  left: ['left', 'right'],
  right: ['right', 'left'],
  above: ['top', 'bottom'],
  below: ['bottom', 'top'],
};

const RELATION: Record<Side, string> = {
  left: 'left of',
  right: 'right of',
  above: 'above',
  below: 'below',
};

export function relation(side: Side): string {
  return RELATION[side];
}

export function crossing(side: Side, screenName: string, anchorText: string): string {
  const [from, to] = EDGES[side];
  return `The pointer leaves the ${from} edge of ${anchorText} and enters the ${to} edge of ${screenName}.`;
}

export function alignLabel(align: Align, horizontal: boolean): string {
  if (align === 'center') return 'Centred';
  if (align === 'start') return horizontal ? 'Top' : 'Left';
  return horizontal ? 'Bottom' : 'Right';
}

export function isHorizontal(side: Side): boolean {
  return side === 'left' || side === 'right';
}

/** "the built-in screen", or the anchor's name on a desktop. */
export function anchorText(state: AppState): string {
  const anchor = state.screens.find((s) => s.id === state.arrangement?.anchor);
  if (!anchor) return 'the main screen';
  return anchor.builtin ? 'the built-in screen' : anchor.name;
}

export function nameOf(state: AppState, id: string): string {
  return state.screens.find((s) => s.id === id)?.name ?? id;
}

export function headline(state: AppState): string {
  const on = state.screens.filter((s) => s.enabled).length;
  if (on === 0) return 'No screen is switched on.';
  if (on === 1) return 'Only one screen is switched on.';
  const arr = state.arrangement;
  if (!arr) return 'The arrangement is not a simple side-by-side layout.';
  const anchor = anchorText(state);
  const builtin = state.screens.find((s) => s.id === arr.anchor)?.builtin ?? false;
  if (arr.placements.length === 1 && builtin) {
    return `External screen is ${relation(arr.placements[0]!.side)} ${anchor}.`;
  }
  return `${arr.placements.map((p) => `${nameOf(state, p.screen)} is ${relation(p.side)} ${anchor}`).join('; ')}.`;
}

/** `<Super><Alt>s` as people read it. */
export function humanKeys(keys: string): string {
  const parts: string[] = [];
  let rest = keys;
  for (let m = rest.match(/^<([^>]+)>/); m; m = rest.match(/^<([^>]+)>/)) {
    parts.push(m[1] === 'Primary' || m[1] === 'Control' ? 'Ctrl' : m[1]!);
    rest = rest.slice(m[0].length);
  }
  if (rest) parts.push(rest.length === 1 ? rest.toUpperCase() : rest);
  return parts.join('+');
}
