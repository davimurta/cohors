import { useEffect, useRef, useState } from 'react';
import { FitAddon } from '@xterm/addon-fit';
import { Terminal } from '@xterm/xterm';
import '@xterm/xterm/css/xterm.css';
import { Button } from '../components/Button.tsx';
import type { TranscriptView as Transcript } from '../state/sessions.ts';
import { useAppStore } from '../state/store.ts';
import { terminalOptions } from '../terminal/registry.ts';
import { ConversationView } from './ConversationView.tsx';
import { flattenTranscript } from './flattenTranscript.ts';
import styles from './TranscriptView.module.css';

const SCROLLBACK_LINES = 100_000;

type Mode = 'conversation' | 'terminal';

/**
 * The session as a conversation, read from Claude's own log, or as the raw
 * terminal output replayed read-only. Shells only have the terminal.
 */
export function TranscriptView({ name, transcript }: { name: string; transcript: Transcript }) {
  const closeTranscript = useAppStore((state) => state.closeTranscript);
  const loadConversation = useAppStore((state) => state.loadConversation);
  const conversation = useAppStore((state) => state.conversations[transcript.id]);
  const [chosen, setChosen] = useState<Mode | null>(null);
  const entries = conversation?.entries ?? null;
  const mode: Mode = chosen ?? (entries === null ? 'terminal' : 'conversation');

  useEffect(() => loadConversation(transcript.id), [loadConversation, transcript.id]);

  return (
    <main className={styles.view}>
      <header className={styles.header}>
        <div className={styles.heading}>
          <h1 className={styles.title}>Transcript de {name}</h1>
          <span className={styles.meta}>
            {mode === 'conversation'
              ? 'Mensagens da conversa com o Claude.'
              : transcript.data === null
                ? 'Carregando…'
                : transcript.truncated
                  ? 'Somente leitura. Mostrando os últimos 2 MB.'
                  : 'Somente leitura.'}
          </span>
        </div>
        {entries !== null && (
          <div className={styles.modes} role="group" aria-label="Formato do transcript">
            {(
              [
                ['conversation', 'Conversa'],
                ['terminal', 'Terminal'],
              ] as const
            ).map(([value, label]) => (
              <button
                key={value}
                type="button"
                className={styles.mode}
                aria-pressed={mode === value}
                onClick={() => setChosen(value)}
              >
                {label}
              </button>
            ))}
          </div>
        )}
        <Button onClick={closeTranscript}>Voltar</Button>
      </header>
      {mode === 'conversation' && entries !== null ? (
        <section className={styles.conversation} aria-label={`Conversa de ${name}`}>
          <ConversationView entries={entries} />
        </section>
      ) : (
        <TerminalReplay name={name} transcript={transcript} />
      )}
    </main>
  );
}

function TerminalReplay({ name, transcript }: { name: string; transcript: Transcript }) {
  const containerRef = useRef<HTMLElement>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (container === null || transcript.data === null) return;
    const terminal = new Terminal({
      ...terminalOptions(),
      disableStdin: true,
      cursorBlink: false,
      scrollback: SCROLLBACK_LINES,
    });
    const fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.open(container);
    fit.fit();
    terminal.write(flattenTranscript(transcript.data, terminal.rows));
    const observer = new ResizeObserver(() => fit.fit());
    observer.observe(container);
    return () => {
      observer.disconnect();
      terminal.dispose();
    };
  }, [transcript.data]);

  return (
    <section ref={containerRef} className={styles.terminal} aria-label={`Transcript de ${name}`} />
  );
}
