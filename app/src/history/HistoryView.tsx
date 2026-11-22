import { useEffect, useState } from 'react';
import { FileText, RotateCcw, Terminal, Trash2 } from 'lucide-react';
import type { SessionMeta } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import { ConfirmDialog } from '../components/ConfirmDialog.tsx';
import { StatusPill } from '../components/StatusPill.tsx';
import {
  AGENT_LABEL,
  displayPath,
  formatSpan,
  formatTime,
  formatTimeRange,
} from '../session/format.ts';
import { useHomeDir } from '../session/useHomeDir.ts';
import { useNow } from '../session/useNow.ts';
import { RenameButton } from '../rename/RenameButton.tsx';
import { useAppStore } from '../state/store.ts';
import { CopyButton } from './CopyButton.tsx';
import { SummaryText } from './SummaryText.tsx';
import styles from './HistoryView.module.css';

/** A finished session: summary, details and notes. */
export function HistoryView({ session }: { session: SessionMeta }) {
  const home = useHomeDir();
  const now = useNow();
  const openTranscript = useAppStore((state) => state.openTranscript);
  const deleteSession = useAppStore((state) => state.deleteSession);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const ended = session.endedAt ?? session.createdAt;

  return (
    <main className={styles.view}>
      <header className={styles.header}>
        <div className={styles.heading}>
          <div className={styles.titleRow}>
            <h1 className={styles.title}>{session.name}</h1>
            <RenameButton session={session} />
            <StatusPill status={session.status} />
          </div>
          <span className={styles.meta}>
            {displayPath(session.cwd, home)} · {AGENT_LABEL[session.agent]} ·{' '}
            {formatTimeRange(session.createdAt, ended, now)}
          </span>
        </div>
        <div className={styles.actions}>
          <Button
            icon={<Terminal size={16} aria-hidden="true" />}
            onClick={() => openTranscript(session.id)}
          >
            Ver transcript
          </Button>
          <Button
            variant="ghost"
            className={styles.danger}
            icon={<Trash2 size={16} aria-hidden="true" />}
            onClick={() => setConfirmingDelete(true)}
          >
            Excluir
          </Button>
        </div>
      </header>

      <div className={styles.body}>
        <SummaryCard session={session} />
        <div className={styles.side}>
          <section className={styles.card} aria-labelledby="details-title">
            <h2 id="details-title" className={styles.cardTitle}>
              Detalhes
            </h2>
            <dl className={styles.details}>
              <dt>Pasta</dt>
              <dd>{displayPath(session.cwd, home)}</dd>
              <dt>Agente</dt>
              <dd>{AGENT_LABEL[session.agent]}</dd>
              <dt>Início</dt>
              <dd>{formatTime(session.createdAt)}</dd>
              <dt>Fim</dt>
              <dd>{formatTime(ended)}</dd>
              <dt>Duração</dt>
              <dd>{formatSpan(session.createdAt, ended)}</dd>
            </dl>
          </section>
          <NotesCard session={session} />
        </div>
      </div>

      <ConfirmDialog
        open={confirmingDelete}
        title="Excluir sessão finalizada"
        confirmLabel="Excluir"
        onConfirm={() => deleteSession(session.id)}
        onClose={() => setConfirmingDelete(false)}
      >
        O resumo, as notas e o transcript de {session.name} serão apagados. Isso não pode ser
        desfeito.
      </ConfirmDialog>
    </main>
  );
}

function SummaryCard({ session }: { session: SessionMeta }) {
  const summary = useAppStore((state) => state.summaries[session.id]);
  const loadSummary = useAppStore((state) => state.loadSummary);
  const retrySummary = useAppStore((state) => state.retrySummary);
  const skipSummary = useAppStore((state) => state.skipSummary);
  const done = session.summaryStatus === 'done';

  useEffect(() => {
    if (done) loadSummary(session.id);
  }, [done, loadSummary, session.id]);

  return (
    <section
      className={`${styles.card} ${styles.summaryCard}`}
      aria-labelledby="summary-title"
    >
      <div className={styles.cardHeader}>
        <h2 id="summary-title" className={styles.summaryTitle}>
          Resumo
        </h2>
        {done && summary !== undefined && summary !== null && (
          <CopyButton text={summary} label="Copiar resumo" />
        )}
      </div>
      {session.summaryStatus === 'failed' ? (
        <div className={styles.failure}>
          <p className={styles.muted}>
            Não foi possível gerar o resumo. Você pode tentar de novo ou deixar a sessão sem
            resumo.
          </p>
          <div className={styles.failureActions}>
            <Button
              variant="primary"
              icon={<RotateCcw size={16} aria-hidden="true" />}
              onClick={() => retrySummary(session.id)}
            >
              Tentar de novo
            </Button>
            <Button onClick={() => skipSummary(session.id)}>Finalizar sem resumo</Button>
          </div>
        </div>
      ) : done ? (
        summary === undefined || summary === null ? (
          <p className={styles.muted}>Carregando resumo…</p>
        ) : (
          <SummaryText markdown={summary} />
        )
      ) : session.agent === 'shell' ? (
        <p className={styles.muted}>Sessões de shell não têm resumo.</p>
      ) : (
        <div className={styles.failure}>
          <p className={styles.muted}>Esta sessão foi finalizada sem resumo.</p>
          <Button onClick={() => retrySummary(session.id)}>Gerar resumo</Button>
        </div>
      )}
    </section>
  );
}

function NotesCard({ session }: { session: SessionMeta }) {
  const openNotes = useAppStore((state) => state.openNotes);
  return (
    <section className={styles.card} aria-labelledby="notes-title">
      <h2 id="notes-title" className={styles.cardTitle}>
        Notas
      </h2>
      <p className={styles.notes}>{session.notes === '' ? 'Sem notas.' : session.notes}</p>
      <Button
        variant="ghost"
        size="small"
        className={styles.notesButton}
        icon={<FileText size={14} aria-hidden="true" />}
        onClick={() => openNotes(session.id)}
      >
        Editar notas
      </Button>
    </section>
  );
}
