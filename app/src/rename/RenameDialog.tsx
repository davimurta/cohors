import { useId, useState, type FormEvent } from 'react';
import type { SessionMeta } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import { Dialog } from '../components/Dialog.tsx';
import field from '../components/Field.module.css';
import { useAppStore } from '../state/store.ts';
import styles from './RenameDialog.module.css';

export function RenameDialog() {
  const session = useAppStore((state) =>
    state.sessions.find((candidate) => candidate.id === state.renameId),
  );
  const close = useAppStore((state) => state.closeRename);

  return (
    <Dialog open={session !== undefined} title="Renomear sessão" onClose={close}>
      {session !== undefined && <RenameForm session={session} onCancel={close} />}
    </Dialog>
  );
}

function RenameForm({ session, onCancel }: { session: SessionMeta; onCancel: () => void }) {
  const renameSession = useAppStore((state) => state.renameSession);
  const [name, setName] = useState(session.name);
  const id = useId();
  const valid = name.trim() !== '';

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (valid) renameSession(session.id, name.trim());
  };

  return (
    <form className={styles.form} onSubmit={submit}>
      <div className={field.field}>
        <label htmlFor={id} className={field.label}>
          Nome
        </label>
        <input
          id={id}
          className={field.control}
          value={name}
          data-autofocus=""
          onFocus={(event) => event.target.select()}
          onChange={(event) => setName(event.target.value)}
        />
      </div>
      <div className={styles.footer}>
        <Button onClick={onCancel}>Cancelar</Button>
        <Button type="submit" variant="primary" disabled={!valid}>
          Renomear
        </Button>
      </div>
    </form>
  );
}
