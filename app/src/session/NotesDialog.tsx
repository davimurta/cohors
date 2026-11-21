import { useId, useState, type FormEvent } from 'react';
import type { SessionMeta } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import { Dialog } from '../components/Dialog.tsx';
import { useAppStore } from '../state/store.ts';
import styles from './NotesDialog.module.css';

export function NotesDialog() {
  const session = useAppStore((state) =>
    state.sessions.find((candidate) => candidate.id === state.notesId),
  );
  const close = useAppStore((state) => state.closeNotes);

  return (
    <Dialog open={session !== undefined} title="Notas" onClose={close}>
      {session !== undefined && <NotesForm session={session} onCancel={close} />}
    </Dialog>
  );
}

function NotesForm({ session, onCancel }: { session: SessionMeta; onCancel: () => void }) {
  const saveNotes = useAppStore((state) => state.saveNotes);
  const [notes, setNotes] = useState(session.notes);
  const id = useId();

  const submit = (event: FormEvent) => {
    event.preventDefault();
    saveNotes(session.id, notes);
  };

  return (
    <form className={styles.form} onSubmit={submit}>
      <textarea
        id={id}
        className={styles.textarea}
        value={notes}
        aria-label={`Notas de ${session.name}`}
        placeholder="O que lembrar sobre esta sessão"
        data-autofocus=""
        onChange={(event) => setNotes(event.target.value)}
      />
      <div className={styles.footer}>
        <Button onClick={onCancel}>Cancelar</Button>
        <Button type="submit" variant="primary">
          Salvar
        </Button>
      </div>
    </form>
  );
}
