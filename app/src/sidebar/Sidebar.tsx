import { Plus, Power } from 'lucide-react';
import { Button } from '../components/Button.tsx';
import type { ConnectionStatus } from '../daemon/client.ts';
import { NEW_SESSION_KEYS, NEW_SESSION_SHORTCUT } from '../platform.ts';
import { useHomeDir } from '../session/useHomeDir.ts';
import { useAppStore } from '../state/store.ts';
import { SessionItem } from './SessionItem.tsx';
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
        onClick={openNewSession}
      >
        <span className={styles.newSessionLabel}>Nova sessão</span>
        <span className={styles.shortcut}>{NEW_SESSION_SHORTCUT}</span>
      </Button>

      <div role="tablist" aria-label="Sessões" className={styles.tabs}>
        <button type="button" role="tab" aria-selected="true" className={styles.tab}>
          Ativas <span className={styles.count}>{sessions.length}</span>
        </button>
        <button type="button" role="tab" aria-selected="false" className={styles.tab} disabled>
          Finalizadas <span className={styles.count}>0</span>
        </button>
      </div>

      {sessions.length === 0 ? (
        <p className={styles.empty}>Suas sessões aparecem aqui.</p>
      ) : (
        <nav aria-label="Sessões ativas" className={styles.list}>
          {sessions.map((session) => (
            <SessionItem
              key={session.id}
              session={session}
              selected={session.id === selectedId}
              home={home}
              onSelect={select}
            />
          ))}
        </nav>
      )}

      <div className={styles.footer}>
        <span className={`${styles.connection} ${styles[connection]}`} role="status">
          <span className={styles.connectionDot} aria-hidden="true" />
          {CONNECTION_LABEL[connection]}
        </span>
        {sessions.length > 0 && (
          <Button variant="ghost" size="small" icon={<Power size={14} aria-hidden="true" />} disabled>
            Encerrar tudo
          </Button>
        )}
      </div>
    </aside>
  );
}
