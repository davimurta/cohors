import { create } from 'zustand';
import type { Agent } from '@cohors/protocol';
import { DaemonClient } from '../daemon/client.ts';
import {
  applyConnectionStatus,
  applyServerMessage,
  initialSessionsState,
  type SessionsState,
} from './sessions.ts';

export const daemon = new DaemonClient();

export interface CreateSessionRequest {
  agent: Agent;
  cwd: string;
  name: string;
}

interface SessionsActions {
  select: (id: string) => void;
  openNewSession: () => void;
  closeNewSession: () => void;
  createSession: (request: CreateSessionRequest) => void;
}

export const useAppStore = create<SessionsState & SessionsActions>()((set, get) => ({
  ...initialSessionsState,
  select: (id) => set({ selectedId: id }),
  openNewSession: () => set({ newSessionOpen: true, createError: null }),
  closeNewSession: () => set({ newSessionOpen: false, creating: false, createError: null }),
  createSession: ({ agent, cwd, name }) => {
    if (get().connection !== 'ready') {
      set({ createError: 'O daemon está desconectado.' });
      return;
    }
    set({ creating: true, createError: null });
    daemon.send({ type: 'session.create', agent, cwd, name });
  },
}));

/** Mirrors daemon events into the store. Returns a function that stops it. */
export function startDaemonSync(): () => void {
  const stopMessages = daemon.onMessage((message) => {
    if (message.type === 'ready') daemon.send({ type: 'session.list' });
    useAppStore.setState((state) => applyServerMessage(state, message));
  });
  const stopStatus = daemon.onStatus((status) => {
    useAppStore.setState((state) => applyConnectionStatus(state, status));
  });
  daemon.start();
  return () => {
    stopMessages();
    stopStatus();
    daemon.stop();
  };
}
