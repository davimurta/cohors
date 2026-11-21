import { describe, expect, it } from 'vitest';
import { displayPath, formatElapsed } from './format.ts';

const start = '2026-01-01T00:00:00Z';
const at = (ms: number) => Date.parse(start) + ms;

describe('formatElapsed', () => {
  it('formats each range', () => {
    expect(formatElapsed(start, at(30_000))).toBe('agora');
    expect(formatElapsed(start, at(42 * 60_000))).toBe('há 42 min');
    expect(formatElapsed(start, at(3 * 3_600_000))).toBe('há 3 h');
    expect(formatElapsed(start, at(24 * 3_600_000))).toBe('há 1 dia');
    expect(formatElapsed(start, at(50 * 3_600_000))).toBe('há 2 dias');
  });

  it('treats clock skew as now', () => {
    expect(formatElapsed(start, at(-5_000))).toBe('agora');
  });
});

describe('displayPath', () => {
  it('abbreviates the home folder', () => {
    expect(displayPath('/Users/me/dev/cohors', '/Users/me')).toBe('~/dev/cohors');
    expect(displayPath('/Users/me', '/Users/me/')).toBe('~');
  });

  it('keeps other paths and similar prefixes', () => {
    expect(displayPath('/Users/meow/dev', '/Users/me')).toBe('/Users/meow/dev');
    expect(displayPath('/tmp', '/Users/me')).toBe('/tmp');
    expect(displayPath('/tmp', null)).toBe('/tmp');
  });

  it('handles windows separators', () => {
    expect(displayPath('C:\\Users\\me\\dev', 'C:\\Users\\me')).toBe('~\\dev');
  });
});
