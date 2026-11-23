import { EVEN_SPLIT, clampShare, type SplitSizes } from './layout.ts';
import type { SidebarTab } from './sessions.ts';

const KEY = 'cohors.layout';

/** What the app restores on the next start: the view, not any session data. */
export interface SavedLayout {
  panes: string[];
  selectedId: string | null;
  sidebarTab: SidebarTab;
  splitSizes: SplitSizes;
}

/**
 * The saved layout, or null when there is none or it is unreadable. Storage
 * can be missing or blocked, so every access is guarded.
 */
export function loadLayout(): SavedLayout | null {
  try {
    const raw = window.localStorage.getItem(KEY);
    return raw === null ? null : parseLayout(JSON.parse(raw));
  } catch {
    return null;
  }
}

export function saveLayout(layout: SavedLayout): void {
  try {
    window.localStorage.setItem(KEY, JSON.stringify(layout));
  } catch {
    // Without storage the layout simply starts fresh next time.
  }
}

export function parseLayout(value: unknown): SavedLayout | null {
  if (typeof value !== 'object' || value === null) return null;
  const { panes, selectedId, sidebarTab, splitSizes } = value as Record<string, unknown>;
  if (!Array.isArray(panes) || !panes.every((pane) => typeof pane === 'string')) return null;
  if (selectedId !== null && typeof selectedId !== 'string') return null;
  if (sidebarTab !== 'active' && sidebarTab !== 'finished') return null;
  return { panes, selectedId, sidebarTab, splitSizes: parseSizes(splitSizes) };
}

function parseSizes(value: unknown): SplitSizes {
  if (typeof value !== 'object' || value === null) return EVEN_SPLIT;
  const { column, row } = value as Record<string, unknown>;
  if (typeof column !== 'number' || typeof row !== 'number') return EVEN_SPLIT;
  return { column: clampShare(column), row: clampShare(row) };
}
