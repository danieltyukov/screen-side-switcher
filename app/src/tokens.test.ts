import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

// Vitest replaces CSS imports with nothing, so read the file itself. Tests run
// from the repository root (vitest.config.ts) or from app/.
const path = ['app/src/tokens.css', 'src/tokens.css']
  .map((p) => join(process.cwd(), p))
  .find((p) => existsSync(p));
const css = readFileSync(path ?? 'tokens.css', 'utf8');

/*
 * The one rule tokens.css must keep: light is declared in full on bare
 * :root, and the two dark blocks (system dark, explicit dark) are identical
 * and only redefine tokens that exist in light. Drift between the two dark
 * blocks is how this pattern usually breaks.
 */

function declarations(block: string): Map<string, string> {
  const map = new Map<string, string>();
  for (const m of block.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) {
    map.set(m[1]!, m[2]!.trim());
  }
  return map;
}

function blockAfter(selector: string): string {
  const start = css.indexOf(selector);
  expect(start, `${selector} is missing`).toBeGreaterThanOrEqual(0);
  const open = css.indexOf('{', start);
  const close = css.indexOf('}', open);
  return css.slice(open + 1, close);
}

describe('tokens.css', () => {
  const light = declarations(blockAfter(':root {'));
  const systemDark = declarations(blockAfter(":root:not([data-theme='light'])"));
  const explicitDark = declarations(blockAfter(":root[data-theme='dark']"));

  it('declares a full light palette', () => {
    for (const name of ['--bg', '--surface', '--text', '--muted', '--accent', '--pointer', '--focus']) {
      expect(light.has(name), name).toBe(true);
    }
  });

  it('keeps the two dark blocks identical', () => {
    expect([...systemDark.entries()].sort()).toEqual([...explicitDark.entries()].sort());
    expect(systemDark.size).toBeGreaterThan(5);
  });

  it('only redefines tokens that light declares', () => {
    for (const name of systemDark.keys()) {
      expect(light.has(name), `${name} has no light value`).toBe(true);
    }
  });
});
