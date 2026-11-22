import { useState } from 'react';
import { CircleCheck } from 'lucide-react';
import type { SessionMeta } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import { ConfirmDialog } from '../components/ConfirmDialog.tsx';
import { useAppStore } from '../state/store.ts';
import { isLive } from './status.ts';

interface FinishButtonProps {
  session: SessionMeta;
  label: string;
  size?: 'default' | 'large';
  /** Icon only, for the compact pane headers of the split view. */
  compact?: boolean;
}

/** Finalizar, with confirmation: ends the process and moves the session to the history. */
export function FinishButton({ session, label, size = 'default', compact = false }: FinishButtonProps) {
  const finishSession = useAppStore((state) => state.finishSession);
  const [confirming, setConfirming] = useState(false);
  const summarize = session.agent === 'claude';

  return (
    <>
      {compact ? (
        <Button
          variant="ghost"
          size="small"
          icon={<CircleCheck size={14} aria-hidden="true" />}
          aria-label={`${label} ${session.name}`}
          title={label}
          onClick={() => setConfirming(true)}
        />
      ) : (
        <Button
          size={size}
          icon={<CircleCheck size={16} aria-hidden="true" />}
          onClick={() => setConfirming(true)}
        >
          {label}
        </Button>
      )}
      <ConfirmDialog
        open={confirming}
        title="Finalizar sessão"
        confirmLabel={summarize ? 'Finalizar e gerar resumo' : 'Finalizar'}
        onConfirm={() => finishSession(session.id, summarize)}
        onClose={() => setConfirming(false)}
      >
        {confirmation(session, summarize)}
      </ConfirmDialog>
    </>
  );
}

function confirmation(session: SessionMeta, summarize: boolean): string {
  const stop = isLive(session.status) ? `O processo de ${session.name} será encerrado. ` : '';
  const summary = summarize
    ? 'O Claude vai gerar um resumo do que foi feito, usando o limite do seu plano. '
    : '';
  return `${stop}${summary}A sessão vai para Finalizadas.`;
}
