import { describe, expect, it } from 'vitest';
import { normalizeWhitespace, previewText } from './format';
import { fuzzyMatch, highlightMatch } from './fuzzy';

describe('search previews', () => {
  it('highlights the displayed positions across collapsed whitespace', () => {
    const { preview } = previewText('  alpha\n\t  beta  ');
    const match = fuzzyMatch(preview, normalizeWhitespace('  alpha\n beta '))!;
    expect(preview).toBe('alpha beta');
    expect(fuzzyMatch(preview, 'beta')!.indices).toEqual([6, 7, 8, 9]);
    expect(match.indices).toHaveLength(preview.length);
    expect(highlightMatch(preview, fuzzyMatch(preview, 'beta')!.indices).filter(s => s.match).map(s => s.text).join('')).toBe('beta');
  });
  it('maps expanding lowercase and astral characters to original positions', () => {
    const text = 'İ 😀 Beta';
    expect(fuzzyMatch(text, 'beta')!.indices).toEqual([5, 6, 7, 8]);
    expect(highlightMatch(text, fuzzyMatch(text, '😀')!.indices).filter(s => s.match).map(s => s.text).join('')).toBe('😀');
    expect(fuzzyMatch('İx', 'i\u0307')!.indices).toEqual([0]);
  });
});
