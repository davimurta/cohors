import { describe, expect, it } from 'vitest';
import { parseLayout } from './persist.ts';

describe('parseLayout', () => {
  it('reads a saved layout', () => {
    const saved = {
      panes: ['a', 'b'],
      selectedId: 'b',
      sidebarTab: 'active',
      splitSizes: { column: 0.6, row: 0.5 },
    };
    expect(parseLayout(saved)).toEqual(saved);
  });

  it('rejects broken data and repairs sizes', () => {
    expect(parseLayout(null)).toBeNull();
    expect(parseLayout({ panes: [1], selectedId: null, sidebarTab: 'active' })).toBeNull();
    expect(parseLayout({ panes: [], selectedId: null, sidebarTab: 'other' })).toBeNull();
    expect(
      parseLayout({ panes: [], selectedId: null, sidebarTab: 'finished', splitSizes: { column: 3, row: 0 } })
        ?.splitSizes,
    ).toEqual({ column: 0.8, row: 0.2 });
  });
});
