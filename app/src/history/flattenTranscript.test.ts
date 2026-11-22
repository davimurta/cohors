import { describe, expect, it } from 'vitest';
import { flattenTranscript } from './flattenTranscript.ts';

const ESC = '\u001b';

describe('flattenTranscript', () => {
  it('drops alternate screen switches and scrollback clears', () => {
    const raw = `a${ESC}[?1049hb${ESC}[3Jc${ESC}[?1049l${ESC}[?47hd${ESC}[?1047l`;
    expect(flattenTranscript(raw, 3)).toBe('abcd');
  });

  it('scrolls the screen into the scrollback before a clear', () => {
    expect(flattenTranscript(`old${ESC}[2Jnew`, 2)).toBe(`old${ESC}[2;1H\n\n${ESC}[2Jnew`);
  });

  it('keeps other sequences', () => {
    const raw = `${ESC}[1;31mred${ESC}[0m${ESC}[?2004h`;
    expect(flattenTranscript(raw, 3)).toBe(raw);
  });
});
