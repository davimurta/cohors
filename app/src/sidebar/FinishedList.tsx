import { Fragment } from 'react';
import type { SessionMeta } from '@cohors/protocol';
import { formatDay } from '../session/format.ts';
import { useNow } from '../session/useNow.ts';
import { SessionItem } from './SessionItem.tsx';
import styles from './FinishedList.module.css';

interface FinishedListProps {
  sessions: SessionMeta[];
  selectedId: string | null;
  home: string | null;
  onSelect: (id: string) => void;
}

/** Finished sessions, newest first, under a heading per day. */
export function FinishedList({ sessions, selectedId, home, onSelect }: FinishedListProps) {
  const now = useNow();
  if (sessions.length === 0) {
    return <p className={styles.empty}>As sessões finalizadas aparecem aqui.</p>;
  }
  const days = groupByDay(sessions, now);

  return (
    <nav aria-label="Sessões finalizadas" className={styles.list}>
      {days.map(({ day, sessions: daySessions }) => (
        <Fragment key={day}>
          <span className={styles.day}>{day}</span>
          {daySessions.map((session) => (
            <SessionItem
              key={session.id}
              session={session}
              selected={session.id === selectedId}
              home={home}
              now={now}
              onSelect={onSelect}
            />
          ))}
        </Fragment>
      ))}
    </nav>
  );
}

function groupByDay(sessions: SessionMeta[], now: number) {
  const ended = (session: SessionMeta) => session.endedAt ?? session.createdAt;
  const sorted = [...sessions].sort((a, b) => ended(b).localeCompare(ended(a)));
  const days: { day: string; sessions: SessionMeta[] }[] = [];
  for (const session of sorted) {
    const day = formatDay(ended(session), now);
    const last = days.at(-1);
    if (last?.day === day) last.sessions.push(session);
    else days.push({ day, sessions: [session] });
  }
  return days;
}
