import { isTauri } from '@tauri-apps/api/core';
import { Command } from '@tauri-apps/plugin-shell';

/** Only the desktop app can start and stop the bundled daemon. */
export const canManageDaemon = isTauri();

/** Starts the daemon in the background; returns once it accepts connections. */
export async function startDaemon(): Promise<void> {
  await runDaemon('--detach');
}

/** Stops the daemon cleanly, which interrupts its sessions. */
export async function stopDaemon(): Promise<void> {
  await runDaemon('--stop');
}

/** Build id of the daemon this app would start, to compare with the running one. */
export async function bundledBuildId(): Promise<string> {
  return (await runDaemon('--build-id')).trim();
}

async function runDaemon(flag: '--detach' | '--stop' | '--build-id'): Promise<string> {
  const output = await Command.sidecar('binaries/cohors-daemon', [flag]).execute();
  if (output.code !== 0) {
    throw new Error(output.stderr.trim() || `cohors-daemon ${flag} exited with ${output.code}`);
  }
  return output.stdout;
}
