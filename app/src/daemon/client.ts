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
  private stopped = true;
  private readonly messageListeners = new Set<MessageListener>();
  private readonly statusListeners = new Set<StatusListener>();

  start(): void {
    this.stopped = false;
    void this.connect();
  }

  stop(): void {
    this.stopped = true;
    if (this.retryTimer !== null) clearTimeout(this.retryTimer);
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

  private async connect(): Promise<void> {
    this.setStatus('connecting');
    const connection = await loadDaemonConnection();
    if (this.stopped) return;
    if (connection === null) {
      this.retry();
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
      this.retry();
    };
  }

  private retry(): void {
    this.setStatus('disconnected');
    if (this.stopped) return;
    this.retryTimer = setTimeout(() => void this.connect(), RETRY_DELAY_MS);
  }

  private setStatus(status: ConnectionStatus): void {
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
