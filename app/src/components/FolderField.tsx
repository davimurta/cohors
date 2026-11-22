import { useId } from 'react';
import { Folder, X } from 'lucide-react';
import { open as openFolderPicker } from '@tauri-apps/plugin-dialog';
import { Button } from './Button.tsx';
import styles from './Field.module.css';

interface FolderFieldProps {
  id: string;
  label: string;
  value: string;
  /** Where the picker opens when no folder is chosen yet. */
  startIn?: string;
  /** Shown while no folder is chosen. */
  placeholder?: string;
  hint?: string;
  onChange: (folder: string) => void;
  /** Makes the folder optional: a button clears the choice. */
  onClear?: () => void;
}

/** Read-only path plus a button that opens the system folder picker. */
export function FolderField({
  id,
  label,
  value,
  startIn,
  placeholder = 'Nenhuma pasta escolhida',
  hint,
  onChange,
  onClear,
}: FolderFieldProps) {
  const hintId = useId();
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
          placeholder={placeholder}
          readOnly
          {...(hint === undefined ? {} : { 'aria-describedby': hintId })}
        />
        {onClear !== undefined && value !== '' && (
          <Button
            variant="ghost"
            icon={<X size={16} aria-hidden="true" />}
            aria-label={`Limpar ${label}`}
            onClick={onClear}
          />
        )}
        <Button icon={<Folder size={16} aria-hidden="true" />} onClick={() => void choose()}>
          Escolher
        </Button>
      </div>
      {hint !== undefined && (
        <span id={hintId} className={styles.hint}>
          {hint}
        </span>
      )}
    </div>
  );
}
