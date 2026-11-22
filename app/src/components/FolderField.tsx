import { Folder } from 'lucide-react';
import { open as openFolderPicker } from '@tauri-apps/plugin-dialog';
import { Button } from './Button.tsx';
import styles from './Field.module.css';

interface FolderFieldProps {
  id: string;
  label: string;
  value: string;
  /** Where the picker opens when no folder is chosen yet. */
  startIn?: string;
  onChange: (folder: string) => void;
}

/** Read-only path plus a button that opens the system folder picker. */
export function FolderField({ id, label, value, startIn, onChange }: FolderFieldProps) {
  const choose = async () => {
    const defaultPath = value === '' ? startIn : value;
    const folder = await openFolderPicker({
      directory: true,
      multiple: false,
      ...(defaultPath === undefined ? {} : { defaultPath }),
    }).catch(() => null);
    if (typeof folder === 'string') onChange(folder);
  };

  return (
    <div className={styles.field}>
      <label htmlFor={id} className={styles.label}>
        {label}
      </label>
      <div className={styles.row}>
        <input
          id={id}
          className={`${styles.control} ${styles.path}`}
          value={value}
          placeholder="Nenhuma pasta escolhida"
          readOnly
        />
        <Button icon={<Folder size={16} aria-hidden="true" />} onClick={() => void choose()}>
          Escolher
        </Button>
      </div>
    </div>
  );
}
