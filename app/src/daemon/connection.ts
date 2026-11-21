import { isTauri } from '@tauri-apps/api/core';
import { BaseDirectory, readTextFile } from '@tauri-apps/plugin-fs';

export interface DaemonConnection {
  port: number;
  token: string;
}

const DAEMON_FILE = 'cohors/daemon.json';
// Served by the Vite dev server when the UI runs in a plain browser.
const DEV_DAEMON_FILE_URL = '/daemon.json';

/** Reads the daemon port and token. Returns null while the daemon is not running. */
export async function loadDaemonConnection(): Promise<DaemonConnection | null> {
  try {
    const contents = isTauri()
      ? await readTextFile(DAEMON_FILE, { baseDir: BaseDirectory.Data })
      : await fetchDevDaemonFile();
    return parseConnection(JSON.parse(contents));
  } catch {
    return null;
  }
}

async function fetchDevDaemonFile(): Promise<string> {
  const response = await fetch(DEV_DAEMON_FILE_URL, { cache: 'no-store' });
  if (!response.ok) throw new Error(`daemon.json unavailable: ${response.status}`);
  return response.text();
}

function parseConnection(value: unknown): DaemonConnection | null {
  if (typeof value !== 'object' || value === null) return null;
  if (!('port' in value) || !('token' in value)) return null;
  const { port, token } = value;
  if (typeof port !== 'number' || typeof token !== 'string') return null;
  return { port, token };
}
