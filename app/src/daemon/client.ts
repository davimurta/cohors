import {
  PROTOCOL_VERSION,
  type ClientMessage,
  type ServerMessage,
} from '@cohors/protocol';
import { loadDaemonConnection } from './connection.ts';

const RETRY_DELAY_MS = 1000;

export type ConnectionStatus = 'connecting' | 'ready' | 'disconnected';

type MessageListener = (message: ServerMessage) => void;
type StatusListener = (status: ConnectionStatus) => void;

/** Single entry point to the daemon: connects, authenticates and reconnects. */
export class DaemonClient {
  private socket: WebSocket | null = null;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private status: ConnectionStatus = 'connecting';
  /** Bumped on start and stop so stale connection attempts give up. */
  private generation = 0;
  private readonly messageListeners = new Set<MessageListener>();
  private readonly statusListeners = new Set<StatusListener>();

  start(): void {
    this.generation += 1;
    void this.connect(this.generation);
  }

  stop(): void {
    this.generation += 1;
    if (this.retryTimer !== null) clearTimeout(this.retryTimer);
    this.retryTimer = null;
    this.socket?.close();
    this.socket = null;
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

  private async connect(generation: number): Promise<void> {
    // Retries stay "disconnected" instead of flickering back to "connecting".
    if (this.status !== 'disconnected') this.setStatus('connecting');
    const connection = await loadDaemonConnection();
    if (generation !== this.generation) return;
    if (connection === null) {
      this.retry(generation);
      return;
    }

    const socket = new WebSocket(`ws://127.0.0.1:${connection.port}`);
    this.socket = socket;
    socket.onopen = () => {
      this.send({ type: 'hello', token: connection.token, protocolVersion: PROTOCOL_VERSION });
    };
    socket.onmessage = (event: MessageEvent<string>) => {
      const message = parseMessage(event.data);
      if (message === null) return;
      if (message.type === 'ready') this.setStatus('ready');
      for (const listener of this.messageListeners) listener(message);
    };
    socket.onclose = () => {
      if (this.socket !== socket) return;
      this.socket = null;
      this.retry(generation);
    };
  }

  private retry(generation: number): void {
    if (generation !== this.generation) return;
    this.setStatus('disconnected');
    this.retryTimer = setTimeout(() => void this.connect(generation), RETRY_DELAY_MS);
  }

  private setStatus(status: ConnectionStatus): void {
    if (status === this.status) return;
    this.status = status;
    for (const listener of this.statusListeners) listener(status);
  }
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
