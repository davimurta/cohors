import { Check } from 'lucide-react';
import type { SessionMeta } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import { AGENT_LABEL, displayPath, formatElapsed } from '../session/format.ts';
import { useHomeDir } from '../session/useHomeDir.ts';
import { useNow } from '../session/useNow.ts';
import { useAppStore } from '../state/store.ts';
import styles from './FinishingView.module.css';

type StepState = 'done' | 'current' | 'upcoming';

// The process is stopped and the transcript flushed before the summary
// starts, so only the summary step is in progress while this is shown.
const STEPS: { label: string; state: StepState }[] = [
  { label: 'Processo encerrado', state: 'done' },
  { label: 'Transcript salvo', state: 'done' },
  { label: 'Gerando resumo', state: 'current' },
  { label: 'Salvando em Finalizadas', state: 'upcoming' },
];

/** Shown while Claude writes the summary of a session being finished. */
export function FinishingView({ session }: { session: SessionMeta }) {
  const home = useHomeDir();
  const now = useNow();
  const skipSummary = useAppStore((state) => state.skipSummary);

  return (
    <main className={styles.view}>
      <header className={styles.header}>
        <div className={styles.titleRow}>
          <h1 className={styles.title}>{session.name}</h1>
          <span className={styles.pill}>Finalizando</span>
        </div>
        <span className={styles.meta}>
          {displayPath(session.cwd, home)} · {AGENT_LABEL[session.agent]} · Iniciada{' '}
          {formatElapsed(session.createdAt, now)}
        </span>
      </header>
      <section className={styles.panel} aria-labelledby="finishing-title" aria-busy="true">
        <div className={styles.content}>
          <div className={styles.text}>
            <h2 id="finishing-title" className={styles.panelTitle}>
              Gerando o resumo da sessão
            </h2>
            <p className={styles.description}>
              O Claude está resumindo o que foi feito. Isso usa o limite do seu plano e leva
              alguns segundos.
            </p>
          </div>
          <ol className={styles.steps}>
            {STEPS.map((step) => (
              <li
                key={step.label}
                className={`${styles.step} ${step.state === 'done' ? '' : styles[step.state]}`}
                aria-current={step.state === 'current' ? 'step' : undefined}
              >
                <StepMarker state={step.state} />
                {step.label}
              </li>
            ))}
          </ol>
          <div className={styles.actions}>
            <Button onClick={() => skipSummary(session.id)}>Finalizar sem resumo</Button>
          </div>
        </div>
      </section>
    </main>
  );
}

function StepMarker({ state }: { state: StepState }) {
  if (state === 'done') {
    return (
      <span className={`${styles.marker} ${styles.done}`}>
        <Check size={12} strokeWidth={3} aria-hidden="true" />
      </span>
    );
  }
  if (state === 'current') {
    return (
      <span className={styles.marker}>
        <span className={styles.spinner} aria-hidden="true" />
      </span>
    );
  }
  return <span className={`${styles.marker} ${styles.pending}`} aria-hidden="true" />;
}
