// Mirror of daemon/src/protocol.rs. Keep both in sync.

export const PROTOCOL_VERSION = 1;

export type Agent = 'claude' | 'shell';

export interface SessionInfo {
  id: string;
  agent: Agent;
  cwd: string;
}

export type ErrorCode =
  | 'unauthorized'
  | 'unsupported_protocol'
  | 'bad_request'
  | 'session_not_found'
  | 'internal';

export type ClientMessage =
  | { type: 'hello'; token: string; protocolVersion: number }
  | { type: 'session.list' }
  | { type: 'session.attach'; id: string }
  | { type: 'session.detach'; id: string }
  | { type: 'session.input'; id: string; data: string }
  | { type: 'session.resize'; id: string; cols: number; rows: number };

export type ServerMessage =
  | { type: 'ready'; protocolVersion: number; daemonVersion: string }
  | { type: 'session.list.result'; sessions: SessionInfo[] }
  | { type: 'session.snapshot'; id: string; seq: number; data: string }
  | { type: 'session.output'; id: string; seq: number; data: string }
  | { type: 'error'; code: ErrorCode; message: string; ref?: string };
