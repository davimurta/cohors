import { BaseDirectory, readTextFile } from '@tauri-apps/plugin-fs';

export interface DaemonConnection {
  port: number;
  token: string;
}

const DAEMON_FILE = 'cohors/daemon.json';

/** Reads the daemon port and token. Returns null while the daemon is not running. */
export async function loadDaemonConnection(): Promise<DaemonConnection | null> {
  try {
    const contents = await readTextFile(DAEMON_FILE, { baseDir: BaseDirectory.Data });
    return parseConnection(JSON.parse(contents));
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
