import { CircleCheck, Play, Power, RotateCcw } from 'lucide-react';
import type { SessionMeta } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import { useAppStore } from '../state/store.ts';
import styles from './EndedPanel.module.css';

/** Shown instead of the terminal when the session has no running process. */
export function EndedPanel({ session }: { session: SessionMeta }) {
  const resuming = useAppStore((state) => state.resumingId === session.id);
  const resumeError = useAppStore((state) => state.resumeError);
  const resumeSession = useAppStore((state) => state.resumeSession);
  const interrupted = session.status === 'interrupted';
  const isClaude = session.agent === 'claude';

  return (
    <section
      className={`${styles.panel} ${interrupted ? styles.interrupted : ''}`}
      aria-labelledby="ended-title"
    >
      <div className={styles.content}>
        <span className={styles.icon}>
          {interrupted ? (
            <RotateCcw size={24} strokeWidth={1.75} aria-hidden="true" />
          ) : (
            <Power size={24} strokeWidth={1.75} aria-hidden="true" />
          )}
        </span>
        <div className={styles.text}>
          <h2 id="ended-title" className={styles.title}>
            {interrupted ? 'Esta sessão foi interrompida' : 'Esta sessão foi encerrada'}
          </h2>
          <p className={styles.description}>{description(session)}</p>
        </div>
        <div className={styles.actions}>
          <Button
            variant="primary"
            size="large"
            icon={<Play size={16} aria-hidden="true" />}
            disabled={resuming}
            onClick={() => resumeSession(session.id)}
          >
            {resuming ? 'Retomando…' : 'Retomar sessão'}
          </Button>
          <Button size="large" icon={<CircleCheck size={16} aria-hidden="true" />} disabled>
            {isClaude ? 'Finalizar e gerar resumo' : 'Finalizar sessão'}
          </Button>
        </div>
        {resumeError !== null && (
          <p role="alert" className={styles.error}>
            {resumeError}
          </p>
        )}
        <div className={styles.note}>
          {isClaude ? (
            <>
              <span>
                Ao retomar, o Cohors reabre o Claude Code na mesma pasta com o histórico desta
                conversa.
              </span>
              <span className={styles.command}>claude --resume {session.agentSessionId}</span>
            </>
          ) : (
            <span>Ao retomar, o Cohors abre um novo shell na mesma pasta.</span>
          )}
        </div>
      </div>
    </section>
  );
}

function description(session: SessionMeta): string {
  const next =
    session.agent === 'claude'
      ? 'Você pode continuar a conversa de onde parou ou finalizar a sessão.'
      : 'Você pode abrir um novo shell na mesma pasta ou finalizar a sessão.';
  if (session.status === 'interrupted') {
    return `O processo não está mais rodando, provavelmente por causa de uma reinicialização. ${next}`;
  }
  const code = session.exitCode === null ? '' : ` (código ${session.exitCode})`;
  return `O processo terminou${code}. ${next}`;
}
