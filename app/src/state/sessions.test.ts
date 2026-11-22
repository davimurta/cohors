import { describe, expect, it } from 'vitest';
import type { SessionMeta } from '@cohors/protocol';
import {
  applyConnectionStatus,
  applyServerMessage,
  initialSessionsState,
  type SessionsState,
} from './sessions.ts';

function meta(id: string): SessionMeta {
  return {
    id,
    name: id,
    agent: 'shell',
    cwd: '/tmp',
    agentSessionId: null,
    status: 'running',
    exitCode: null,
    createdAt: '2026-01-01T00:00:00Z',
    endedAt: null,
    idleSince: null,
    projectId: null,
    claudeConfigDir: null,
    sshKey: null,
    contextDir: null,
    attention: null,
    notes: '',
    summaryStatus: 'none',
  };
}

const withSessions = (ids: string[], overrides: Partial<SessionsState> = {}): SessionsState => ({
  ...initialSessionsState,
  sessions: ids.map(meta),
  ...overrides,
});

describe('applyServerMessage', () => {
  it('records claude availability on ready', () => {
    const state = applyServerMessage(initialSessionsState, {
      type: 'ready',
      protocolVersion: 1,
      daemonVersion: '0.1.0',
      claudeAvailable: false,
    });
    expect(state.claudeAvailable).toBe(false);
  });

  it('selects the first session when nothing is selected', () => {
    const state = applyServerMessage(initialSessionsState, {
      type: 'session.list.result',
      sessions: [meta('a'), meta('b')],
    });
    expect(state.selectedId).toBe('a');
  });

  it('keeps the current selection when it still exists', () => {
    const state = applyServerMessage(withSessions(['a', 'b'], { selectedId: 'b' }), {
      type: 'session.list.result',
      sessions: [meta('a'), meta('b')],
    });
    expect(state.selectedId).toBe('b');
  });

  it('drops a selection that no longer exists', () => {
    const state = applyServerMessage(withSessions(['a'], { selectedId: 'gone' }), {
      type: 'session.list.result',
      sessions: [],
    });
    expect(state.selectedId).toBeNull();
  });

  it('opens a session this client created and closes the dialog', () => {
    const state = applyServerMessage(
      withSessions(['a'], { selectedId: 'a', creating: true, newSessionOpen: true }),
      { type: 'session.created', session: meta('b') },
    );
    expect(state.sessions.map((session) => session.id)).toEqual(['a', 'b']);
    expect(state.selectedId).toBe('b');
    expect(state.newSessionOpen).toBe(false);
    expect(state.creating).toBe(false);
  });

  it('adds a session created elsewhere without changing selection', () => {
    const state = applyServerMessage(withSessions(['a'], { selectedId: 'a' }), {
      type: 'session.created',
      session: meta('b'),
    });
    expect(state.sessions).toHaveLength(2);
    expect(state.selectedId).toBe('a');
  });

  it('selects a session created elsewhere when nothing is selected', () => {
    const state = applyServerMessage(initialSessionsState, {
      type: 'session.created',
      session: meta('a'),
    });
    expect(state.selectedId).toBe('a');
  });

  it('ignores a duplicate created event', () => {
    const state = applyServerMessage(withSessions(['a']), {
      type: 'session.created',
      session: meta('a'),
    });
    expect(state.sessions).toHaveLength(1);
  });

  it('replaces an updated session in place', () => {
    const state = applyServerMessage(withSessions(['a', 'b']), {
      type: 'session.updated',
      session: { ...meta('a'), status: 'exited', exitCode: 1 },
    });
    expect(state.sessions.map((session) => session.status)).toEqual(['exited', 'running']);
  });

  it('turns a create error into a readable message', () => {
    const state = applyServerMessage(withSessions([], { creating: true }), {
      type: 'error',
      code: 'invalid_cwd',
      message: 'folder does not exist',
      ref: 'session.create',
    });
    expect(state.creating).toBe(false);
    expect(state.createError).toBe('Essa pasta não existe. Escolha outra.');
  });

  it('ignores errors unrelated to creation', () => {
    const before = withSessions([], { creating: true });
    const after = applyServerMessage(before, {
      type: 'error',
      code: 'session_not_found',
      message: 'missing',
      ref: 'x',
    });
    expect(after).toBe(before);
  });
});

