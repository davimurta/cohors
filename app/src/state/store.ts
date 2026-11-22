import { create } from 'zustand';
import type { Agent, ProjectSettings } from '@cohors/protocol';
import { DaemonClient } from '../daemon/client.ts';
import { closePane, selectSession, toggleSplit } from './layout.ts';
import {
  applyConnectionStatus,
  applyServerMessage,
  firstInTab,
  inTab,
  initialSessionsState,
  syncLayout,
  type SessionsState,
  type SidebarTab,
} from './sessions.ts';
import { isLive } from '../session/status.ts';

export const daemon = new DaemonClient();

export interface CreateSessionRequest {
  agent: Agent;
  cwd: string;
  name: string;
  projectId: string | null;
}

interface SessionsActions {
  select: (id: string) => void;
  /** Shows a live session next to the current ones, or takes it out. */
  toggleSplit: (id: string) => void;
  closePane: (id: string) => void;
  openNewSession: (projectId?: string) => void;
  closeNewSession: () => void;
  createSession: (request: CreateSessionRequest) => void;
  resumeSession: (id: string) => void;
  retryConnection: () => void;
  restartDaemon: () => void;
  /** "Encerrar tudo": the daemon stops every session and exits. */
  shutdownDaemon: () => void;
  setSidebarTab: (tab: SidebarTab) => void;
  finishSession: (id: string, withSummary: boolean) => void;
  retrySummary: (id: string) => void;
  skipSummary: (id: string) => void;
  loadSummary: (id: string) => void;
  openNotes: (id: string) => void;
  closeNotes: () => void;
  saveNotes: (id: string, notes: string) => void;
  openRename: (id: string) => void;
  closeRename: () => void;
  renameSession: (id: string, name: string) => void;
  openTranscript: (id: string) => void;
  closeTranscript: () => void;
  deleteSession: (id: string) => void;
  openProjectDialog: (projectId: string | null) => void;
  closeProjectDialog: () => void;
  saveProject: (settings: ProjectSettings) => void;
  deleteProject: (id: string) => void;
  openContext: (projectId: string) => void;
  closeContext: () => void;
  saveContext: (rules: string, plan: string) => void;
}

export const useAppStore = create<SessionsState & SessionsActions>()((set, get) => ({
  ...initialSessionsState,
  select: (id) =>
    set((state) => {
      const session = state.sessions.find((candidate) => candidate.id === id);
      const live = session !== undefined && isLive(session.status);
      const layout = selectSession(state, id, live);
      return { ...layout, resumeError: null, transcript: null };
    }),
  toggleSplit: (id) => set((state) => ({ ...toggleSplit(state, id), transcript: null })),
  closePane: (id) => set((state) => closePane(state, id)),
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
  setSidebarTab: (tab) =>
    set((state) => {
      const selected = state.sessions.find((session) => session.id === state.selectedId);
      const keep = selected !== undefined && inTab(selected, tab);
      return syncLayout({
        ...state,
        sidebarTab: tab,
        selectedId: keep ? state.selectedId : firstInTab(state.sessions, tab),
        panes: keep ? state.panes : [],
        transcript: null,
      });
    }),
  finishSession: (id, withSummary) => {
    // The session moves to the history; follow it there.
    set({ sidebarTab: 'finished', selectedId: id, panes: [id], transcript: null });
    daemon.send({ type: 'session.finish', id, withSummary });
  },
  retrySummary: (id) => daemon.send({ type: 'session.summarize', id }),
  skipSummary: (id) => daemon.send({ type: 'session.skipSummary', id }),
  loadSummary: (id) => {
    if (!(id in get().summaries)) daemon.send({ type: 'session.getSummary', id });
  },
  openRename: (id) => set({ renameId: id }),
  closeRename: () => set({ renameId: null }),
  renameSession: (id, name) => {
    daemon.send({ type: 'session.rename', id, name });
    set({ renameId: null });
  },
  openNotes: (id) => set({ notesId: id }),
  closeNotes: () => set({ notesId: null }),
  saveNotes: (id, notes) => {
    daemon.send({ type: 'session.notes', id, notes });
    set({ notesId: null });
  },
  openTranscript: (id) => {
    set({ transcript: { id, data: null, truncated: false } });
    daemon.send({ type: 'session.transcript', id });
  },
  closeTranscript: () => set({ transcript: null }),
  deleteSession: (id) => daemon.send({ type: 'session.delete', id }),
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
  openContext: (projectId) => {
    set({ contextEditor: { projectId, files: null, saving: false, error: null } });
    daemon.send({ type: 'project.context.get', id: projectId });
  },
  closeContext: () => set({ contextEditor: null }),
  saveContext: (rules, plan) => {
    const editor = get().contextEditor;
    if (editor === null || editor.saving) return;
    set({ contextEditor: { ...editor, saving: true, error: null } });
    daemon.send({ type: 'project.context.save', id: editor.projectId, rules, plan });
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
