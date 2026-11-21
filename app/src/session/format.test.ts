import { describe, expect, it } from 'vitest';
import { displayPath, formatElapsed, formatLastActivity } from './format.ts';

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

describe('formatLastActivity', () => {
  // Local times, so the test holds in any time zone.
  const now = new Date(2026, 10, 20, 21, 0).getTime();
  const iso = (...parts: [number, number, number, number, number]) =>
    new Date(...parts).toISOString();

  it('names today and yesterday', () => {
    expect(formatLastActivity(iso(2026, 10, 20, 18, 42), now)).toBe('hoje, 18:42');
    expect(formatLastActivity(iso(2026, 10, 19, 9, 5), now)).toBe('ontem, 09:05');
  });

  it('uses the date for older days', () => {
    expect(formatLastActivity(iso(2026, 10, 12, 18, 42), now)).toBe('12/11, 18:42');
    expect(formatLastActivity(iso(2025, 11, 31, 23, 59), now)).toBe('31/12/2025, 23:59');
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
