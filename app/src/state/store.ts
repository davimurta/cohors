import { create } from 'zustand';
import type { Agent, ProjectSettings } from '@cohors/protocol';
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
  projectId: string | null;
}

interface SessionsActions {
  select: (id: string) => void;
  openNewSession: (projectId?: string) => void;
  closeNewSession: () => void;
  createSession: (request: CreateSessionRequest) => void;
  resumeSession: (id: string) => void;
  retryConnection: () => void;
  restartDaemon: () => void;
  /** "Encerrar tudo": the daemon stops every session and exits. */
  shutdownDaemon: () => void;
  openProjectDialog: (projectId: string | null) => void;
  closeProjectDialog: () => void;
  saveProject: (settings: ProjectSettings) => void;
  deleteProject: (id: string) => void;
}

export const useAppStore = create<SessionsState & SessionsActions>()((set, get) => ({
  ...initialSessionsState,
  select: (id) => set({ selectedId: id, resumeError: null }),
  openNewSession: (projectId) =>
    set({ newSessionOpen: true, newSessionProjectId: projectId ?? null, createError: null }),
  closeNewSession: () => set({ newSessionOpen: false, creating: false, createError: null }),
  createSession: ({ agent, cwd, name, projectId }) => {
    if (get().connection !== 'ready') {
      set({ createError: 'O daemon está desconectado.' });
      return;
    }
    set({ creating: true, createError: null });
    daemon.send({
      type: 'session.create',
      agent,
      cwd,
      name,
      ...(projectId === null ? {} : { projectId }),
    });
  },
  resumeSession: (id) => {
    if (get().connection !== 'ready') {
      set({ resumeError: 'O daemon está desconectado.' });
      return;
    }
    set({ resumingId: id, resumeError: null });
    daemon.send({ type: 'session.resume', id });
  },
  retryConnection: () => daemon.retryNow(),
  restartDaemon: () => void daemon.restartDaemon(),
  shutdownDaemon: () => daemon.send({ type: 'daemon.shutdown' }),
  openProjectDialog: (projectId) => {
    // Accounts and keys may have changed since the app started.
    daemon.send({ type: 'accounts.list' });
    set({ projectDialog: { projectId }, projectError: null });
  },
  closeProjectDialog: () => set({ projectDialog: null, savingProject: false, projectError: null }),
  saveProject: (project) => {
    const projectId = get().projectDialog?.projectId ?? null;
    set({ savingProject: true, projectError: null });
    daemon.send(
      projectId === null
        ? { type: 'project.create', project }
        : { type: 'project.update', id: projectId, project },
    );
  },
  deleteProject: (id) => {
    set({ projectDialog: null });
    daemon.send({ type: 'project.delete', id });
  },
}));

/** Mirrors daemon events into the store. Returns a function that stops it. */
export function startDaemonSync(): () => void {
  const stopMessages = daemon.onMessage((message) => {
    if (message.type === 'ready') {
      daemon.send({ type: 'session.list' });
      daemon.send({ type: 'project.list' });
      daemon.send({ type: 'accounts.list' });
    }
    useAppStore.setState((state) => applyServerMessage(state, message));
  });
  const stopStatus = daemon.onStatus((status, detail) => {
    useAppStore.setState((state) => applyConnectionStatus(state, status, detail));
  });
  daemon.start();
  return () => {
    stopMessages();
    stopStatus();
    daemon.stop();
  };
}
