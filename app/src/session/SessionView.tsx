import { FileText, Folder, GitBranch } from 'lucide-react';
import type { SessionMeta } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import { StatusPill } from '../components/StatusPill.tsx';
import { TerminalPane } from '../terminal/TerminalPane.tsx';
import { SplitMenu } from '../split/SplitMenu.tsx';
import { RenameButton } from '../rename/RenameButton.tsx';
import { useAppStore } from '../state/store.ts';
import { EndedPanel } from './EndedPanel.tsx';
import { FinishButton } from './FinishButton.tsx';
import { isLive } from './status.ts';
import { AGENT_LABEL, displayPath, formatElapsed, formatLastActivity } from './format.ts';
import { useHomeDir } from './useHomeDir.ts';
import { useNow } from './useNow.ts';
import styles from './SessionView.module.css';

export function SessionView({ session }: { session: SessionMeta }) {
  const home = useHomeDir();
  const now = useNow();
  const live = isLive(session.status);
  const openNotes = useAppStore((state) => state.openNotes);

  return (
    <main className={styles.view}>
      <header className={styles.header}>
        <div className={styles.heading}>
          <div className={styles.titleRow}>
            <h1 className={styles.title}>{session.name}</h1>
            <RenameButton session={session} />
            <StatusPill status={session.status} />
          </div>
          {live ? (
            <div className={styles.meta}>
              <span className={styles.folder}>
                <Folder size={14} aria-hidden="true" />
                {displayPath(session.cwd, home)}
              </span>
              {session.worktree !== null && (
                <span className={styles.folder} title="Worktree isolada desta sessão">
                  <GitBranch size={14} aria-hidden="true" />
                  {session.worktree.branch}
                </span>
              )}
              <span>{AGENT_LABEL[session.agent]}</span>
              <span>Iniciada {formatElapsed(session.createdAt, now)}</span>
            </div>
          ) : (
            <span className={styles.meta}>
              {[
                displayPath(session.cwd, home),
                AGENT_LABEL[session.agent],
                session.endedAt === null
                  ? null
                  : `Última atividade ${formatLastActivity(session.endedAt, now)}`,
              ]
                .filter(Boolean)
                .join(' · ')}
            </span>
          )}
        </div>
        {live && (
          <div className={styles.actions}>
            <SplitMenu />
            <Button
              icon={<FileText size={16} aria-hidden="true" />}
              onClick={() => openNotes(session.id)}
            >
              Notas
            </Button>
            <FinishButton session={session} label="Finalizar" />
          </div>
        )}
      </header>
      {live ? (
        <TerminalPane sessionId={session.id} label={`Terminal da sessão ${session.name}`} />
      ) : (
        <EndedPanel session={session} />
      )}
    </main>
  );
}
