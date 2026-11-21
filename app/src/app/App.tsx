import { useEffect } from 'react';
import { NewSessionDialog } from '../new-session/NewSessionDialog.tsx';
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
      {selected !== undefined && <SessionView key={selected.id} session={selected} />}
      <NewSessionDialog />
    </div>
  );
}
