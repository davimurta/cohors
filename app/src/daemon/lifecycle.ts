import { isTauri } from '@tauri-apps/api/core';
import { Command } from '@tauri-apps/plugin-shell';

/** Only the desktop app can start and stop the bundled daemon. */
export const canManageDaemon = isTauri();

/** Starts the daemon in the background; returns once it accepts connections. */
export function startDaemon(): Promise<void> {
  return runDaemon('--detach');
}

/** Stops the daemon cleanly, which interrupts its sessions. */
export function stopDaemon(): Promise<void> {
  return runDaemon('--stop');
}

async function runDaemon(flag: '--detach' | '--stop'): Promise<void> {
  const output = await Command.sidecar('binaries/cohors-daemon', [flag]).execute();
  if (output.code !== 0) {
    throw new Error(output.stderr.trim() || `cohors-daemon ${flag} exited with ${output.code}`);
  }
}
