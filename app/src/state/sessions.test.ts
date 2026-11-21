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
    createdAt: '2026-01-01T00:00:00Z',
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

describe('applyConnectionStatus', () => {
  it('cancels a pending creation when the connection drops', () => {
    const state = applyConnectionStatus(withSessions([], { creating: true }), 'disconnected');
    expect(state.connection).toBe('disconnected');
    expect(state.creating).toBe(false);
  });
});
