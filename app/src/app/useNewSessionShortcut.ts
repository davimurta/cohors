import { useEffect } from 'react';
import { isMac } from '../platform.ts';
import { useAppStore } from '../state/store.ts';

/**
 * Cmd+N on macOS, Ctrl+N elsewhere. Listens in the capture phase so the
 * shortcut wins over the focused terminal.
 */
export function useNewSessionShortcut(): void {
  const openNewSession = useAppStore((state) => state.openNewSession);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const modifier = isMac ? event.metaKey : event.ctrlKey;
      if (!modifier || event.shiftKey || event.altKey || event.key.toLowerCase() !== 'n') return;
      event.preventDefault();
      event.stopPropagation();
      if (useAppStore.getState().connection === 'ready') openNewSession();
    };
    window.addEventListener('keydown', onKeyDown, { capture: true });
    return () => window.removeEventListener('keydown', onKeyDown, { capture: true });
  }, [openNewSession]);
}
