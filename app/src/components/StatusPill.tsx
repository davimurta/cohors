import type { SessionStatus } from '@cohors/protocol';
import { STATUS_LABEL } from '../session/status.ts';
import { StatusDot } from './StatusDot.tsx';
import styles from './StatusPill.module.css';

export function StatusPill({ status }: { status: SessionStatus }) {
  return (
    <span className={`${styles.pill} ${styles[status]}`}>
      <StatusDot status={status} size="small" />
      {STATUS_LABEL[status]}
    </span>
  );
}
