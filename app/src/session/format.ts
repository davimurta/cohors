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

/** "Hoje", "Ontem" or "12/11" (with the year if not the current one). */
export function formatDay(at: string, now: number): string {
  const date = new Date(at);
  const today = new Date(now);
  const dayDiff = Math.round((startOfDay(today) - startOfDay(date)) / DAY_MS);
  if (dayDiff === 0) return 'Hoje';
  if (dayDiff === 1) return 'Ontem';
  const day = `${pad(date.getDate())}/${pad(date.getMonth() + 1)}`;
  return date.getFullYear() === today.getFullYear() ? day : `${day}/${date.getFullYear()}`;
}

export function formatTime(at: string): string {
  const date = new Date(at);
  return `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/** "hoje, 18:42", "ontem, 09:05" or "12/11, 18:42". */
export function formatLastActivity(at: string, now: number): string {
  const day = formatDay(at, now);
  const label = day === 'Hoje' || day === 'Ontem' ? day.toLowerCase() : day;
  return `${label}, ${formatTime(at)}`;
}

/** "Hoje, 17:30 a 18:42", or both days when the session crossed midnight. */
export function formatTimeRange(start: string, end: string, now: number): string {
  const startDay = formatDay(start, now);
  const endDay = formatDay(end, now);
  const until = endDay === startDay ? formatTime(end) : `${endDay}, ${formatTime(end)}`;
  return `${startDay}, ${formatTime(start)} a ${until}`;
}

/** "38 min", "1 h 05 min", or "menos de 1 min". */
export function formatSpan(start: string, end: string): string {
  const minutes = Math.floor(Math.max(Date.parse(end) - Date.parse(start), 0) / MINUTE_MS);
  if (minutes < 1) return 'menos de 1 min';
  if (minutes < 60) return `${minutes} min`;
  return `${Math.floor(minutes / 60)} h ${pad(minutes % 60)} min`;
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

const TOKENS = new Intl.NumberFormat('pt-BR', { notation: 'compact', maximumFractionDigits: 1 });

/** "950", "12,3 mil", "1,2 mi". */
export function formatTokens(count: number): string {
  return TOKENS.format(count);
}
