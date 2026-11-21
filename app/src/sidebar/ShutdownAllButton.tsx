import { useState } from 'react';
import { Power } from 'lucide-react';
import { Button } from '../components/Button.tsx';
import { ConfirmDialog } from '../components/ConfirmDialog.tsx';
import { useAppStore } from '../state/store.ts';

export function ShutdownAllButton() {
  const [confirming, setConfirming] = useState(false);
  const running = useAppStore(
    (state) => state.sessions.filter((session) => session.status === 'running').length,
  );
  const shutdownDaemon = useAppStore((state) => state.shutdownDaemon);

  return (
    <>
      <Button
        variant="ghost"
        size="small"
        icon={<Power size={14} aria-hidden="true" />}
        onClick={() => setConfirming(true)}
      >
        Encerrar tudo
      </Button>
      <ConfirmDialog
        open={confirming}
        title="Encerrar tudo"
        confirmLabel="Encerrar tudo"
        onConfirm={shutdownDaemon}
        onClose={() => setConfirming(false)}
      >
        {shutdownMessage(running)}
      </ConfirmDialog>
    </>
  );
}

function shutdownMessage(running: number): string {
  if (running === 0) return 'Nenhuma sessão está rodando agora. O daemon será reiniciado.';
  const sessions = running === 1 ? '1 sessão será interrompida' : `${running} sessões serão interrompidas`;
  return `${sessions}. Você poderá retomá-las depois.`;
}
