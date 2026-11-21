import type { ErrorCode, SessionMeta, ServerMessage } from '@cohors/protocol';
import type { ConnectionStatus } from '../daemon/client.ts';

export interface SessionsState {
  connection: ConnectionStatus;
  /** Unknown until the daemon handshake completes. */
  claudeAvailable: boolean | null;
  sessions: SessionMeta[];
  selectedId: string | null;
  newSessionOpen: boolean;
  creating: boolean;
  createError: string | null;
}

export const initialSessionsState: SessionsState = {
  connection: 'connecting',
  claudeAvailable: null,
  sessions: [],
  selectedId: null,
  newSessionOpen: false,
  creating: false,
  createError: null,
};

const CREATE_ERROR_MESSAGES: Partial<Record<ErrorCode, string>> = {
  invalid_cwd: 'Essa pasta não existe. Escolha outra.',
  agent_not_found: 'Claude Code não encontrado. Instale o CLI ou escolha Shell.',
};
const CREATE_ERROR_FALLBACK = 'Não foi possível criar a sessão.';

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
    case 'error':
      if (message.ref !== 'session.create') return state;
      return {
        ...state,
        creating: false,
        createError: CREATE_ERROR_MESSAGES[message.code] ?? CREATE_ERROR_FALLBACK,
      };
    case 'session.snapshot':
    case 'session.output':
      return state;
  }
}

export function applyConnectionStatus(
  state: SessionsState,
  connection: ConnectionStatus,
): SessionsState {
  // A create in flight is lost with the connection.
  const creating = connection === 'ready' ? state.creating : false;
  return { ...state, connection, creating };
}

function sessionCreated(state: SessionsState, session: SessionMeta): SessionsState {
  const exists = state.sessions.some((existing) => existing.id === session.id);
  const sessions = exists ? state.sessions : [...state.sessions, session];
  if (!state.creating) return { ...state, sessions };
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
