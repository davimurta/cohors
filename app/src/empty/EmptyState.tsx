import type { MouseEvent } from 'react';
import { Plus, Terminal, TriangleAlert } from 'lucide-react';
import { isTauri } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { Button } from '../components/Button.tsx';
import { useAppStore } from '../state/store.ts';
import styles from './EmptyState.module.css';

const INSTALL_URL = 'https://docs.claude.com/en/docs/claude-code/overview';

// The app web view must not navigate away, so links open in the system browser.
function openInBrowser(event: MouseEvent<HTMLAnchorElement>) {
  if (!isTauri()) return;
  event.preventDefault();
  void openUrl(event.currentTarget.href);
}

export function EmptyState() {
  const claudeAvailable = useAppStore((state) => state.claudeAvailable);
  const openNewSession = useAppStore((state) => state.openNewSession);

  return (
    <main className={styles.main}>
      <div className={styles.content}>
        <span className={styles.icon}>
          <Terminal size={24} strokeWidth={1.75} aria-hidden="true" />
        </span>
        <div className={styles.text}>
          <h1 className={styles.title}>Nenhuma sessão ainda</h1>
          <p className={styles.description}>
            Abra o Claude Code ou um shell em qualquer pasta. As sessões continuam rodando mesmo
            quando você fecha o Cohors.
          </p>
        </div>
        <Button
          variant="primary"
          size="large"
          icon={<Plus size={16} aria-hidden="true" />}
          onClick={openNewSession}
        >
          Nova sessão
        </Button>

        {claudeAvailable === false && (
          <div role="note" className={styles.note}>
            <span className={styles.noteIcon}>
              <TriangleAlert size={18} aria-hidden="true" />
            </span>
            <div className={styles.noteText}>
              <span className={styles.noteTitle}>Claude Code não encontrado</span>
              <span>
                Instale o CLI e faça login nele para abrir sessões do Claude. O shell comum funciona
                normalmente.{' '}
                <a href={INSTALL_URL} target="_blank" rel="noreferrer" onClick={openInBrowser}>
                  Ver instruções de instalação
                </a>
              </span>
            </div>
          </div>
        )}
      </div>
    </main>
  );
}
