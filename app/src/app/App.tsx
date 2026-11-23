import { useEffect } from 'react';
import { DaemonErrorScreen } from '../daemon-error/DaemonErrorScreen.tsx';
import { NewSessionDialog } from '../new-session/NewSessionDialog.tsx';
import { ContextDialog } from '../projects/ContextDialog.tsx';
import { ProjectDialog } from '../projects/ProjectDialog.tsx';
import { NotesDialog } from '../session/NotesDialog.tsx';
import { RenameDialog } from '../rename/RenameDialog.tsx';
import { Sidebar } from '../sidebar/Sidebar.tsx';
import { startDaemonSync, useAppStore } from '../state/store.ts';
import { startTerminalRouting } from '../terminal/registry.ts';
import styles from './App.module.css';
import { MainView } from './MainView.tsx';
import { useAttentionNotifications } from './useAttentionNotifications.ts';
import { useNewSessionShortcut } from './useNewSessionShortcut.ts';
import { useSessionShortcuts } from './useSessionShortcuts.ts';
import { useDaemonBuildCheck } from './useDaemonBuildCheck.ts';
import { OutdatedDaemonBanner } from './OutdatedDaemonBanner.tsx';

export function App() {
  const connection = useAppStore((state) => state.connection);
  const daemonDown = connection === 'unavailable' || connection === 'incompatible';

  useNewSessionShortcut();
  useAttentionNotifications();
  useSessionShortcuts();
  useDaemonBuildCheck();

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
        <div className={styles.content}>
          <OutdatedDaemonBanner />
          <MainView />
        </div>
      )}
      <NewSessionDialog />
      <ProjectDialog />
      <ContextDialog />
      <NotesDialog />
      <RenameDialog />
    </div>
  );
}
