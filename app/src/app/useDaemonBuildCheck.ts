import { useEffect } from 'react';
import { bundledBuildId, canManageDaemon } from '../daemon/lifecycle.ts';
import { useAppStore } from '../state/store.ts';

/**
 * Compares the running daemon with the one bundled in this app. A daemon
 * keeps running across app updates, so after one it may be an older build
 * that speaks the same protocol but lacks newer features.
 */
export function useDaemonBuildCheck(): void {
  const runningBuild = useAppStore((state) => state.daemonBuildId);
  const setDaemonOutdated = useAppStore((state) => state.setDaemonOutdated);

  useEffect(() => {
    if (!canManageDaemon || runningBuild === null) return;
    let current = true;
    bundledBuildId()
      .then((bundled) => {
        if (current) setDaemonOutdated(bundled !== runningBuild);
      })
      .catch((error: unknown) => console.warn('could not read the bundled daemon build', error));
    return () => {
      current = false;
    };
  }, [runningBuild, setDaemonOutdated]);
}
