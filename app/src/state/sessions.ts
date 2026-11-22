import type {
  ClaudeAccount,
  ConversationEntry,
  ErrorCode,
  Project,
  ServerMessage,
  SessionMeta,
  SshKey,
  TokenUsage,
} from '@cohors/protocol';
import type { ConnectionStatus } from '../daemon/client.ts';
import { isLive } from '../session/status.ts';
import { pruneLayout } from './layout.ts';

export type SidebarTab = 'active' | 'finished';

export interface TranscriptView {
  id: string;
  /** null while loading. */
  data: string | null;
  truncated: boolean;
}

/** Claude conversation of a session; both null when Claude kept no log. */
export interface Conversation {
  entries: ConversationEntry[] | null;
  usage: TokenUsage | null;
}

export interface HistorySearch {
  query: string;
  /** Matching finished sessions; null until the first answer arrives. */
  ids: string[] | null;
}

export interface ContextEditor {
  projectId: string;
  /** null until the daemon sends the files. */
  files: { dir: string; rules: string; plan: string } | null;
  saving: boolean;
  error: string | null;
}

export interface SessionsState {
  connection: ConnectionStatus;
  /** Technical reason shown on the daemon error screens. */
  connectionDetail: string | null;
  /** Unknown until the daemon handshake completes. */
  claudeAvailable: boolean | null;
  sessions: SessionMeta[];
  /** The focused session; always one of `panes` when any is shown. */
  selectedId: string | null;
  /** Sessions shown together in the main area, in order. */
  panes: string[];
  sidebarTab: SidebarTab;
  /** Summary text by session id, loaded when a finished session is opened. */
  summaries: Record<string, string | null>;
  transcript: TranscriptView | null;
  /** Conversations by session id, loaded on demand. */
  conversations: Record<string, Conversation>;
  /** Search in the finished sessions; null when the box is empty. */
  historySearch: HistorySearch | null;
  /** Session whose notes dialog is open. */
  notesId: string | null;
  /** Session whose rename dialog is open. */
  renameId: string | null;
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
  /** Open editor of a project's rules and plan. */
  contextEditor: ContextEditor | null;
}

export const initialSessionsState: SessionsState = {
  connection: 'connecting',
  connectionDetail: null,
  claudeAvailable: null,
  sessions: [],
  selectedId: null,
  panes: [],
  sidebarTab: 'active',
  summaries: {},
  transcript: null,
  conversations: {},
  historySearch: null,
  notesId: null,
  renameId: null,
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
  contextEditor: null,
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
  invalid_cwd: 'A pasta escolhida não existe. Escolha outra.',
  project_not_found: 'Este projeto não existe mais.',
};
const PROJECT_ERROR_FALLBACK = 'Não foi possível salvar o projeto.';

const CONTEXT_READ_ERROR = 'Não foi possível ler as regras e o plano.';
const CONTEXT_SAVE_ERROR = 'Não foi possível salvar as regras e o plano.';
const PROJECT_GONE_ERROR = 'Este projeto não existe mais.';

/** Applies a daemon message to the state. Pure, so it can be tested alone. */
export function applyServerMessage(state: SessionsState, message: ServerMessage): SessionsState {
  return syncLayout(reduce(state, message));
}

/** Keeps the panes in step with the selection and the sessions that are still live. */
export function syncLayout(state: SessionsState): SessionsState {
  const { selectedId, panes } = state;
  let layout = { panes, selectedId };
  if (selectedId === null) {
    if (panes.length > 0) layout = { panes: [], selectedId };
  } else if (!panes.includes(selectedId)) {
    layout = { panes: [selectedId], selectedId };
  }
  layout = pruneLayout(layout, state.sessions);
  if (layout.panes === panes && layout.selectedId === selectedId) return state;
  return { ...state, panes: layout.panes, selectedId: layout.selectedId };
}

function reduce(state: SessionsState, message: ServerMessage): SessionsState {
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
    case 'session.summary':
      return { ...state, summaries: { ...state.summaries, [message.id]: message.text } };
    case 'session.deleted':
      return sessionDeleted(state, message.id);
    case 'session.transcript.result':
      if (state.transcript?.id !== message.id) return state;
      return {
        ...state,
        transcript: { id: message.id, data: message.data, truncated: message.truncated },
      };
    case 'session.conversation.result':
      return {
        ...state,
        conversations: {
          ...state.conversations,
          [message.id]: { entries: message.entries, usage: message.usage },
        },
      };
    case 'session.search.result':
      // Answers to an older query are dropped.
      if (state.historySearch?.query !== message.query) return state;
      return { ...state, historySearch: { query: message.query, ids: message.ids } };
    case 'accounts.list.result':
      return { ...state, claudeAccounts: message.claudeAccounts, sshKeys: message.sshKeys };
    case 'project.list.result': {
      const projects = message.projects;
      const editor = state.contextEditor;
      const contextEditor =
        editor !== null && projects.some((project) => project.id === editor.projectId)
          ? editor
          : null;
      // A save this client asked for succeeded: close its dialog.
      if (!state.savingProject) return { ...state, projects, contextEditor };
      return {
        ...state,
        projects,
        contextEditor,
        savingProject: false,
        projectDialog: null,
        projectError: null,
      };
    }
    case 'project.context.result': {
      const editor = state.contextEditor;
      if (editor?.projectId !== message.id) return state;
      // A save went through: the editor closes.
      if (editor.saving) return { ...state, contextEditor: null };
      const { dir, rules, plan } = message;
      return { ...state, contextEditor: { ...editor, files: { dir, rules, plan } } };
    }
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
    contextEditor: null,
  };
}

function sessionDeleted(state: SessionsState, id: string): SessionsState {
  const sessions = state.sessions.filter((session) => session.id !== id);
  const { [id]: _removed, ...summaries } = state.summaries;
  const { [id]: _conversation, ...conversations } = state.conversations;
  return {
    ...state,
    sessions,
    summaries,
    conversations,
    selectedId:
      state.selectedId === id ? firstInTab(sessions, state.sidebarTab) : state.selectedId,
    transcript: state.transcript?.id === id ? null : state.transcript,
    notesId: state.notesId === id ? null : state.notesId,
    renameId: state.renameId === id ? null : state.renameId,
  };
}

export function inTab(session: SessionMeta, tab: SidebarTab): boolean {
  return (session.status === 'finished') === (tab === 'finished');
}

export function firstInTab(sessions: SessionMeta[], tab: SidebarTab): string | null {
  return sessions.find((session) => inTab(session, tab))?.id ?? null;
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
  const editor = state.contextEditor;
  if (editor !== null && ref === editor.projectId) {
    const reading = editor.files === null;
    const error =
      code === 'project_not_found'
        ? PROJECT_GONE_ERROR
        : reading
          ? CONTEXT_READ_ERROR
          : CONTEXT_SAVE_ERROR;
    return { ...state, contextEditor: { ...editor, saving: false, error } };
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
  return firstInTab(sessions, 'active');
}
