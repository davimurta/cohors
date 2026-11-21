import { useEffect } from 'react';
import { DaemonErrorScreen } from '../daemon-error/DaemonErrorScreen.tsx';
import { EmptyState } from '../empty/EmptyState.tsx';
import { NewSessionDialog } from '../new-session/NewSessionDialog.tsx';
import { ProjectDialog } from '../projects/ProjectDialog.tsx';
import { SessionView } from '../session/SessionView.tsx';
import { Sidebar } from '../sidebar/Sidebar.tsx';
import { startDaemonSync, useAppStore } from '../state/store.ts';
import { startTerminalRouting } from '../terminal/registry.ts';
import styles from './App.module.css';
import { useNewSessionShortcut } from './useNewSessionShortcut.ts';

export function App() {
  const selected = useAppStore((state) =>
    state.sessions.find((session) => session.id === state.selectedId),
  );
  const hasSessions = useAppStore((state) => state.sessions.length > 0);
  const connection = useAppStore((state) => state.connection);
  const daemonDown = connection === 'unavailable' || connection === 'incompatible';

  useNewSessionShortcut();

  useEffect(() => {
    const stopRouting = startTerminalRouting();
    const stopSync = startDaemonSync();
    return () => {
      stopSync();
      stopRouting();
    };
  }, []);

  return (
    <div className={styles.app}>
      <Sidebar />
      {daemonDown ? (
        <DaemonErrorScreen incompatible={connection === 'incompatible'} />
      ) : (
        <>
          {!hasSessions && <EmptyState />}
          {selected !== undefined && <SessionView key={selected.id} session={selected} />}
        </>
      )}
      <NewSessionDialog />
      <ProjectDialog />
    </div>
  );
}
