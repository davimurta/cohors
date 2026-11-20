export interface DaemonConnection {
  port: number;
  token: string;
}

/** Reads daemon.json through the dev server. Returns null while the daemon is not running. */
export async function loadDaemonConnection(): Promise<DaemonConnection | null> {
  try {
    const response = await fetch('/daemon.json', { cache: 'no-store' });
    if (!response.ok) return null;
    return parseConnection(await response.json());
  } catch {
    return null;
  }
}

function parseConnection(value: unknown): DaemonConnection | null {
  if (typeof value !== 'object' || value === null) return null;
  if (!('port' in value) || !('token' in value)) return null;
  const { port, token } = value;
  if (typeof port !== 'number' || typeof token !== 'string') return null;
  return { port, token };
}
