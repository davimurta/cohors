import { useEffect, useState } from 'react';
import { Search, X } from 'lucide-react';
import { useAppStore } from '../state/store.ts';
import styles from './HistorySearch.module.css';

/** Waits for a pause in typing before asking the daemon. */
const SEARCH_DELAY_MS = 250;

/**
 * Searches finished sessions by name, notes, summary and conversation.
 * `finishedCount` reruns the search when a session joins or leaves the
 * history.
 */
export function HistorySearch({ finishedCount }: { finishedCount: number }) {
  const searchHistory = useAppStore((state) => state.searchHistory);
  const [query, setQuery] = useState(
    () => useAppStore.getState().historySearch?.query ?? '',
  );

  useEffect(() => {
    const timer = window.setTimeout(() => searchHistory(query), SEARCH_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [query, finishedCount, searchHistory]);

  return (
    <div className={styles.search}>
      <Search size={14} aria-hidden="true" className={styles.icon} />
      <input
        type="search"
        className={styles.input}
        value={query}
        placeholder="Buscar no histórico"
        aria-label="Buscar nas sessões finalizadas"
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Escape' && query !== '') {
            event.preventDefault();
            setQuery('');
          }
        }}
      />
      {query !== '' && (
        <button
          type="button"
          className={styles.clear}
          aria-label="Limpar busca"
          onClick={() => setQuery('')}
        >
          <X size={14} aria-hidden="true" />
        </button>
      )}
    </div>
  );
}
