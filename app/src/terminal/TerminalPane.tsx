import { useEffect, useRef } from 'react';
import '@xterm/xterm/css/xterm.css';
import { terminalFor } from './registry.ts';
import styles from './TerminalPane.module.css';

interface TerminalPaneProps {
  sessionId: string;
  label: string;
  /** Whether the terminal takes keyboard focus when shown. */
  focus?: boolean;
}

/** Shows the session terminal; the xterm itself outlives this component. */
export function TerminalPane({ sessionId, label, focus = true }: TerminalPaneProps) {
  const containerRef = useRef<HTMLElement>(null);
  // Read when the terminal first shows; later focus changes come from clicks.
  const focusRef = useRef(focus);
  useEffect(() => {
    focusRef.current = focus;
  }, [focus]);

  useEffect(() => {
    const container = containerRef.current;
    if (container === null) return;
    const terminal = terminalFor(sessionId);
    container.append(terminal.host);
    let cancelled = false;
    // Measure with the real font, or xterm sizes the grid with a fallback.
    void document.fonts.load(`13px ${getComputedStyle(container).getPropertyValue('--font-mono')}`).then(() => {
      if (!cancelled) terminal.show(focusRef.current);
    });
    const observer = new ResizeObserver(() => terminal.fitToHost());
    observer.observe(container);
    return () => {
      cancelled = true;
      observer.disconnect();
      terminal.hide();
      terminal.host.remove();
    };
  }, [sessionId]);

  return <section ref={containerRef} className={styles.pane} aria-label={label} />;
}
