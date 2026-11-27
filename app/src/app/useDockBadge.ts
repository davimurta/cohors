import { useEffect } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { sidebarOrder } from '../projects/groupSessions.ts';
import { useAppStore } from '../state/store.ts';
import { waitingCount } from './attention.ts';

/**
 * The Dock (macOS) and taskbar (Linux) badge shows how many active sessions
 * wait for the user. Windows has no numeric badge, so Tauri ignores it there.
 */
export function useDockBadge(): void {
  useEffect(() => {
    if (!isTauri()) return;
    let shown = -1;
    const update = () => {
      const { sessions, projects } = useAppStore.getState();
      const count = waitingCount(sidebarOrder(sessions, projects));
      if (count === shown) return;
      shown = count;
      void getCurrentWindow()
        .setBadgeCount(count > 0 ? count : undefined)
        .catch(() => {
          // A badge is a convenience; the sidebar still shows who waits.
        });
    };
    update();
    const unsubscribe = useAppStore.subscribe((state, previous) => {
      if (state.sessions !== previous.sessions || state.projects !== previous.projects) update();
    });
    return () => {
      unsubscribe();
      void getCurrentWindow()
        .setBadgeCount(undefined)
        .catch(() => {});
    };
  }, []);
}
