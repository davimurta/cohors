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
