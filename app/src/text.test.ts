import { describe, expect, it } from 'vitest';
import { createMock } from './backend/mock';
import { alignLabel, crossing, headline } from './text';

describe('text', () => {
  it('says where the pointer crosses', () => {
    expect(crossing('left', 'DELL U2723QE', 'the built-in screen')).toBe(
      'The pointer leaves the left edge of the built-in screen and enters the right edge of DELL U2723QE.',
    );
    expect(crossing('above', 'LG', 'the built-in screen')).toContain('top edge of the built-in screen');
  });

  it('names alignments by the axis they line up on', () => {
    expect(alignLabel('start', true)).toBe('Top');
    expect(alignLabel('end', true)).toBe('Bottom');
    expect(alignLabel('start', false)).toBe('Left');
    expect(alignLabel('center', false)).toBe('Centred');
  });

  it('opens with the same headline as the command line', async () => {
    expect(headline(await createMock({ screens: 2 }).getState())).toBe(
      'External screen is left of the built-in screen.',
    );
    expect(headline(await createMock({ screens: 3 }).getState())).toBe(
      'DELL U2723QE is left of the built-in screen; LG HDR 4K is above the built-in screen.',
    );
    expect(headline(await createMock({ screens: 1 }).getState())).toBe('Only one screen is switched on.');
  });
});