describe('resume', () => {
  const exited = { ...meta('a'), status: 'exited' as const, exitCode: 1 };

  it('clears the pending resume once the session runs again', () => {
    const state = applyServerMessage(
      { ...withSessions([]), sessions: [exited], resumingId: 'a' },
      { type: 'session.updated', session: { ...exited, status: 'running', exitCode: null } },
    );
    expect(state.resumingId).toBeNull();
    expect(state.sessions[0]?.status).toBe('running');
  });

  it('turns a resume error into a readable message', () => {
    const state = applyServerMessage(
      { ...withSessions([]), sessions: [exited], resumingId: 'a' },
      { type: 'error', code: 'invalid_cwd', message: 'folder does not exist', ref: 'a' },
    );
    expect(state.resumingId).toBeNull();
    expect(state.resumeError).toBe('A pasta desta sessão não existe mais.');
  });
});

describe('projects', () => {
  const project = {
    id: 'p',
    name: 'api',
    root: '/dev/api',
    claudeConfigDir: null,
    sshKey: null,
    contextDir: null,
  };

  it('closes the dialog once the save is confirmed', () => {
    const state = applyServerMessage(
      { ...withSessions([]), projectDialog: { projectId: null }, savingProject: true },
      { type: 'project.list.result', projects: [project] },
    );
    expect(state.projects).toEqual([project]);
    expect(state.projectDialog).toBeNull();
  });

  it('keeps the dialog open when another client changes projects', () => {
    const state = applyServerMessage(
      { ...withSessions([]), projectDialog: { projectId: null } },
      { type: 'project.list.result', projects: [project] },
    );
    expect(state.projectDialog).not.toBeNull();
  });

  it('shows a save error in the dialog', () => {
    const state = applyServerMessage(
      { ...withSessions([]), projectDialog: { projectId: 'p' }, savingProject: true },
      { type: 'error', code: 'invalid_cwd', message: 'folder does not exist', ref: 'p' },
    );
    expect(state.savingProject).toBe(false);
    expect(state.projectError).toBe('A pasta escolhida não existe. Escolha outra.');
  });
});

describe('project context', () => {
  const editor = { projectId: 'p', files: null, saving: false, error: null };
  const files = { dir: '/data/contexts/p', rules: 'Use tabs.', plan: '1. Login' };
  const result = { type: 'project.context.result', id: 'p', ...files } as const;

  it('fills the editor when the files arrive', () => {
    const state = applyServerMessage({ ...withSessions([]), contextEditor: editor }, result);
    expect(state.contextEditor?.files).toEqual(files);
  });

  it('ignores files of another project', () => {
    const state = applyServerMessage(
      { ...withSessions([]), contextEditor: editor },
      { ...result, id: 'other' },
    );
    expect(state.contextEditor?.files).toBeNull();
  });

  it('closes the editor after a save', () => {
    const saving = { ...editor, files, saving: true };
    const state = applyServerMessage({ ...withSessions([]), contextEditor: saving }, result);
    expect(state.contextEditor).toBeNull();
  });

  it('keeps the text and shows the error when a save fails', () => {
    const saving = { ...editor, files, saving: true };
    const state = applyServerMessage(
      { ...withSessions([]), contextEditor: saving },
      { type: 'error', code: 'internal', message: 'disk full', ref: 'p' },
    );
    expect(state.contextEditor).toEqual({
      ...saving,
      saving: false,
      error: 'Não foi possível salvar as regras e o plano.',
    });
  });

  it('closes the editor when its project is deleted', () => {
    const state = applyServerMessage(
      { ...withSessions([]), contextEditor: editor },
      { type: 'project.list.result', projects: [] },
    );
    expect(state.contextEditor).toBeNull();
  });
});

