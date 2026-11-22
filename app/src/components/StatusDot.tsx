import type { SessionStatus } from '@cohors/protocol';
import styles from './StatusDot.module.css';

interface StatusDotProps {
  status: SessionStatus;
  size?: 'small' | 'default';
}

/** Decorative: always pair it with a visible status label. */
export function StatusDot({ status, size = 'default' }: StatusDotProps) {
  const classes = [styles.dot, styles[status], size === 'small' ? styles.small : '']
    .filter(Boolean)
    .join(' ');
  return <span aria-hidden="true" className={classes} />;
}
