import { Pencil } from 'lucide-react';
import type { SessionMeta } from '@cohors/protocol';
import { useAppStore } from '../state/store.ts';
import styles from './RenameButton.module.css';

/** Small pencil next to a session title. */
export function RenameButton({ session }: { session: SessionMeta }) {
  const openRename = useAppStore((state) => state.openRename);
  return (
    <button
      type="button"
      className={styles.button}
      aria-label={`Renomear sessão ${session.name}`}
      title="Renomear"
      onClick={() => openRename(session.id)}
    >
      <Pencil size={14} aria-hidden="true" />
    </button>
  );
}
