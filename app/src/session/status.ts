import type { SessionStatus } from '@cohors/protocol';

export const STATUS_LABEL: Record<SessionStatus, string> = {
  running: 'Rodando',
  idle: 'Ociosa',
  exited: 'Encerrada',
  interrupted: 'Interrompida',
};

/** Running or idle: the session has a live process and a terminal. */
export function isLive(status: SessionStatus): boolean {
  return status === 'running' || status === 'idle';
}
