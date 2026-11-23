import type { SessionMeta } from '@cohors/protocol';
import { isLive } from '../session/status.ts';

/** Most terminals shown side by side; beyond that each gets too small. */
export const MAX_PANES = 4;

/**
 * Sessions shown together in the main area. A single pane is the normal
 * view; the focused pane is the selected session.
 */
export interface Layout {
  panes: string[];
  selectedId: string | null;
}

/** Plain click: show the session, in place of the focused pane when split. */
export function selectSession(layout: Layout, id: string, live: boolean): Layout {
  if (layout.panes.includes(id)) return { ...layout, selectedId: id };
  if (!live || layout.panes.length < 2) return { panes: [id], selectedId: id };
  const panes = layout.panes.map((pane) => (pane === layout.selectedId ? id : pane));
  return { panes, selectedId: id };
}

/** Modifier click or "Dividir": add a live session next to the others, or take it out. */
export function toggleSplit(layout: Layout, id: string): Layout {
  if (layout.panes.includes(id)) return closePane(layout, id);
  const panes = layout.selectedId === null ? [] : layout.panes;
  if (panes.length >= MAX_PANES) return layout;
  return { panes: [...panes, id], selectedId: id };
}

/** Moves a pane to another position; the focus stays on it. */
export function movePane(layout: Layout, id: string, index: number): Layout {
  const from = layout.panes.indexOf(id);
  const to = Math.max(0, Math.min(index, layout.panes.length - 1));
  if (from === -1 || from === to) return layout;
  const panes = layout.panes.filter((pane) => pane !== id);
  panes.splice(to, 0, id);
  return { panes, selectedId: id };
}

/** How the split divides the space: the first column's and first row's share. */
export interface SplitSizes {
  column: number;
  row: number;
}

export const EVEN_SPLIT: SplitSizes = { column: 0.5, row: 0.5 };
/** Each pane keeps at least this share, so a terminal never collapses. */
export const MIN_SHARE = 0.2;

export function clampShare(share: number): number {
  return Math.min(1 - MIN_SHARE, Math.max(MIN_SHARE, share));
}

export function closePane(layout: Layout, id: string): Layout {
  const index = layout.panes.indexOf(id);
  if (index === -1 || layout.panes.length === 1) return layout;
  const panes = layout.panes.filter((pane) => pane !== id);
  const selectedId =
    layout.selectedId === id ? (panes[Math.min(index, panes.length - 1)] ?? null) : layout.selectedId;
  return { panes, selectedId };
}

/**
 * Keeps a split made of live sessions only: one that ends or disappears
 * leaves it. If the focused session ended, it stays shown alone.
 */
export function pruneLayout(layout: Layout, sessions: SessionMeta[]): Layout {
  if (layout.panes.length < 2) return layout;
  const byId = new Map(sessions.map((session) => [session.id, session]));
  const live = layout.panes.filter((pane) => {
    const session = byId.get(pane);
    return session !== undefined && isLive(session.status);
  });
  if (live.length === layout.panes.length) return layout;
  const selected = layout.selectedId === null ? undefined : byId.get(layout.selectedId);
  if (selected !== undefined && !isLive(selected.status)) {
    return { panes: [selected.id], selectedId: selected.id };
  }
  const selectedId =
    layout.selectedId !== null && live.includes(layout.selectedId)
      ? layout.selectedId
      : (live[0] ?? null);
  return { panes: live.length > 0 ? live : selectedId === null ? [] : [selectedId], selectedId };
}
