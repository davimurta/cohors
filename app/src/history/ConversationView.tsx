import type { ConversationEntry } from '@cohors/protocol';
import { Wrench } from 'lucide-react';
import { formatTime } from '../session/format.ts';
import { SummaryText } from './SummaryText.tsx';
import styles from './ConversationView.module.css';

type Turn =
  | { kind: 'message'; entry: ConversationEntry }
  | { kind: 'tools'; entries: ConversationEntry[] };

/** User and Claude messages, with each run of tool calls folded into a list. */
export function ConversationView({ entries }: { entries: ConversationEntry[] }) {
  if (entries.length === 0) {
    return <p className={styles.empty}>A conversa não tem mensagens.</p>;
  }
  return (
    <ol className={styles.conversation} aria-label="Conversa">
      {turns(entries).map((turn, index) =>
        turn.kind === 'tools' ? (
          <li key={index} className={styles.tools}>
            <ul aria-label="Ferramentas usadas" className={styles.toolList}>
              {turn.entries.map((entry, toolIndex) => (
                <li key={toolIndex} className={styles.tool}>
                  <Wrench size={12} aria-hidden="true" />
                  <span>{entry.text}</span>
                </li>
              ))}
            </ul>
          </li>
        ) : (
          <li
            key={index}
            className={turn.entry.role === 'user' ? styles.user : styles.assistant}
          >
            <span className={styles.author}>
              {turn.entry.role === 'user' ? 'Você' : 'Claude'}
              {turn.entry.timestamp !== null && (
                <time className={styles.time} dateTime={turn.entry.timestamp}>
                  {formatTime(turn.entry.timestamp)}
                </time>
              )}
            </span>
            {turn.entry.role === 'user' ? (
              <p className={styles.userText}>{turn.entry.text}</p>
            ) : (
              <SummaryText markdown={turn.entry.text} />
            )}
          </li>
        ),
      )}
    </ol>
  );
}

function turns(entries: ConversationEntry[]): Turn[] {
  const result: Turn[] = [];
  for (const entry of entries) {
    const last = result.at(-1);
    if (entry.kind === 'tool') {
      if (last?.kind === 'tools') last.entries.push(entry);
      else result.push({ kind: 'tools', entries: [entry] });
    } else {
      result.push({ kind: 'message', entry });
    }
  }
  return result;
}
