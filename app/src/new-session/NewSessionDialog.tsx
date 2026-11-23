import { useId, useState, type FormEvent } from 'react';
import { Check } from 'lucide-react';
import type { Agent } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import { Dialog } from '../components/Dialog.tsx';
import field from '../components/Field.module.css';
import { FolderField } from '../components/FolderField.tsx';
import { Select } from '../components/Select.tsx';
import { environmentSummary, folderName } from '../projects/labels.ts';
import { AGENT_LABEL } from '../session/format.ts';
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
  { agent: 'codex', title: 'Codex', description: 'Agente de código da OpenAI no terminal' },
  { agent: 'gemini', title: 'Gemini', description: 'Agente do Google no terminal' },
  { agent: 'shell', title: 'Shell', description: 'Terminal comum do seu sistema' },
];

const AGENT_MISSING = 'Não encontrado neste computador';
/** Codex and Gemini resume by folder, not by conversation id. */
const RESUMES_LATEST: ReadonlySet<Agent> = new Set(['codex', 'gemini']);
const NO_PROJECT = '';

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
  const availableAgents = useAppStore((state) => state.availableAgents);
  const creating = useAppStore((state) => state.creating);
  const createError = useAppStore((state) => state.createError);
  const createSession = useAppStore((state) => state.createSession);
  const projects = useAppStore((state) => state.projects);
  const accounts = useAppStore((state) => state.claudeAccounts);
  const keys = useAppStore((state) => state.sshKeys);
  const presetProjectId = useAppStore((state) => state.newSessionProjectId);

  const presetProject = projects.find((project) => project.id === presetProjectId);
  const isMissing = (candidate: Agent) =>
    candidate !== 'shell' && availableAgents !== null && !availableAgents.includes(candidate);
  const [agent, setAgent] = useState<Agent>(isMissing('claude') ? 'shell' : 'claude');
  const [projectId, setProjectId] = useState(presetProject?.id ?? NO_PROJECT);
  const [cwd, setCwd] = useState(presetProject?.root ?? '');
  const [name, setName] = useState('');
  const [worktree, setWorktree] = useState(false);
  const ids = {
    agent: useId(),
    agentHint: useId(),
    project: useId(),
    projectLabel: useId(),
    projectHint: useId(),
    folder: useId(),
    name: useId(),
    nameHint: useId(),
    worktree: useId(),
    worktreeHint: useId(),
  };
  const project = projects.find((candidate) => candidate.id === projectId);

  const changeProject = (nextId: string) => {
    const next = projects.find((candidate) => candidate.id === nextId);
    // Follow the project root unless the user picked another folder.
    if (cwd === '' || cwd === project?.root) setCwd(next?.root ?? '');
    setProjectId(nextId);
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (cwd === '' || creating) return;
    createSession({ agent, cwd, name, projectId: project?.id ?? null, worktree });
  };

  return (
    <form className={styles.form} onSubmit={submit}>
      <div
        className={styles.group}
        role="group"
        aria-labelledby={ids.agent}
        aria-describedby={RESUMES_LATEST.has(agent) ? ids.agentHint : undefined}
      >
        <span id={ids.agent} className={field.label}>
          Agente
        </span>
        <div className={styles.agents}>
          {AGENT_OPTIONS.map((option) => {
            const missing = isMissing(option.agent);
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
                  {selected && (
                    <Check size={16} strokeWidth={2.25} className={styles.check} aria-hidden="true" />
                  )}
                </span>
                <span className={styles.agentDescription}>
                  {missing ? AGENT_MISSING : option.description}
                </span>
              </button>
            );
          })}
        </div>
        {RESUMES_LATEST.has(agent) && (
          <span id={ids.agentHint} className={field.hint}>
            Ao retomar, o {AGENT_LABEL[agent]} abre a conversa mais recente desta pasta. Para várias
            sessões na mesma pasta, isole cada uma em worktree.
          </span>
        )}
      </div>

      {projects.length > 0 && (
        <div className={field.field}>
          <label id={ids.projectLabel} htmlFor={ids.project} className={field.label}>
            Projeto
          </label>
          <Select
            id={ids.project}
            labelId={ids.projectLabel}
            value={projectId}
            aria-describedby={ids.projectHint}
            options={[
              { value: NO_PROJECT, label: 'Nenhum' },
              ...projects.map((candidate) => ({ value: candidate.id, label: candidate.name })),
            ]}
            onChange={changeProject}
          />
          <span id={ids.projectHint} className={field.hint}>
            {project === undefined
              ? 'Sem projeto, a sessão usa a conta padrão do Claude.'
              : environmentSummary(project, accounts, keys)}
          </span>
        </div>
      )}

      <FolderField
        id={ids.folder}
        label="Pasta"
        value={cwd}
        {...(project === undefined ? {} : { startIn: project.root })}
        onChange={setCwd}
      />

      <div className={field.field}>
        <label htmlFor={ids.name} className={field.label}>
          Nome <span className={styles.optional}>(opcional)</span>
        </label>
        <input
          id={ids.name}
          className={field.control}
          value={name}
          placeholder={cwd === '' ? '' : folderName(cwd)}
          aria-describedby={ids.nameHint}
          onChange={(event) => setName(event.target.value)}
        />
        <span id={ids.nameHint} className={field.hint}>
          Se ficar em branco, usamos o nome da pasta.
        </span>
      </div>

      <div className={styles.checkField}>
        <input
          id={ids.worktree}
          type="checkbox"
          className={styles.checkbox}
          checked={worktree}
          aria-describedby={ids.worktreeHint}
          onChange={(event) => setWorktree(event.target.checked)}
        />
        <div className={styles.checkText}>
          <label htmlFor={ids.worktree} className={field.label}>
            Isolar em worktree
          </label>
          <span id={ids.worktreeHint} className={field.hint}>
            Cria uma branch e uma pasta só para esta sessão, a partir do commit atual. A pasta
            precisa estar num repositório git.
          </span>
        </div>
      </div>

      {createError !== null && (
        <p role="alert" className={field.error}>
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
