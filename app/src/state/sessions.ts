import type {
  ClaudeAccount,
  ErrorCode,
  Project,
  ServerMessage,
  SessionMeta,
  SshKey,
} from '@cohors/protocol';
import type { ConnectionStatus } from '../daemon/client.ts';
import { isLive } from '../session/status.ts';

export interface SessionsState {
  connection: ConnectionStatus;
  /** Technical reason shown on the daemon error screens. */
  connectionDetail: string | null;
  /** Unknown until the daemon handshake completes. */
  claudeAvailable: boolean | null;
  sessions: SessionMeta[];
  selectedId: string | null;
  projects: Project[];
  claudeAccounts: ClaudeAccount[];
  sshKeys: SshKey[];
  newSessionOpen: boolean;
  /** Project preselected when the dialog was opened from a project. */
  newSessionProjectId: string | null;
  creating: boolean;
  createError: string | null;
  /** Session whose resume request is waiting for the daemon. */
  resumingId: string | null;
  resumeError: string | null;
  /** Open project dialog: null for a new project, an id to edit one. */
  projectDialog: { projectId: string | null } | null;
  savingProject: boolean;
  projectError: string | null;
}

export const initialSessionsState: SessionsState = {
  connection: 'connecting',
  connectionDetail: null,
  claudeAvailable: null,
  sessions: [],
  selectedId: null,
  projects: [],
  claudeAccounts: [],
  sshKeys: [],
  newSessionOpen: false,
  newSessionProjectId: null,
  creating: false,
  createError: null,
  resumingId: null,
  resumeError: null,
  projectDialog: null,
  savingProject: false,
  projectError: null,
};

const CREATE_ERROR_MESSAGES: Partial<Record<ErrorCode, string>> = {
  invalid_cwd: 'Essa pasta não existe. Escolha outra.',
  agent_not_found: 'Claude Code não encontrado. Instale o CLI ou escolha Shell.',
};
const CREATE_ERROR_FALLBACK = 'Não foi possível criar a sessão.';

const RESUME_ERROR_MESSAGES: Partial<Record<ErrorCode, string>> = {
  invalid_cwd: 'A pasta desta sessão não existe mais.',
  agent_not_found: 'Claude Code não encontrado. Instale o CLI para retomar esta sessão.',
};
const RESUME_ERROR_FALLBACK = 'Não foi possível retomar a sessão.';

const PROJECT_ERROR_MESSAGES: Partial<Record<ErrorCode, string>> = {
  invalid_cwd: 'A pasta raiz não existe. Escolha outra.',
  project_not_found: 'Este projeto não existe mais.',
};
const PROJECT_ERROR_FALLBACK = 'Não foi possível salvar o projeto.';

/** Applies a daemon message to the state. Pure, so it can be tested alone. */
export function applyServerMessage(state: SessionsState, message: ServerMessage): SessionsState {
  switch (message.type) {
    case 'ready':
      return { ...state, claudeAvailable: message.claudeAvailable };
    case 'session.list.result':
      return {
        ...state,
        sessions: message.sessions,
        selectedId: keepSelection(state.selectedId, message.sessions),
      };
    case 'session.created':
      return sessionCreated(state, message.session);
    case 'session.updated':
      return sessionUpdated(state, message.session);
    case 'accounts.list.result':
      return { ...state, claudeAccounts: message.claudeAccounts, sshKeys: message.sshKeys };
    case 'project.list.result':
      // A save this client asked for succeeded: close its dialog.
      if (!state.savingProject) return { ...state, projects: message.projects };
      return {
        ...state,
        projects: message.projects,
        savingProject: false,
        projectDialog: null,
        projectError: null,
      };
    case 'error':
      return requestFailed(state, message.code, message.ref);
    case 'session.snapshot':
    case 'session.output':
      return state;
  }
}

export function applyConnectionStatus(
  state: SessionsState,
  connection: ConnectionStatus,
  connectionDetail: string | null,
): SessionsState {
  if (connection === 'ready') return { ...state, connection, connectionDetail };
  // Requests in flight are lost with the connection.
  return {
    ...state,
    connection,
    connectionDetail,
    creating: false,
    resumingId: null,
    savingProject: false,
  };
}

function sessionUpdated(state: SessionsState, updated: SessionMeta): SessionsState {
  const sessions = state.sessions.map((session) => (session.id === updated.id ? updated : session));
  if (state.resumingId !== updated.id || !isLive(updated.status)) {
    return { ...state, sessions };
  }
  return { ...state, sessions, resumingId: null, resumeError: null };
}

function requestFailed(state: SessionsState, code: ErrorCode, ref?: string): SessionsState {
  if (ref === 'session.create') {
    return {
      ...state,
      creating: false,
      createError: CREATE_ERROR_MESSAGES[code] ?? CREATE_ERROR_FALLBACK,
    };
  }
  if (state.savingProject && (ref === 'project.create' || ref === state.projectDialog?.projectId)) {
    return {
      ...state,
      savingProject: false,
      projectError: PROJECT_ERROR_MESSAGES[code] ?? PROJECT_ERROR_FALLBACK,
    };
  }
  if (ref !== undefined && ref === state.resumingId) {
    return {
      ...state,
      resumingId: null,
      resumeError: RESUME_ERROR_MESSAGES[code] ?? RESUME_ERROR_FALLBACK,
    };
  }
  return state;
}

function sessionCreated(state: SessionsState, session: SessionMeta): SessionsState {
  const exists = state.sessions.some((existing) => existing.id === session.id);
  const sessions = exists ? state.sessions : [...state.sessions, session];
  if (!state.creating) return { ...state, sessions, selectedId: state.selectedId ?? session.id };
  // The creation this client asked for: open it and close the dialog.
  return {
    ...state,
    sessions,
    selectedId: session.id,
    creating: false,
    newSessionOpen: false,
    createError: null,
  };
}

function keepSelection(selectedId: string | null, sessions: SessionMeta[]): string | null {
  if (selectedId !== null && sessions.some((session) => session.id === selectedId)) {
    return selectedId;
  }
  return sessions[0]?.id ?? null;
}
