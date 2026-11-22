import { FolderPlus, Pencil, Plus } from 'lucide-react';
import type { Project, SessionMeta } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import { groupSessions } from '../projects/groupSessions.ts';
import { useNow } from '../session/useNow.ts';
import { useAppStore } from '../state/store.ts';
import { SessionItem } from './SessionItem.tsx';
import styles from './SessionList.module.css';

interface SessionListProps {
  sessions: SessionMeta[];
  selectedId: string | null;
  home: string | null;
  onSelect: (id: string) => void;
}

export function SessionList({ sessions, selectedId, home, onSelect }: SessionListProps) {
  const projects = useAppStore((state) => state.projects);
  const openProjectDialog = useAppStore((state) => state.openProjectDialog);
  const now = useNow();
  const groups = groupSessions(sessions, projects);
  const showHeaders = projects.length > 0;

  return (
    <nav aria-label="Sessões ativas" className={styles.list}>
      {sessions.length === 0 && projects.length === 0 && (
        <p className={styles.empty}>Suas sessões aparecem aqui.</p>
      )}
      {groups.map((group) => (
        <section
          key={group.project?.id ?? 'loose'}
          className={styles.group}
          aria-label={group.project?.name ?? 'Sem projeto'}
        >
          {showHeaders && (
            <GroupHeader project={group.project} onEdit={() => openProjectDialog(group.project?.id ?? null)} />
          )}
          {group.sessions.map((session) => (
            <SessionItem
              key={session.id}
              session={session}
              selected={session.id === selectedId}
              home={home}
              now={now}
              onSelect={onSelect}
            />
          ))}
          {group.project !== null && group.sessions.length === 0 && (
            <p className={styles.emptyGroup}>Nenhuma sessão</p>
          )}
        </section>
      ))}
      <Button
        variant="ghost"
        size="small"
        className={styles.newProject}
        icon={<FolderPlus size={14} aria-hidden="true" />}
        onClick={() => openProjectDialog(null)}
      >
        Novo projeto
      </Button>
    </nav>
  );
}

function GroupHeader({ project, onEdit }: { project: Project | null; onEdit: () => void }) {
  const openNewSession = useAppStore((state) => state.openNewSession);
  if (project === null) {
    return (
      <div className={styles.groupHeader}>
        <span className={styles.groupName}>Sem projeto</span>
      </div>
    );
  }
  return (
    <div className={styles.groupHeader}>
      <span className={styles.groupName} title={project.root}>
        {project.name}
      </span>
      <button
        type="button"
        className={styles.iconButton}
        aria-label={`Nova sessão em ${project.name}`}
        title="Nova sessão neste projeto"
        onClick={() => openNewSession(project.id)}
      >
        <Plus size={14} aria-hidden="true" />
      </button>
      <button
        type="button"
        className={styles.iconButton}
        aria-label={`Editar projeto ${project.name}`}
        title="Editar projeto"
        onClick={onEdit}
      >
        <Pencil size={14} aria-hidden="true" />
      </button>
    </div>
  );
}
