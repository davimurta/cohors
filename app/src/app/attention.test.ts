import { describe, expect, it } from 'vitest';
import type { Attention, SessionMeta } from '@cohors/protocol';
import { newlyWaiting, nextWaiting, waitingCount } from './attention.ts';

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

describe('nextWaiting', () => {
  const list = [session('a', 'done'), session('b', null), session('c', 'input'), session('d', null)];

  it('goes to the next waiting session after the selected one', () => {
    expect(nextWaiting(list, 'a')?.id).toBe('c');
    expect(nextWaiting(list, 'b')?.id).toBe('c');
  });

  it('wraps around to the start', () => {
    expect(nextWaiting(list, 'c')?.id).toBe('a');
    expect(nextWaiting(list, 'd')?.id).toBe('a');
  });

  it('starts from the top when nothing is selected', () => {
    expect(nextWaiting(list, null)?.id).toBe('a');
    expect(nextWaiting(list, 'finished')?.id).toBe('a');
  });

  it('returns null when no other session waits', () => {
    expect(nextWaiting([session('a', 'done'), session('b', null)], 'a')).toBeNull();
    expect(nextWaiting([session('b', null)], null)).toBeNull();
    expect(nextWaiting([], null)).toBeNull();
  });
});

describe('waitingCount', () => {
  it('counts sessions with attention', () => {
    expect(waitingCount([session('a', 'done'), session('b', null), session('c', 'input')])).toBe(2);
  });
});
