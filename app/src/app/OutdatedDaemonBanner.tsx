import { useState } from 'react';
import { Power, RefreshCw } from 'lucide-react';
import { Button } from '../components/Button.tsx';
import { useAppStore } from '../state/store.ts';
import styles from './OutdatedDaemonBanner.module.css';

/** Offers to restart a daemon left over from an older build of the app. */
export function OutdatedDaemonBanner() {
  const outdated = useAppStore((state) => state.daemonOutdated);
  const buildId = useAppStore((state) => state.daemonBuildId);
  const restartDaemon = useAppStore((state) => state.restartDaemon);
  // "Agora não" hides it for this daemon; a later outdated one asks again.
  const [dismissedBuild, setDismissedBuild] = useState<string | null>(null);

  if (!outdated || buildId === dismissedBuild) return null;
  return (
    <div role="status" className={styles.banner}>
      <RefreshCw size={16} aria-hidden="true" className={styles.icon} />
      <p className={styles.text}>
        <strong>Há uma versão nova do daemon.</strong> O que está rodando é de uma versão anterior
        do Cohors, e algumas novidades só funcionam depois de reiniciá-lo. Reiniciar interrompe as
        sessões abertas; você pode retomá-las em seguida.
      </p>
      <div className={styles.actions}>
        <Button size="small" onClick={() => setDismissedBuild(buildId)}>
          Agora não
        </Button>
        <Button
          size="small"
          variant="primary"
          icon={<Power size={14} aria-hidden="true" />}
          onClick={restartDaemon}
        >
          Reiniciar daemon
        </Button>
      </div>
    </div>
  );
}
