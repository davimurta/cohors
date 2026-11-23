import { describe, expect, it } from 'vitest';
import type { Attention, SessionMeta } from '@cohors/protocol';
import { newlyWaiting } from './attention.ts';

const session = (id: string, attention: Attention | null): SessionMeta => ({
  id,
  name: id,
  agent: 'claude',
  cwd: '/dev',
  agentSessionId: 'a',
  status: 'running',
  exitCode: null,
  createdAt: '2026-01-01T00:00:00Z',
  endedAt: null,
  idleSince: null,
  projectId: null,
  claudeConfigDir: null,
  sshKey: null,
  contextDir: null,
  attention,
  worktree: null,
  notes: '',
  summaryStatus: 'none',
});

describe('newlyWaiting', () => {
  const nothingVisible = () => false;

  it('reports sessions that just started waiting', () => {
    const result = newlyWaiting(
      [session('a', null), session('b', 'done')],
      [session('a', 'input'), session('b', 'done')],
      nothingVisible,
    );
    expect(result.map((waiting) => waiting.id)).toEqual(['a']);
  });

  it('ignores sessions it did not know before', () => {
    expect(newlyWaiting([], [session('a', 'done')], nothingVisible)).toEqual([]);
  });

  it('skips the session the user is looking at', () => {
    const result = newlyWaiting([session('a', null)], [session('a', 'done')], (id) => id === 'a');
    expect(result).toEqual([]);
  });
});
