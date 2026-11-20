import { useEffect, useRef } from 'react';
import { FitAddon } from '@xterm/addon-fit';
import { Terminal } from '@xterm/xterm';
import '@xterm/xterm/css/xterm.css';
import type { ServerMessage } from '@cohors/protocol';
import { DaemonClient } from '../daemon/client.ts';

/** Attaches to the first daemon session and mirrors it in a terminal. */
export function SessionTerminal() {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (container === null) return;

    const terminal = new Terminal({ fontSize: 13, lineHeight: 1.2 });
    const fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.open(container);
    fit.fit();

    const client = new DaemonClient();
    let sessionId: string | null = null;

    const sendResize = () => {
      if (sessionId === null) return;
      client.send({ type: 'session.resize', id: sessionId, cols: terminal.cols, rows: terminal.rows });
    };

    const handleMessage = (message: ServerMessage) => {
      switch (message.type) {
        case 'ready':
          client.send({ type: 'session.list' });
          break;
        case 'session.list.result':
          sessionId = message.sessions[0]?.id ?? null;
          if (sessionId !== null) client.send({ type: 'session.attach', id: sessionId });
          break;
        case 'session.snapshot':
          terminal.reset();
          terminal.write(message.data);
          sendResize();
          break;
        case 'session.output':
          terminal.write(message.data);
          break;
        case 'error':
          console.error(`daemon error ${message.code}: ${message.message}`);
          break;
      }
    };

    const unsubscribe = client.onMessage(handleMessage);
    const input = terminal.onData((data) => {
      if (sessionId !== null) client.send({ type: 'session.input', id: sessionId, data });
    });
    const resize = terminal.onResize(sendResize);
    const observer = new ResizeObserver(() => fit.fit());
    observer.observe(container);
    client.start();

    return () => {
      observer.disconnect();
      resize.dispose();
      input.dispose();
      unsubscribe();
      client.stop();
      terminal.dispose();
    };
  }, []);

  return <div ref={containerRef} style={{ height: '100%' }} />;
}
