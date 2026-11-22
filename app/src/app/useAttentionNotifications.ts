import { useEffect } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';
import { useAppStore } from '../state/store.ts';
import { ATTENTION_MESSAGE, newlyWaiting } from './attention.ts';

/** A system notification when a Claude session starts waiting for the user. */
export function useAttentionNotifications(): void {
  useEffect(() => {
    if (!isTauri()) return;
    return useAppStore.subscribe((state, previous) => {
      if (state.sessions === previous.sessions) return;
      const visible = (id: string) => document.hasFocus() && state.panes.includes(id);
      for (const session of newlyWaiting(previous.sessions, state.sessions, visible)) {
        if (session.attention === null) continue;
        void notify(session.name, ATTENTION_MESSAGE[session.attention]);
      }
    });
  }, []);
}

async function notify(title: string, body: string): Promise<void> {
  try {
    const granted = (await isPermissionGranted()) || (await requestPermission()) === 'granted';
    if (granted) sendNotification({ title, body });
  } catch {
    // Notifications are a convenience; the sidebar still shows the session.
  }
}
