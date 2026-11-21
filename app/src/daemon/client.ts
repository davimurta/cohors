import {
  PROTOCOL_VERSION,
  type ClientMessage,
  type ServerMessage,
} from '@cohors/protocol';
import { loadDaemonConnection, type DaemonConnection } from './connection.ts';
import { canManageDaemon, startDaemon, stopDaemon } from './lifecycle.ts';

const RETRY_DELAY_MS = 1000;
const UNAVAILABLE_RETRY_DELAY_MS = 3000;
/** Failed attempts in a row before the app shows the daemon error screen. */
const ATTEMPTS_BEFORE_UNAVAILABLE = 3;

/**
 * `disconnected`: lost and retrying. `unavailable`: still failing after
 * several attempts. `incompatible`: the daemon speaks another protocol and
 * must be restarted.
 */
export type ConnectionStatus =
  | 'connecting'
  | 'ready'
  | 'disconnected'
  | 'unavailable'
  | 'incompatible';

type MessageListener = (message: ServerMessage) => void;
type StatusListener = (status: ConnectionStatus, detail: string | null) => void;

type Attempt = { outcome: 'ready' } | { outcome: 'incompatible' | 'failed'; detail: string };

/**
 * Single entry point to the daemon: starts it when needed, connects,
 * authenticates and reconnects.
 */
export class DaemonClient {
  private socket: WebSocket | null = null;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private status: ConnectionStatus = 'connecting';
  private failures = 0;
  /** Bumped on every (re)start so stale connection attempts give up. */
  private generation = 0;
  private readonly messageListeners = new Set<MessageListener>();
  private readonly statusListeners = new Set<StatusListener>();

  start(): void {
    this.reset();
    void this.connect(this.generation);
  }

  stop(): void {
    this.reset();
  }

  /** Connects again right away, e.g. from "Tentar de novo". */
  retryNow(): void {
    this.setStatus('connecting', null);
    this.start();
  }

  /** Stops and starts the daemon. Its sessions become interrupted. */
  async restartDaemon(): Promise<void> {
    this.reset();
    this.setStatus('connecting', null);
    try {
      await stopDaemon();
    } catch (error) {
      this.setStatus('unavailable', describe(error));
      return;
    }
    this.start();
  }

  send(message: ClientMessage): void {
    if (this.socket?.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify(message));
    }
  }

  onMessage(listener: MessageListener): () => void {
    this.messageListeners.add(listener);
    return () => this.messageListeners.delete(listener);
  }

  onStatus(listener: StatusListener): () => void {
    this.statusListeners.add(listener);
    return () => this.statusListeners.delete(listener);
  }

  private reset(): void {
    this.generation += 1;
    this.failures = 0;
    if (this.retryTimer !== null) clearTimeout(this.retryTimer);
    this.retryTimer = null;
    const socket = this.socket;
    this.socket = null;
    socket?.close();
  }

  private async connect(generation: number): Promise<void> {
    const attempt = await this.attempt(generation);
    if (generation !== this.generation) return;
    if (attempt.outcome === 'ready') {
      this.failures = 0;
      return;
    }
    if (attempt.outcome === 'incompatible') {
      this.setStatus('incompatible', attempt.detail);
      return;
    }
    this.failures += 1;
    const unavailable = this.failures >= ATTEMPTS_BEFORE_UNAVAILABLE;
    if (unavailable) {
      this.setStatus('unavailable', attempt.detail);
    } else if (this.status !== 'connecting') {
      this.setStatus('disconnected', attempt.detail);
    }
    const delay = unavailable ? UNAVAILABLE_RETRY_DELAY_MS : RETRY_DELAY_MS;
    this.retryTimer = setTimeout(() => void this.connect(generation), delay);
  }

  private async attempt(generation: number): Promise<Attempt> {
    let connection = await loadDaemonConnection();
    let attempt: Attempt =
      connection === null
        ? { outcome: 'failed', detail: 'daemon.json not found' }
        : await this.open(connection, generation);
    if (attempt.outcome !== 'failed' || !canManageDaemon) return attempt;

    // Nothing answered: start the bundled daemon (a no-op if one is running).
    try {
      await startDaemon();
    } catch (error) {
      return { outcome: 'failed', detail: describe(error) };
    }
    if (generation !== this.generation) return attempt;
    connection = await loadDaemonConnection();
    if (connection === null) return attempt;
    attempt = await this.open(connection, generation);
    return attempt;
  }

  /** Opens a socket and resolves once the handshake succeeds or fails. */
  private open(connection: DaemonConnection, generation: number): Promise<Attempt> {
    return new Promise((resolve) => {
      const address = `127.0.0.1:${connection.port}`;
      const socket = new WebSocket(`ws://${address}`);
      let settled = false;
      const settle = (attempt: Attempt) => {
        if (settled) return;
        settled = true;
        resolve(attempt);
      };

      socket.onopen = () => {
        socket.send(
          JSON.stringify({ type: 'hello', token: connection.token, protocolVersion: PROTOCOL_VERSION }),
        );
      };
      socket.onmessage = (event: MessageEvent<string>) => {
        const message = parseMessage(event.data);
        if (message === null) return;
        if (!settled) {
          const attempt = handshakeResult(message);
          if (attempt.outcome !== 'ready' || generation !== this.generation) {
            socket.close();
            settle(attempt);
            return;
          }
          this.socket = socket;
          this.setStatus('ready', null);
          settle(attempt);
        }
        for (const listener of this.messageListeners) listener(message);
      };
      socket.onclose = () => {
        settle({ outcome: 'failed', detail: `could not connect to ${address}` });
        if (this.socket !== socket) return;
        // An established connection dropped: start over.
        this.socket = null;
        this.setStatus('disconnected', null);
        this.failures = 0;
        this.retryTimer = setTimeout(() => void this.connect(generation), RETRY_DELAY_MS);
      };
    });
  }

  private setStatus(status: ConnectionStatus, detail: string | null): void {
    if (status === this.status && status !== 'unavailable') return;
    this.status = status;
    for (const listener of this.statusListeners) listener(status, detail);
  }
}

function handshakeResult(message: ServerMessage): Attempt {
  if (message.type === 'ready') return { outcome: 'ready' };
  if (message.type === 'error' && message.code === 'unsupported_protocol') {
    return { outcome: 'incompatible', detail: message.message };
  }
  const detail = message.type === 'error' ? message.message : `unexpected ${message.type}`;
  return { outcome: 'failed', detail };
}

function describe(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function parseMessage(raw: string): ServerMessage | null {
  try {
    const value: unknown = JSON.parse(raw);
    if (typeof value === 'object' && value !== null && 'type' in value) {
      return value as ServerMessage;
    }
  } catch {
    // Ignore malformed frames; the daemon only sends JSON.
  }
  return null;
}
