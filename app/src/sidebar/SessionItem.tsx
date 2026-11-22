import type { MouseEvent } from 'react';
import type { SessionMeta } from '@cohors/protocol';
import { StatusDot } from '../components/StatusDot.tsx';
import { AGENT_LABEL, displayPath, formatDuration, formatSpan } from '../session/format.ts';
import { isMac } from '../platform.ts';
import { STATUS_LABEL, isLive } from '../session/status.ts';
import { useAppStore } from '../state/store.ts';
import styles from './SessionItem.module.css';

interface SessionItemProps {
  session: SessionMeta;
  selected: boolean;
  home: string | null;
  now: number;
  onSelect: (id: string) => void;
}

export function SessionItem({ session, selected, home, now, onSelect }: SessionItemProps) {
  const toggleSplit = useAppStore((state) => state.toggleSplit);
  const openRename = useAppStore((state) => state.openRename);
  const shown = useAppStore(
    (state) => state.panes.length > 1 && state.panes.includes(session.id),
  );

  // Cmd+click (Ctrl+click off macOS) shows a live session next to the others.
  const onClick = (event: MouseEvent) => {
    const split = isMac ? event.metaKey : event.ctrlKey;
    if (split && isLive(session.status)) toggleSplit(session.id);
    else onSelect(session.id);
  };

  return (
    <button
      type="button"
      className={styles.item}
      aria-current={selected}
      data-shown={shown}
      title="Clique duplo para renomear"
      onClick={onClick}
      onDoubleClick={() => openRename(session.id)}
    >
      <StatusDot status={session.status} />
      <span className={styles.text}>
        <span className={styles.name}>{session.name}</span>
        <span className={styles.detail}>
          {AGENT_LABEL[session.agent]} · {displayPath(session.cwd, home)}
          {session.status === 'finished' && session.endedAt !== null
            ? ` · ${formatSpan(session.createdAt, session.endedAt)}`
            : ''}
        </span>
      </span>
      <span className={`${styles.status} ${styles[session.status]}`}>
        {statusText(session, now)}
      </span>
    </button>
  );
}

/** "Ociosa 4 min" for idle sessions, the plain label otherwise. Finished
 * sessions are all alike in the history, so only a summary in progress shows. */
function statusText(session: SessionMeta, now: number): string {
  if (session.status === 'finished') {
    return session.summaryStatus === 'pending' ? 'Gerando resumo' : '';
  }
  const label = STATUS_LABEL[session.status];
  if (session.status !== 'idle' || session.idleSince === null) return label;
  const duration = formatDuration(session.idleSince, now);
  return duration === null ? label : `${label} ${duration}`;
}
