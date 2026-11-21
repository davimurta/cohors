import { CircleCheck, FileText, Folder } from 'lucide-react';
import type { SessionMeta } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import { StatusPill } from '../components/StatusPill.tsx';
import { TerminalPane } from '../terminal/TerminalPane.tsx';
import { AGENT_LABEL, displayPath, formatElapsed } from './format.ts';
import { useHomeDir } from './useHomeDir.ts';
import { useNow } from './useNow.ts';
import styles from './SessionView.module.css';

export function SessionView({ session }: { session: SessionMeta }) {
  const home = useHomeDir();
  const now = useNow();

  return (
    <main className={styles.view}>
      <header className={styles.header}>
        <div className={styles.heading}>
          <div className={styles.titleRow}>
            <h1 className={styles.title}>{session.name}</h1>
            <StatusPill status={session.status} />
          </div>
          <div className={styles.meta}>
            <span className={styles.folder}>
              <Folder size={14} aria-hidden="true" />
              {displayPath(session.cwd, home)}
            </span>
            <span>{AGENT_LABEL[session.agent]}</span>
            <span>Iniciada {formatElapsed(session.createdAt, now)}</span>
          </div>
        </div>
        <div className={styles.actions}>
          <Button icon={<FileText size={16} aria-hidden="true" />} disabled>
            Notas
          </Button>
          <Button icon={<CircleCheck size={16} aria-hidden="true" />} disabled>
            Finalizar
          </Button>
        </div>
      </header>
      <TerminalPane sessionId={session.id} label={`Terminal da sessão ${session.name}`} />
    </main>
  );
}
