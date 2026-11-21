import { useEffect, useState } from 'react';

const TICK_MS = 30_000;

/** Current time, refreshed often enough for minute-level labels. */
export function useNow(): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), TICK_MS);
    return () => clearInterval(timer);
  }, []);
  return now;
}
