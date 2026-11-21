import { FitAddon } from '@xterm/addon-fit';
import { Terminal, type ITheme } from '@xterm/xterm';
import type { ServerMessage } from '@cohors/protocol';
import { daemon } from '../state/store.ts';

const FONT_SIZE = 13;
// Taller lines leave gaps between the box drawing characters of TUIs.
const LINE_HEIGHT = 1.2;

/** One xterm per session, created on first view and kept while switching. */
class SessionTerminal {
  readonly host = document.createElement('div');
  private readonly id: string;
  private readonly terminal: Terminal;
  private readonly fit = new FitAddon();
  private opened = false;
  private visible = false;
  private lastSeq = 0;

  constructor(id: string) {
    this.id = id;
    this.host.style.height = '100%';
    this.terminal = new Terminal({
      fontFamily: cssToken('--font-mono'),
      fontSize: FONT_SIZE,
      lineHeight: LINE_HEIGHT,
      theme: terminalTheme(),
    });
    this.terminal.loadAddon(this.fit);
    this.terminal.onData((data) => daemon.send({ type: 'session.input', id, data }));
    this.terminal.onResize(() => this.sendResize());
  }

  /** Called once the host is in the document, so xterm can measure it. */
  show(): void {
    if (!this.opened) {
      this.terminal.open(this.host);
      this.opened = true;
      this.attach();
    }
    this.visible = true;
    this.fitToHost();
    this.terminal.focus();
  }

  hide(): void {
    this.visible = false;
  }

  dispose(): void {
    this.terminal.dispose();
    this.host.remove();
  }

  fitToHost(): void {
    if (this.visible) this.fit.fit();
  }

  attach(): void {
    daemon.send({ type: 'session.attach', id: this.id });
  }

  receive(message: ServerMessage): void {
    if (message.type === 'session.snapshot') {
      this.lastSeq = message.seq;
      this.terminal.reset();
      this.terminal.write(message.data);
      this.forceRedraw();
    } else if (message.type === 'session.output') {
      if (message.seq <= this.lastSeq) return;
      if (message.seq !== this.lastSeq + 1) {
        this.attach();
        return;
      }
      this.lastSeq = message.seq;
      this.terminal.write(message.data);
    }
  }

  // The kernel only signals SIGWINCH when the size changes, so shrink by one
  // row and restore it to make the agent repaint over the replay.
  private forceRedraw(): void {
    if (!this.visible) return;
    this.sendResize(Math.max(this.terminal.rows - 1, 1));
    this.sendResize();
  }

  // Only the visible terminal is fitted, so only it may size the PTY.
  private sendResize(rows = this.terminal.rows): void {
    if (!this.visible) return;
    daemon.send({ type: 'session.resize', id: this.id, cols: this.terminal.cols, rows });
  }
}

const terminals = new Map<string, SessionTerminal>();

export function terminalFor(id: string): SessionTerminal {
  let terminal = terminals.get(id);
  if (terminal === undefined) {
    terminal = new SessionTerminal(id);
    terminals.set(id, terminal);
  }
  return terminal;
}

/** Routes output to terminals and re-attaches them after a reconnect. */
export function startTerminalRouting(): () => void {
  return daemon.onMessage((message) => {
    if (message.type === 'session.list.result') {
      syncWithSessions(new Set(message.sessions.map((session) => session.id)));
    } else if (message.type === 'session.snapshot' || message.type === 'session.output') {
      terminals.get(message.id)?.receive(message);
    }
  });
}

/** Drops terminals of sessions the daemon no longer has, re-attaches the rest. */
function syncWithSessions(ids: Set<string>): void {
  for (const [id, terminal] of terminals) {
    if (ids.has(id)) {
      terminal.attach();
    } else {
      terminal.dispose();
      terminals.delete(id);
    }
  }
}

function cssToken(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function terminalTheme(): ITheme {
  return {
    background: cssToken('--color-terminal-bg'),
    foreground: cssToken('--color-terminal-text'),
    cursor: cssToken('--color-terminal-text'),
    cursorAccent: cssToken('--color-terminal-bg'),
    selectionBackground: cssToken('--color-terminal-divider'),
  };
}
