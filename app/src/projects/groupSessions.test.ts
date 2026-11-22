import { describe, expect, it } from 'vitest';
import type { Project, SessionMeta } from '@cohors/protocol';
import { groupSessions } from './groupSessions.ts';

const project = (id: string): Project => ({
  id,
  name: id,
  root: `/dev/${id}`,
  claudeConfigDir: null,
  sshKey: null,
  contextDir: null,
});

const session = (id: string, projectId: string | null): SessionMeta => ({
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
  projectId,
  claudeConfigDir: null,
  sshKey: null,
  contextDir: null,
  notes: '',
  summaryStatus: 'none',
});

const ids = (groups: ReturnType<typeof groupSessions>) =>
  groups.map((group) => [group.project?.id ?? null, group.sessions.map((s) => s.id)]);

describe('groupSessions', () => {
  it('puts everything in one loose group without projects', () => {
    expect(ids(groupSessions([session('a', null)], []))).toEqual([[null, ['a']]]);
    expect(ids(groupSessions([], []))).toEqual([[null, []]]);
  });

  it('groups by project and keeps empty projects', () => {
    const groups = groupSessions(
      [session('a', 'p1'), session('b', null), session('c', 'p1')],
      [project('p1'), project('p2')],
    );
    expect(ids(groups)).toEqual([
      ['p1', ['a', 'c']],
      ['p2', []],
      [null, ['b']],
    ]);
  });

  it('treats sessions of deleted projects as loose', () => {
    expect(ids(groupSessions([session('a', 'gone')], [project('p1')]))).toEqual([
      ['p1', []],
      [null, ['a']],
    ]);
  });
});
