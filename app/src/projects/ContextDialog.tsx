import { useEffect, useId, useRef, useState, type FormEvent } from 'react';
import { FolderOpen } from 'lucide-react';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { Button } from '../components/Button.tsx';
import { Dialog } from '../components/Dialog.tsx';
import field from '../components/Field.module.css';
import { useAppStore } from '../state/store.ts';
import type { ContextEditor } from '../state/sessions.ts';
import styles from './ContextDialog.module.css';

/** Rules and plan of a project, which its Claude sessions read on start and resume. */
export function ContextDialog() {
  const editor = useAppStore((state) => state.contextEditor);
  const project = useAppStore((state) =>
    state.projects.find((candidate) => candidate.id === state.contextEditor?.projectId),
  );
  const close = useAppStore((state) => state.closeContext);

  return (
    <Dialog
      open={editor !== null}
      title={project === undefined ? 'Contexto do projeto' : `Contexto de ${project.name}`}
      wide
      onClose={close}
    >
      {editor !== null &&
        (editor.files === null ? (
          <Loading error={editor.error} onCancel={close} />
        ) : (
          <ContextForm
            key={editor.projectId}
            files={editor.files}
            saving={editor.saving}
            error={editor.error}
            onCancel={close}
          />
        ))}
    </Dialog>
  );
}

function Loading({ error, onCancel }: { error: string | null; onCancel: () => void }) {
  return (
    <div className={styles.form}>
      {error === null ? (
        <p role="status" className={styles.intro}>
          Carregando…
        </p>
      ) : (
        <p role="alert" className={field.error}>
          {error}
        </p>
      )}
      <div className={styles.footer}>
        <Button onClick={onCancel}>Fechar</Button>
      </div>
    </div>
  );
}

interface ContextFormProps {
  files: NonNullable<ContextEditor['files']>;
  saving: boolean;
  error: string | null;
  onCancel: () => void;
}

function ContextForm({ files, saving, error, onCancel }: ContextFormProps) {
  const saveContext = useAppStore((state) => state.saveContext);
  const [rules, setRules] = useState(files.rules);
  const [plan, setPlan] = useState(files.plan);
  const ids = { intro: useId(), rules: useId(), plan: useId() };
  const rulesRef = useRef<HTMLTextAreaElement>(null);

  // The files arrive after the dialog opened, so its autofocus already ran.
  useEffect(() => rulesRef.current?.focus(), []);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    saveContext(rules, plan);
  };

  return (
    <form className={styles.form} aria-describedby={ids.intro} onSubmit={submit}>
      <p id={ids.intro} className={styles.intro}>
        O Claude lê as regras e o plano ao iniciar ou retomar uma sessão deste projeto, e pode
        atualizar os arquivos durante o trabalho.
      </p>

      <div className={field.field}>
        <label htmlFor={ids.rules} className={field.label}>
          Regras
        </label>
        <textarea
          ref={rulesRef}
          id={ids.rules}
          className={styles.textarea}
          value={rules}
          placeholder="Convenções, comandos de teste, o que evitar"
          spellCheck={false}
          onChange={(event) => setRules(event.target.value)}
        />
      </div>

      <div className={field.field}>
        <label htmlFor={ids.plan} className={field.label}>
          Plano
        </label>
        <textarea
          id={ids.plan}
          className={styles.textarea}
          value={plan}
          placeholder="Etapas, o que já foi feito, próximos passos"
          spellCheck={false}
          onChange={(event) => setPlan(event.target.value)}
        />
      </div>

      <div className={styles.location}>
        <span className={styles.path} title={files.dir}>
          {files.dir}
        </span>
        <Button
          variant="ghost"
          size="small"
          icon={<FolderOpen size={14} aria-hidden="true" />}
          onClick={() => void revealItemInDir(files.dir).catch(() => undefined)}
        >
          Mostrar pasta
        </Button>
      </div>

      {error !== null && (
        <p role="alert" className={field.error}>
          {error}
        </p>
      )}

      <div className={styles.footer}>
        <Button onClick={onCancel}>Cancelar</Button>
        <Button type="submit" variant="primary" disabled={saving}>
          {saving ? 'Salvando…' : 'Salvar'}
        </Button>
      </div>
    </form>
  );
}
