import { useEffect } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { sidebarOrder } from '../projects/groupSessions.ts';
import { isMac } from '../platform.ts';
import { useAppStore } from '../state/store.ts';

/** Menu item event sent by the macOS menu bar for Cmd+W. */
const CLOSE_PANE_EVENT = 'close-pane';

/**
 * Cmd+1 to Cmd+9 (Ctrl elsewhere) open the active sessions in sidebar order.
 * Cmd+W on macOS, through the menu bar, and Ctrl+Shift+W elsewhere close the
 * focused split pane; plain Ctrl+W stays with the terminal, where shells
 * use it to delete a word. Capture phase, so the focused terminal does not
 * get the keys first.
 */
export function useSessionShortcuts(): void {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const modifier = isMac ? event.metaKey : event.ctrlKey;
      if (!modifier || event.altKey) return;
      const position = Number(event.key);
      if (!event.shiftKey && Number.isInteger(position) && position >= 1 && position <= 9) {
        event.preventDefault();
        event.stopPropagation();
        openByPosition(position);
        return;
      }
      if (!isMac && event.shiftKey && event.key.toLowerCase() === 'w') {
        event.preventDefault();
        event.stopPropagation();
        closeFocusedPane();
      }
    };
    window.addEventListener('keydown', onKeyDown, { capture: true });
    const unlisten = isTauri() ? listen(CLOSE_PANE_EVENT, closeFocusedPane) : null;
    return () => {
      window.removeEventListener('keydown', onKeyDown, { capture: true });
      void unlisten?.then((stop) => stop());
    };
  }, []);
}

function openByPosition(position: number): void {
  const state = useAppStore.getState();
  if (state.connection !== 'ready') return;
  const session = sidebarOrder(state.sessions, state.projects)[position - 1];
  if (session === undefined) return;
  if (state.sidebarTab !== 'active') state.setSidebarTab('active');
  state.select(session.id);
}

function closeFocusedPane(): void {
  const { selectedId, closePane } = useAppStore.getState();
  if (selectedId !== null) closePane(selectedId);
}
