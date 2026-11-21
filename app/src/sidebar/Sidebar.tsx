import type { KeyboardEvent } from 'react';
import { Plus } from 'lucide-react';
import type { SessionMeta } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import type { ConnectionStatus } from '../daemon/client.ts';
import { NEW_SESSION_KEYS, NEW_SESSION_SHORTCUT } from '../platform.ts';
import { useHomeDir } from '../session/useHomeDir.ts';
import { inTab, type SidebarTab } from '../state/sessions.ts';
import { useAppStore } from '../state/store.ts';
import { FinishedList } from './FinishedList.tsx';
import { SessionList } from './SessionList.tsx';
import { ShutdownAllButton } from './ShutdownAllButton.tsx';
import styles from './Sidebar.module.css';

const CONNECTION_LABEL: Record<ConnectionStatus, string> = {
  connecting: 'Conectando ao daemon',
  ready: 'Daemon conectado',
  disconnected: 'Daemon desconectado',
  unavailable: 'Daemon desconectado',
  incompatible: 'Daemon incompatível',
};

export function Sidebar() {
  const sessions = useAppStore((state) => state.sessions);
  const selectedId = useAppStore((state) => state.selectedId);
  const connection = useAppStore((state) => state.connection);
  const select = useAppStore((state) => state.select);
  const openNewSession = useAppStore((state) => state.openNewSession);
  const home = useHomeDir();
  const daemonDown = connection === 'unavailable' || connection === 'incompatible';

  return (
    <aside className={styles.sidebar}>
      <div className={styles.brand}>
        <span className={styles.logo}>
          <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="#FFFFFF">
            <circle cx="7" cy="7" r="3.2" />
            <circle cx="17" cy="7" r="3.2" />
            <circle cx="7" cy="17" r="3.2" />
            <circle cx="17" cy="17" r="3.2" />
          </svg>
        </span>
        Cohors
      </div>

      <Button
        variant="primary"
        className={styles.newSession}
        icon={<Plus size={16} aria-hidden="true" />}
        aria-keyshortcuts={NEW_SESSION_KEYS}
        disabled={daemonDown}
        onClick={() => openNewSession()}
      >
        <span className={styles.newSessionLabel}>Nova sessão</span>
        {!daemonDown && <span className={styles.shortcut}>{NEW_SESSION_SHORTCUT}</span>}
      </Button>

      {daemonDown ? (
        <p className={styles.empty}>As sessões ficam indisponíveis até o daemon voltar.</p>
      ) : (
        <SessionsNav sessions={sessions} selectedId={selectedId} home={home} onSelect={select} />
      )}

      <div className={styles.footer}>
        <span className={`${styles.connection} ${styles[connection]}`} role="status">
          <span className={styles.connectionDot} aria-hidden="true" />
          {CONNECTION_LABEL[connection]}
        </span>
        {!daemonDown && sessions.length > 0 && <ShutdownAllButton />}
      </div>
    </aside>
  );
}

interface SessionsNavProps {
  sessions: SessionMeta[];
  selectedId: string | null;
  home: string | null;
  onSelect: (id: string) => void;
}

function SessionsNav({ sessions, selectedId, home, onSelect }: SessionsNavProps) {
  const tab = useAppStore((state) => state.sidebarTab);
  const setTab = useAppStore((state) => state.setSidebarTab);
  const active = sessions.filter((session) => inTab(session, 'active'));
  const finished = sessions.filter((session) => inTab(session, 'finished'));
  const tabs: { id: SidebarTab; label: string; count: number }[] = [
    { id: 'active', label: 'Ativas', count: active.length },
    { id: 'finished', label: 'Finalizadas', count: finished.length },
  ];

  // Arrow keys move between the two tabs, as in any tab list.
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    const next = tab === 'active' ? 'finished' : 'active';
    setTab(next);
    document.getElementById(`tab-${next}`)?.focus();
  };

  return (
    <>
      <div role="tablist" aria-label="Sessões" className={styles.tabs} onKeyDown={onKeyDown}>
        {tabs.map((item) => (
          <button
            key={item.id}
            id={`tab-${item.id}`}
            type="button"
            role="tab"
            aria-selected={tab === item.id}
            aria-controls="sessions-panel"
            tabIndex={tab === item.id ? 0 : -1}
            className={styles.tab}
            onClick={() => setTab(item.id)}
          >
            {item.label} <span className={styles.count}>{item.count}</span>
          </button>
        ))}
      </div>

      <div
        id="sessions-panel"
        role="tabpanel"
        aria-labelledby={`tab-${tab}`}
        className={styles.panel}
      >
        {tab === 'active' ? (
          <SessionList sessions={active} selectedId={selectedId} home={home} onSelect={onSelect} />
        ) : (
          <FinishedList
            sessions={finished}
            selectedId={selectedId}
            home={home}
            onSelect={onSelect}
          />
        )}
      </div>
    </>
  );
}
