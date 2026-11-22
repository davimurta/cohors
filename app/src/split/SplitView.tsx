import { FileText, X } from 'lucide-react';
import type { SessionMeta } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import { StatusDot } from '../components/StatusDot.tsx';
import { isMac } from '../platform.ts';
import { FinishButton } from '../session/FinishButton.tsx';
import { STATUS_LABEL } from '../session/status.ts';
import { useAppStore } from '../state/store.ts';
import { TerminalPane } from '../terminal/TerminalPane.tsx';
import { SplitMenu } from './SplitMenu.tsx';
import styles from './SplitView.module.css';

const SPLIT_CLICK = isMac ? 'Cmd+clique' : 'Ctrl+clique';

/** Several live sessions side by side; the focused one is the selection. */
export function SplitView({ sessions }: { sessions: SessionMeta[] }) {
  const selectedId = useAppStore((state) => state.selectedId);

  return (
    <main className={styles.view}>
      <div className={styles.toolbar}>
        <span className={styles.hint}>
          {sessions.length} terminais lado a lado. {SPLIT_CLICK} numa sessão da lista adiciona ou
          tira um terminal.
        </span>
        <SplitMenu />
      </div>
      <div className={styles.grid}>
        {sessions.map((session) => (
          <Pane key={session.id} session={session} focused={session.id === selectedId} />
        ))}
      </div>
    </main>
  );
}

function Pane({ session, focused }: { session: SessionMeta; focused: boolean }) {
  const select = useAppStore((state) => state.select);
  const closePane = useAppStore((state) => state.closePane);
  const openNotes = useAppStore((state) => state.openNotes);

  return (
    <section
      className={styles.pane}
      data-focused={focused}
      aria-label={session.name}
      // Clicking or tabbing into a pane makes it the focused session.
      onFocusCapture={() => {
        if (!focused) select(session.id);
      }}
    >
      <header className={styles.paneHeader}>
        <StatusDot status={session.status} />
        <span className={styles.paneName}>{session.name}</span>
        <span className={styles.paneStatus}>{STATUS_LABEL[session.status]}</span>
        <Button
          variant="ghost"
          size="small"
          icon={<FileText size={14} aria-hidden="true" />}
          aria-label={`Notas de ${session.name}`}
          title="Notas"
          onClick={() => openNotes(session.id)}
        />
        <FinishButton session={session} label="Finalizar" compact />
        <Button
          variant="ghost"
          size="small"
          icon={<X size={14} aria-hidden="true" />}
          aria-label={`Fechar painel de ${session.name}`}
          title="Fechar painel"
          onClick={() => closePane(session.id)}
        />
      </header>
      <div className={styles.terminal}>
        <TerminalPane
          sessionId={session.id}
          label={`Terminal da sessão ${session.name}`}
          focus={focused}
        />
      </div>
    </section>
  );
}
