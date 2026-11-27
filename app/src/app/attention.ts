import type { Attention, SessionMeta } from '@cohors/protocol';

export const ATTENTION_LABEL = 'Aguardando você';

export const ATTENTION_MESSAGE: Record<Attention, string> = {
  done: 'Claude terminou e está esperando você.',
  input: 'Claude precisa da sua resposta.',
};

/**
 * Sessions that started waiting since the previous state, except the one
 * the user is looking at: focused window and shown on screen. Sessions
 * unknown before (the list after connecting) do not count, so reopening the
 * app does not repeat old notifications.
 */
export function newlyWaiting(
  previous: SessionMeta[],
  current: SessionMeta[],
  visible: (id: string) => boolean,
): SessionMeta[] {
  const before = new Map(previous.map((session) => [session.id, session.attention]));
  return current.filter(
    (session) =>
      session.attention !== null && before.get(session.id) === null && !visible(session.id),
  );
}

/**
 * The next session waiting for the user, in sidebar order, after the
 * selected one and wrapping around; the first waiting one when the
 * selected session is not in the list. null when none waits, or when the
 * only waiting session is already selected.
 */
export function nextWaiting(
  ordered: SessionMeta[],
  selectedId: string | null,
): SessionMeta | null {
  const start = ordered.findIndex((session) => session.id === selectedId);
  for (let step = 1; step <= ordered.length; step++) {
    const session = ordered[(start + step) % ordered.length];
    if (session.attention !== null && session.id !== selectedId) return session;
  }
  return null;
}

/** How many sessions wait for the user, for the Dock badge. */
export function waitingCount(sessions: SessionMeta[]): number {
  return sessions.filter((session) => session.attention !== null).length;
}
