import { useId, useState, type FormEvent } from 'react';
import { Trash2 } from 'lucide-react';
import type { Project } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import { ConfirmDialog } from '../components/ConfirmDialog.tsx';
import { Dialog } from '../components/Dialog.tsx';
import field from '../components/Field.module.css';
import { FolderField } from '../components/FolderField.tsx';
import { Select } from '../components/Select.tsx';
import { useAppStore } from '../state/store.ts';
import { accountLabel, folderName } from './labels.ts';
import styles from './ProjectDialog.module.css';

// Select values are strings, so the default account and key need a stand-in.
const DEFAULT_OPTION = '';

export function ProjectDialog() {
  const dialog = useAppStore((state) => state.projectDialog);
  const close = useAppStore((state) => state.closeProjectDialog);
  const project = useAppStore((state) =>
    state.projects.find((candidate) => candidate.id === state.projectDialog?.projectId),
  );

  return (
    <Dialog
      open={dialog !== null}
      title={project === undefined ? 'Novo projeto' : 'Editar projeto'}
      onClose={close}
    >
      <ProjectForm project={project} onCancel={close} />
    </Dialog>
  );
}

function ProjectForm({ project, onCancel }: { project: Project | undefined; onCancel: () => void }) {
  const accounts = useAppStore((state) => state.claudeAccounts);
  const keys = useAppStore((state) => state.sshKeys);
  const saving = useAppStore((state) => state.savingProject);
  const error = useAppStore((state) => state.projectError);
  const saveProject = useAppStore((state) => state.saveProject);
  const deleteProject = useAppStore((state) => state.deleteProject);

  const [name, setName] = useState(project?.name ?? '');
  const [root, setRoot] = useState(project?.root ?? '');
  const [account, setAccount] = useState(project?.claudeConfigDir ?? DEFAULT_OPTION);
  const [sshKey, setSshKey] = useState(project?.sshKey ?? DEFAULT_OPTION);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const ids = {
    name: useId(),
    root: useId(),
    account: useId(),
    accountLabel: useId(),
    key: useId(),
    keyLabel: useId(),
    keyHint: useId(),
  };
  const complete = name.trim() !== '' && root !== '';

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!complete || saving) return;
    saveProject({
      name: name.trim(),
      root,
      claudeConfigDir: account === DEFAULT_OPTION ? null : account,
      sshKey: sshKey === DEFAULT_OPTION ? null : sshKey,
      contextDir: project?.contextDir ?? null,
    });
  };

  const chooseRoot = (folder: string) => {
    setRoot(folder);
    if (name.trim() === '') setName(folderName(folder));
  };

  return (
    <form className={styles.form} onSubmit={submit}>
      <div className={field.field}>
        <label htmlFor={ids.name} className={field.label}>
          Nome
        </label>
        <input
          id={ids.name}
          className={field.control}
          value={name}
          data-autofocus=""
          onChange={(event) => setName(event.target.value)}
        />
      </div>

      <FolderField id={ids.root} label="Pasta raiz" value={root} onChange={chooseRoot} />

      <div className={field.field}>
        <label id={ids.accountLabel} htmlFor={ids.account} className={field.label}>
          Conta do Claude
        </label>
        <Select
          id={ids.account}
          labelId={ids.accountLabel}
          value={account}
          options={accounts.map((candidate) => ({
            value: candidate.configDir ?? DEFAULT_OPTION,
            label: accountLabel(candidate),
          }))}
          onChange={setAccount}
        />
      </div>

      <div className={field.field}>
        <label id={ids.keyLabel} htmlFor={ids.key} className={field.label}>
          Chave SSH
        </label>
        <Select
          id={ids.key}
          labelId={ids.keyLabel}
          value={sshKey}
          aria-describedby={ids.keyHint}
          options={[
            { value: DEFAULT_OPTION, label: 'Configuração padrão do sistema' },
            ...keys.map((key) => ({ value: key.path, label: key.name })),
          ]}
          onChange={setSshKey}
        />
        <span id={ids.keyHint} className={field.hint}>
          Usada pelo git nas sessões deste projeto.
        </span>
      </div>

      {error !== null && (
        <p role="alert" className={field.error}>
          {error}
        </p>
      )}

      <div className={styles.footer}>
        {project !== undefined && (
          <Button
            variant="ghost"
            icon={<Trash2 size={16} aria-hidden="true" />}
            onClick={() => setConfirmingDelete(true)}
          >
            Excluir
          </Button>
        )}
        <span className={styles.spacer} />
        <Button onClick={onCancel}>Cancelar</Button>
        <Button type="submit" variant="primary" disabled={!complete || saving}>
          {saving ? 'Salvando…' : project === undefined ? 'Criar projeto' : 'Salvar'}
        </Button>
      </div>

      {project !== undefined && (
        <ConfirmDialog
          open={confirmingDelete}
          title="Excluir projeto"
          confirmLabel="Excluir projeto"
          onConfirm={() => deleteProject(project.id)}
          onClose={() => setConfirmingDelete(false)}
        >
          O projeto {project.name} será excluído. As sessões dele continuam rodando, sem projeto.
        </ConfirmDialog>
      )}
    </form>
  );
}
