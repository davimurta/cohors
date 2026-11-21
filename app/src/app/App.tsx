import { useEffect } from 'react';
import { SessionView } from '../session/SessionView.tsx';
import { Sidebar } from '../sidebar/Sidebar.tsx';
import { startDaemonSync, useAppStore } from '../state/store.ts';
import { startTerminalRouting } from '../terminal/registry.ts';
import styles from './App.module.css';

export function App() {
  const selected = useAppStore((state) =>
    state.sessions.find((session) => session.id === state.selectedId),
  );

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
    </div>
  );
}
