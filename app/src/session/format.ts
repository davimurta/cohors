import type { Agent } from '@cohors/protocol';

export const AGENT_LABEL: Record<Agent, string> = {
  claude: 'Claude',
  shell: 'Shell',
};

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

/** "42 min", "3 h", "2 dias", or null under a minute. */
export function formatDuration(since: string, now: number): string | null {
  const elapsed = Math.max(now - Date.parse(since), 0);
  if (elapsed < MINUTE_MS) return null;
  if (elapsed < HOUR_MS) return `${Math.floor(elapsed / MINUTE_MS)} min`;
  if (elapsed < DAY_MS) return `${Math.floor(elapsed / HOUR_MS)} h`;
  const days = Math.floor(elapsed / DAY_MS);
  return `${days} ${days === 1 ? 'dia' : 'dias'}`;
}

/** "agora", "há 42 min", "há 3 h", "há 2 dias". */
export function formatElapsed(since: string, now: number): string {
  const duration = formatDuration(since, now);
  return duration === null ? 'agora' : `há ${duration}`;
}

/** "hoje, 18:42", "ontem, 09:05" or "12/11, 18:42" (with the year if not the current one). */
export function formatLastActivity(at: string, now: number): string {
  const date = new Date(at);
  const today = new Date(now);
  const time = `${pad(date.getHours())}:${pad(date.getMinutes())}`;
  const dayDiff = Math.round((startOfDay(today) - startOfDay(date)) / DAY_MS);
  if (dayDiff === 0) return `hoje, ${time}`;
  if (dayDiff === 1) return `ontem, ${time}`;
  const day = `${pad(date.getDate())}/${pad(date.getMonth() + 1)}`;
  const year = date.getFullYear() === today.getFullYear() ? '' : `/${date.getFullYear()}`;
  return `${day}${year}, ${time}`;
}

function startOfDay(date: Date): number {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

function pad(value: number): string {
  return String(value).padStart(2, '0');
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
