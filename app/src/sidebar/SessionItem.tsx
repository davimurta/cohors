import type { SessionMeta } from '@cohors/protocol';
import { StatusDot } from '../components/StatusDot.tsx';
import { AGENT_LABEL, displayPath } from '../session/format.ts';
import { STATUS_LABEL } from '../session/status.ts';
import styles from './SessionItem.module.css';

interface SessionItemProps {
  session: SessionMeta;
  selected: boolean;
  home: string | null;
  onSelect: (id: string) => void;
}

export function SessionItem({ session, selected, home, onSelect }: SessionItemProps) {
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
        {STATUS_LABEL[session.status]}
      </span>
    </button>
  );
}