describe('history', () => {
  it('caches summaries by session', () => {
    const state = applyServerMessage(withSessions(['a']), {
      type: 'session.summary',
      id: 'a',
      status: 'done',
      text: '## O que foi feito',
    });
    expect(state.summaries).toEqual({ a: '## O que foi feito' });
  });

  it('selects the next finished session after a delete', () => {
    const finished = (id: string) => ({ ...meta(id), status: 'finished' as const });
    const state = applyServerMessage(
      {
        ...withSessions([]),
        sessions: [meta('live'), finished('a'), finished('b')],
        sidebarTab: 'finished',
        selectedId: 'a',
        summaries: { a: 'x' },
        transcript: { id: 'a', data: null, truncated: false },
      },
      { type: 'session.deleted', id: 'a' },
    );
    expect(state.sessions.map((session) => session.id)).toEqual(['live', 'b']);
    expect(state.selectedId).toBe('b');
    expect(state.summaries).toEqual({});
    expect(state.transcript).toBeNull();
  });

  it('only accepts the transcript that was asked for', () => {
    const asked = { ...withSessions(['a']), transcript: { id: 'a', data: null, truncated: false } };
    const loaded = applyServerMessage(asked, {
      type: 'session.transcript.result',
      id: 'a',
      data: 'log',
      truncated: true,
    });
    expect(loaded.transcript).toEqual({ id: 'a', data: 'log', truncated: true });
    const other = applyServerMessage(asked, {
      type: 'session.transcript.result',
      id: 'b',
      data: 'x',
      truncated: false,
    });
    expect(other.transcript?.data).toBeNull();
  });
});

describe('layout', () => {
  it('leaves a split when one of its sessions exits', () => {
    const state = applyServerMessage(
      { ...withSessions(['a', 'b', 'c']), panes: ['a', 'b', 'c'], selectedId: 'a' },
      { type: 'session.updated', session: { ...meta('b'), status: 'exited' } },
    );
    expect(state.panes).toEqual(['a', 'c']);
  });

  it('shows the first session alone on the first list', () => {
    const state = applyServerMessage(initialSessionsState, {
      type: 'session.list.result',
      sessions: [meta('a'), meta('b')],
    });
    expect(state.panes).toEqual(['a']);
  });
});

describe('applyConnectionStatus', () => {
  it('cancels a pending creation when the connection drops', () => {
    const state = applyConnectionStatus(
      withSessions([], { creating: true }),
      'disconnected',
      null,
    );
    expect(state.connection).toBe('disconnected');
    expect(state.creating).toBe(false);
  });
});

describe('history search and conversations', () => {
  it('keeps only the answer to the current query', () => {
    const searching = { ...withSessions([]), historySearch: { query: 'bill', ids: null } };
    const stale = applyServerMessage(searching, {
      type: 'session.search.result',
      query: 'bi',
      ids: ['x'],
    });
    expect(stale.historySearch?.ids).toBeNull();
    const current = applyServerMessage(searching, {
      type: 'session.search.result',
      query: 'bill',
      ids: ['a'],
    });
    expect(current.historySearch).toEqual({ query: 'bill', ids: ['a'] });
  });

  it('caches conversations and drops them with the session', () => {
    const loaded = applyServerMessage(withSessions(['a']), {
      type: 'session.conversation.result',
      id: 'a',
      entries: [],
      usage: null,
    });
    expect(loaded.conversations['a']).toEqual({ entries: [], usage: null });
    const deleted = applyServerMessage(loaded, { type: 'session.deleted', id: 'a' });
    expect(deleted.conversations).toEqual({});
  });
});

describe('restored layout', () => {
  const restored = {
    panes: ['a', 'b', 'gone'],
    selectedId: 'b',
    sidebarTab: 'active' as const,
    splitSizes: { column: 0.6, row: 0.5 },
  };

  it('brings back the split without sessions that are gone', () => {
    const state = applyServerMessage(
      { ...initialSessionsState, restoredLayout: restored },
      { type: 'session.list.result', sessions: [meta('a'), meta('b')] },
    );
    expect(state.panes).toEqual(['a', 'b']);
    expect(state.selectedId).toBe('b');
    expect(state.restoredLayout).toBeNull();
  });

  it('falls back to the first session when the focused one is gone', () => {
    const state = applyServerMessage(
      { ...initialSessionsState, restoredLayout: { ...restored, selectedId: 'gone' } },
      { type: 'session.list.result', sessions: [meta('a'), meta('b')] },
    );
    expect(state.selectedId).toBe('a');
    expect(state.panes).toEqual(['a']);
  });
});
