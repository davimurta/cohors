import { useId, useState, type FormEvent } from 'react';
import { Check, Folder } from 'lucide-react';
import { open as openFolderPicker } from '@tauri-apps/plugin-dialog';
import type { Agent } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import { Dialog } from '../components/Dialog.tsx';
import { useAppStore } from '../state/store.ts';
import styles from './NewSessionDialog.module.css';

interface AgentOption {
  agent: Agent;
  title: string;
  description: string;
}

const AGENT_OPTIONS: AgentOption[] = [
  {
    agent: 'claude',
    title: 'Claude Code',
    description: 'Agente de código no terminal, com o seu login',
  },
  { agent: 'shell', title: 'Shell', description: 'Terminal comum do seu sistema' },
];

const CLAUDE_MISSING = 'Não encontrado neste computador';

export function NewSessionDialog() {
  const open = useAppStore((state) => state.newSessionOpen);
  const close = useAppStore((state) => state.closeNewSession);

  return (
    <Dialog open={open} title="Nova sessão" onClose={close}>
      <NewSessionForm onCancel={close} />
    </Dialog>
  );
}

function NewSessionForm({ onCancel }: { onCancel: () => void }) {
  const claudeAvailable = useAppStore((state) => state.claudeAvailable);
  const creating = useAppStore((state) => state.creating);
  const createError = useAppStore((state) => state.createError);
  const createSession = useAppStore((state) => state.createSession);
  const [agent, setAgent] = useState<Agent>(claudeAvailable === false ? 'shell' : 'claude');
  const [cwd, setCwd] = useState('');
  const [name, setName] = useState('');
  const ids = { agent: useId(), folder: useId(), name: useId(), nameHint: useId() };

  const chooseFolder = async () => {
    const folder = await openFolderPicker({ directory: true, multiple: false }).catch(() => null);
    if (typeof folder === 'string') setCwd(folder);
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (cwd !== '' && !creating) createSession({ agent, cwd, name });
  };

  return (
    <form className={styles.form} onSubmit={submit}>
      <div className={styles.field} role="group" aria-labelledby={ids.agent}>
        <span id={ids.agent} className={styles.label}>
          Agente
        </span>
        <div className={styles.agents}>
          {AGENT_OPTIONS.map((option) => {
            const missing = option.agent === 'claude' && claudeAvailable === false;
            const selected = agent === option.agent;
            return (
              <button
                key={option.agent}
                type="button"
                className={styles.agent}
                aria-pressed={selected}
                disabled={missing}
                data-autofocus={selected ? '' : undefined}
                onClick={() => setAgent(option.agent)}
              >
                <span className={styles.agentTitle}>
                  {option.title}
                  {selected && <Check size={16} strokeWidth={2.25} className={styles.check} aria-hidden="true" />}
                </span>
                <span className={styles.agentDescription}>
                  {missing ? CLAUDE_MISSING : option.description}
                </span>
              </button>
            );
          })}
        </div>
      </div>

      <div className={styles.field}>
        <label htmlFor={ids.folder} className={styles.label}>
          Pasta
        </label>
        <div className={styles.folderRow}>
          <input
            id={ids.folder}
            className={`${styles.input} ${styles.folderInput}`}
            value={cwd}
            placeholder="Nenhuma pasta escolhida"
            readOnly
          />
          <Button icon={<Folder size={16} aria-hidden="true" />} onClick={() => void chooseFolder()}>
            Escolher
          </Button>
        </div>
      </div>

      <div className={styles.field}>
        <label htmlFor={ids.name} className={styles.label}>
          Nome <span className={styles.optional}>(opcional)</span>
        </label>
        <input
          id={ids.name}
          className={styles.input}
          value={name}
          placeholder={folderName(cwd)}
          aria-describedby={ids.nameHint}
          onChange={(event) => setName(event.target.value)}
        />
        <span id={ids.nameHint} className={styles.hint}>
          Se ficar em branco, usamos o nome da pasta.
        </span>
      </div>

      {createError !== null && (
        <p role="alert" className={styles.error}>
          {createError}
        </p>
      )}

      <div className={styles.footer}>
        <Button onClick={onCancel}>Cancelar</Button>
        <Button type="submit" variant="primary" disabled={cwd === '' || creating}>
          {creating ? 'Criando…' : 'Criar sessão'}
        </Button>
      </div>
    </form>
  );
}

function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? '';
}
