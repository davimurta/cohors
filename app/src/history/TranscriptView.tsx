import { useEffect, useRef } from 'react';
import { FitAddon } from '@xterm/addon-fit';
import { Terminal } from '@xterm/xterm';
import '@xterm/xterm/css/xterm.css';
import { Button } from '../components/Button.tsx';
import type { TranscriptView as Transcript } from '../state/sessions.ts';
import { useAppStore } from '../state/store.ts';
import { terminalOptions } from '../terminal/registry.ts';
import { flattenTranscript } from './flattenTranscript.ts';
import styles from './TranscriptView.module.css';

const SCROLLBACK_LINES = 100_000;

/** The raw transcript replayed in a read-only terminal. */
export function TranscriptView({ name, transcript }: { name: string; transcript: Transcript }) {
  const closeTranscript = useAppStore((state) => state.closeTranscript);
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
    <main className={styles.view}>
      <header className={styles.header}>
        <div className={styles.heading}>
          <h1 className={styles.title}>Transcript de {name}</h1>
          <span className={styles.meta}>
            {transcript.data === null
              ? 'Carregando…'
              : transcript.truncated
                ? 'Somente leitura. Mostrando os últimos 2 MB.'
                : 'Somente leitura.'}
          </span>
        </div>
        <Button onClick={closeTranscript}>
          Voltar
        </Button>
      </header>
      <section ref={containerRef} className={styles.terminal} aria-label={`Transcript de ${name}`} />
    </main>
  );
}
