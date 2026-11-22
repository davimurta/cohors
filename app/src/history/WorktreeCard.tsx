import { useEffect, useState } from 'react';
import { GitBranch, Trash2 } from 'lucide-react';
import type { SessionMeta, Worktree } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import { ConfirmDialog } from '../components/ConfirmDialog.tsx';
import { displayPath } from '../session/format.ts';
import { useHomeDir } from '../session/useHomeDir.ts';
import type { WorktreeView } from '../state/sessions.ts';
import { useAppStore } from '../state/store.ts';
import styles from './HistoryView.module.css';

/** The session's worktree, with a way to remove it once the work is saved. */
export function WorktreeCard({ session, worktree }: { session: SessionMeta; worktree: Worktree }) {
  const view = useAppStore((state) => state.worktrees[session.id]);
  const loadWorktree = useAppStore((state) => state.loadWorktree);
  const removeWorktree = useAppStore((state) => state.removeWorktree);
  const home = useHomeDir();
  const [confirming, setConfirming] = useState(false);

  useEffect(() => loadWorktree(session.id), [loadWorktree, session.id]);

  const status = view?.status ?? null;
  const canRemove = status !== null && (status.changes === 0 || !status.exists);

  return (
    <section className={styles.card} aria-labelledby="worktree-title">
      <h2 id="worktree-title" className={styles.cardTitle}>
        Worktree
      </h2>
      <dl className={styles.details}>
        <dt>Branch</dt>
        <dd className={styles.branch}>
          <GitBranch size={13} aria-hidden="true" />
          {worktree.branch}
        </dd>
        <dt>Pasta</dt>
        <dd>{displayPath(worktree.path, home)}</dd>
      </dl>
      <p className={styles.worktreeState} role="status">
        {describe(view, worktree)}
      </p>
      {view?.error !== null && view?.error !== undefined && (
        <p role="alert" className={styles.worktreeError}>
          {view.error}
        </p>
      )}
      {status?.exists === true && (
        <Button
          size="small"
          icon={<Trash2 size={14} aria-hidden="true" />}
          disabled={!canRemove || view?.removing === true}
          onClick={() => setConfirming(true)}
        >
          {view?.removing === true ? 'Removendo…' : 'Remover worktree'}
        </Button>
      )}
      <ConfirmDialog
        open={confirming}
        title="Remover worktree"
        confirmLabel="Remover"
        onConfirm={() => removeWorktree(session.id)}
        onClose={() => setConfirming(false)}
      >
        A pasta {displayPath(worktree.path, home)} será apagada.
        {status !== null && status.commits > 0
          ? ` A branch ${worktree.branch} fica, com os commits dela.`
          : ` A branch ${worktree.branch} também, porque não tem commits novos.`}
      </ConfirmDialog>
    </section>
  );
}

function describe(view: WorktreeView | undefined, worktree: Worktree): string {
  const status = view?.status ?? null;
  if (status === null) return 'Verificando…';
  if (!status.exists) {
    if (view?.branchDeleted === true) return `Removida, junto com a branch ${worktree.branch}.`;
    if (view?.branchDeleted === false) {
      return `Removida. A branch ${worktree.branch} continua no repositório.`;
    }
    return 'A pasta não existe mais.';
  }
  const parts = [
    status.changes === 0
      ? 'Sem mudanças pendentes'
      : `${status.changes} ${status.changes === 1 ? 'arquivo alterado' : 'arquivos alterados'} sem commit`,
    status.commits === 0
      ? 'nenhum commit novo'
      : `${status.commits} ${status.commits === 1 ? 'commit novo' : 'commits novos'} na branch`,
  ];
  const sentence = `${parts.join(', ')}.`;
  return status.changes === 0
    ? sentence
    : `${sentence} Faça commit ou descarte as mudanças para poder remover.`;
}
