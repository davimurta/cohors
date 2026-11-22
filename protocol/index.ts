// Mirror of daemon/src/protocol.rs. Keep both in sync.

export const PROTOCOL_VERSION = 2;

export type Agent = 'claude' | 'shell';

/**
 * `idle`: running without output for a while. `exited`: the process ended on
 * its own. `interrupted`: the daemon lost it (reboot, crash, shutdown).
 * `finished`: ended by the user and moved to the history.
 */
export type SessionStatus = 'running' | 'idle' | 'exited' | 'interrupted' | 'finished';

export type SummaryStatus = 'none' | 'pending' | 'done' | 'failed';

/** Why a Claude session waits for the user: it finished an answer, or it asks for input or a permission. */
export type Attention = 'done' | 'input';

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
  /** Time of the last output while the session is idle. */
  idleSince: string | null;
  /** Project and environment captured when the session was created. */
  projectId: string | null;
  claudeConfigDir: string | null;
  sshKey: string | null;
  /** Project context folder Claude reads on start and resume. */
  contextDir: string | null;
  /** Set while a live Claude session waits for the user. */
  attention: Attention | null;
  notes: string;
  summaryStatus: SummaryStatus;
}

/** Editable part of a project. */
export interface ProjectSettings {
  name: string;
  root: string;
  /** Claude Code config folder (`CLAUDE_CONFIG_DIR`); null is the default account. */
  claudeConfigDir: string | null;
  /** Private key path (`GIT_SSH_COMMAND`); null keeps the system SSH setup. */
  sshKey: string | null;
  /** Folder with the rules and plan files; null keeps them in Cohors data. */
  contextDir: string | null;
}

export interface Project extends ProjectSettings {
  id: string;
}

export interface ClaudeAccount {
  /** null for the default account. */
  configDir: string | null;
  email: string | null;
  organization: string | null;
}

export interface SshKey {
  name: string;
  path: string;
}

export type ErrorCode =
  | 'unauthorized'
  | 'unsupported_protocol'
  | 'bad_request'
  | 'session_not_found'
  | 'project_not_found'
  | 'invalid_cwd'
  | 'agent_not_found'
  | 'invalid_state'
  | 'internal';

export type ClientMessage =
  | { type: 'hello'; token: string; protocolVersion: number }
  | { type: 'session.list' }
  | { type: 'session.create'; agent: Agent; cwd: string; name?: string; projectId?: string }
  | { type: 'session.attach'; id: string }
  | { type: 'session.resume'; id: string }
  | { type: 'session.detach'; id: string }
  | { type: 'session.input'; id: string; data: string }
  | { type: 'session.resize'; id: string; cols: number; rows: number }
  | { type: 'session.finish'; id: string; withSummary: boolean }
  | { type: 'session.summarize'; id: string }
  | { type: 'session.skipSummary'; id: string }
  | { type: 'session.getSummary'; id: string }
  | { type: 'session.notes'; id: string; notes: string }
  | { type: 'session.rename'; id: string; name: string }
  | { type: 'session.transcript'; id: string }
  | { type: 'session.delete'; id: string }
  | { type: 'daemon.shutdown' }
  | { type: 'accounts.list' }
  | { type: 'project.list' }
  | { type: 'project.create'; project: ProjectSettings }
  | { type: 'project.update'; id: string; project: ProjectSettings }
  | { type: 'project.delete'; id: string }
  | { type: 'project.context.get'; id: string }
  | { type: 'project.context.save'; id: string; rules: string; plan: string };

export type ServerMessage =
  | { type: 'ready'; protocolVersion: number; daemonVersion: string; claudeAvailable: boolean }
  | { type: 'session.list.result'; sessions: SessionMeta[] }
  | { type: 'session.created'; session: SessionMeta }
  | { type: 'session.updated'; session: SessionMeta }
  | { type: 'session.summary'; id: string; status: SummaryStatus; text: string | null }
  | { type: 'session.deleted'; id: string }
  | { type: 'session.transcript.result'; id: string; data: string; truncated: boolean }
  | { type: 'accounts.list.result'; claudeAccounts: ClaudeAccount[]; sshKeys: SshKey[] }
  | { type: 'project.list.result'; projects: Project[] }
  | { type: 'project.context.result'; id: string; dir: string; rules: string; plan: string }
  | { type: 'session.snapshot'; id: string; seq: number; data: string }
  | { type: 'session.output'; id: string; seq: number; data: string }
  | { type: 'error'; code: ErrorCode; message: string; ref?: string };
