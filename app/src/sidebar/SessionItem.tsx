import type { SessionMeta } from '@cohors/protocol';
import { StatusDot } from '../components/StatusDot.tsx';
import { AGENT_LABEL, displayPath, formatDuration } from '../session/format.ts';
import { STATUS_LABEL } from '../session/status.ts';
import styles from './SessionItem.module.css';

interface SessionItemProps {
  session: SessionMeta;
  selected: boolean;
  home: string | null;
  now: number;
  onSelect: (id: string) => void;
}

export function SessionItem({ session, selected, home, now, onSelect }: SessionItemProps) {
  return (
    <button
      type="button"
      className={styles.item}
      aria-current={selected}
      onClick={() => onSelect(session.id)}
    >
      <StatusDot status={session.status} />
      <span className={styles.text}>
        <span className={styles.name}>{session.name}</span>
        <span className={styles.detail}>
          {AGENT_LABEL[session.agent]} · {displayPath(session.cwd, home)}
        </span>
      </span>
      <span className={`${styles.status} ${styles[session.status]}`}>
        {statusText(session, now)}
      </span>
    </button>
  );
}

/** "Ociosa 4 min" for idle sessions, the plain label otherwise. */
function statusText(session: SessionMeta, now: number): string {
  const label = STATUS_LABEL[session.status];
  if (session.status !== 'idle' || session.idleSince === null) return label;
  const duration = formatDuration(session.idleSince, now);
  return duration === null ? label : `${label} ${duration}`;
}
