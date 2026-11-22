import type { Project, SessionMeta } from '@cohors/protocol';
import { inTab } from '../state/sessions.ts';

export interface SessionGroup {
  /** null holds sessions without a project, or whose project was deleted. */
  project: Project | null;
  sessions: SessionMeta[];
}

/**
 * Every project in order, even without sessions, so new sessions can start
 * from it; then the sessions without a project, if any.
 */
export function groupSessions(sessions: SessionMeta[], projects: Project[]): SessionGroup[] {
  const known = new Set(projects.map((project) => project.id));
  const groups: SessionGroup[] = projects.map((project) => ({
    project,
    sessions: sessions.filter((session) => session.projectId === project.id),
  }));
  const loose = sessions.filter(
    (session) => session.projectId === null || !known.has(session.projectId),
  );
  if (loose.length > 0 || groups.length === 0) groups.push({ project: null, sessions: loose });
  return groups;
}

/** Active sessions in the order the sidebar lists them. */
export function sidebarOrder(sessions: SessionMeta[], projects: Project[]): SessionMeta[] {
  const active = sessions.filter((session) => inTab(session, 'active'));
  return groupSessions(active, projects).flatMap((group) => group.sessions);
}
