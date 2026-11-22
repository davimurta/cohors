import { describe, expect, it } from 'vitest';
import type { SessionMeta, SessionStatus } from '@cohors/protocol';
import { MAX_PANES, closePane, pruneLayout, selectSession, toggleSplit } from './layout.ts';

const single = (id: string) => ({ panes: [id], selectedId: id });

const session = (id: string, status: SessionStatus): SessionMeta => ({
  id,
  name: id,
  agent: 'shell',
  cwd: '/tmp',
  agentSessionId: null,
  status,
  exitCode: null,
  createdAt: '2026-01-01T00:00:00Z',
  endedAt: null,
  idleSince: null,
  projectId: null,
  claudeConfigDir: null,
  sshKey: null,
  notes: '',
  summaryStatus: 'none',
});

describe('selectSession', () => {
  it('shows a session alone when not split', () => {
    expect(selectSession(single('a'), 'b', true)).toEqual(single('b'));
  });

  it('replaces the focused pane when split', () => {
    const layout = { panes: ['a', 'b'], selectedId: 'b' };
    expect(selectSession(layout, 'c', true)).toEqual({ panes: ['a', 'c'], selectedId: 'c' });
  });

  it('focuses a pane that is already shown', () => {
    const layout = { panes: ['a', 'b'], selectedId: 'b' };
    expect(selectSession(layout, 'a', true)).toEqual({ panes: ['a', 'b'], selectedId: 'a' });
  });

  it('leaves the split for a session without a terminal', () => {
    expect(selectSession({ panes: ['a', 'b'], selectedId: 'a' }, 'x', false)).toEqual(single('x'));
  });
});

describe('toggleSplit', () => {
  it('adds and focuses a pane', () => {
    expect(toggleSplit(single('a'), 'b')).toEqual({ panes: ['a', 'b'], selectedId: 'b' });
  });

  it('removes a pane that is shown', () => {
    expect(toggleSplit({ panes: ['a', 'b'], selectedId: 'b' }, 'b')).toEqual(single('a'));
  });

  it('stops at the pane limit', () => {
    const full = { panes: ['a', 'b', 'c', 'd'].slice(0, MAX_PANES), selectedId: 'a' };
    expect(toggleSplit(full, 'e')).toBe(full);
  });
});

describe('closePane', () => {
  it('focuses the neighbour of a closed focused pane', () => {
    expect(closePane({ panes: ['a', 'b', 'c'], selectedId: 'c' }, 'c')).toEqual({
      panes: ['a', 'b'],
      selectedId: 'b',
    });
  });

  it('never closes the last pane', () => {
    expect(closePane(single('a'), 'a')).toEqual(single('a'));
  });
});

describe('pruneLayout', () => {
  it('drops panes whose session ended', () => {
    const layout = { panes: ['a', 'b', 'c'], selectedId: 'a' };
    const sessions = [session('a', 'running'), session('b', 'exited'), session('c', 'idle')];
    expect(pruneLayout(layout, sessions)).toEqual({ panes: ['a', 'c'], selectedId: 'a' });
  });

  it('shows the focused session alone when it ended', () => {
    const layout = { panes: ['a', 'b'], selectedId: 'a' };
    expect(pruneLayout(layout, [session('a', 'finished'), session('b', 'running')])).toEqual(
      single('a'),
    );
  });

  it('keeps an unchanged layout as is', () => {
    const layout = { panes: ['a', 'b'], selectedId: 'a' };
    expect(pruneLayout(layout, [session('a', 'running'), session('b', 'idle')])).toBe(layout);
  });
});
