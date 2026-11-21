// Mirror of daemon/src/protocol.rs. Keep both in sync.

export const PROTOCOL_VERSION = 1;

export type Agent = 'claude' | 'shell';

/** `exited`: the process ended on its own. `interrupted`: the daemon lost it (reboot, crash). */
export type SessionStatus = 'running' | 'exited' | 'interrupted';

export interface SessionMeta {
  id: string;
  name: string;
  agent: Agent;
  cwd: string;
  agentSessionId: string | null;
  status: SessionStatus;
  exitCode: number | null;
  /** ISO 8601 timestamps. */
  createdAt: string;
  endedAt: string | null;
}

export type ErrorCode =
  | 'unauthorized'
  | 'unsupported_protocol'
  | 'bad_request'
  | 'session_not_found'
  | 'invalid_cwd'
  | 'agent_not_found'
  | 'invalid_state'
  | 'internal';

export type ClientMessage =
  | { type: 'hello'; token: string; protocolVersion: number }
  | { type: 'session.list' }
  | { type: 'session.create'; agent: Agent; cwd: string; name?: string }
  | { type: 'session.attach'; id: string }
  | { type: 'session.resume'; id: string }
  | { type: 'session.detach'; id: string }
  | { type: 'session.input'; id: string; data: string }
  | { type: 'session.resize'; id: string; cols: number; rows: number };

export type ServerMessage =
  | { type: 'ready'; protocolVersion: number; daemonVersion: string; claudeAvailable: boolean }
  | { type: 'session.list.result'; sessions: SessionMeta[] }
  | { type: 'session.created'; session: SessionMeta }
  | { type: 'session.updated'; session: SessionMeta }
  | { type: 'session.snapshot'; id: string; seq: number; data: string }
  | { type: 'session.output'; id: string; seq: number; data: string }
  | { type: 'error'; code: ErrorCode; message: string; ref?: string };
