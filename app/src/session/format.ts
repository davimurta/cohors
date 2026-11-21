import type { Agent } from '@cohors/protocol';

export const AGENT_LABEL: Record<Agent, string> = {
  claude: 'Claude',
  shell: 'Shell',
};

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

/** "agora", "há 42 min", "há 3 h", "há 2 dias". */
export function formatElapsed(since: string, now: number): string {
  const elapsed = Math.max(now - Date.parse(since), 0);
  if (elapsed < MINUTE_MS) return 'agora';
  if (elapsed < HOUR_MS) return `há ${Math.floor(elapsed / MINUTE_MS)} min`;
  if (elapsed < DAY_MS) return `há ${Math.floor(elapsed / HOUR_MS)} h`;
  const days = Math.floor(elapsed / DAY_MS);
  return `há ${days} ${days === 1 ? 'dia' : 'dias'}`;
}

/** Shortens paths inside the home folder to "~/...". */
export function displayPath(path: string, home: string | null): string {
  if (home === null || home === '') return path;
  const base = home.replace(/[\\/]+$/, '');
  if (path === base) return '~';
  if (path.startsWith(`${base}/`) || path.startsWith(`${base}\\`)) {
    return `~${path.slice(base.length)}`;
  }
  return path;
}
