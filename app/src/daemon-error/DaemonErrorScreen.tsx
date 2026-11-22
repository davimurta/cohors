import { Power, RotateCcw, TriangleAlert } from 'lucide-react';
import { Button } from '../components/Button.tsx';
import { canManageDaemon } from '../daemon/lifecycle.ts';
import { useAppStore } from '../state/store.ts';
import styles from './DaemonErrorScreen.module.css';

/** Shown while the daemon cannot be reached or runs an incompatible version. */
export function DaemonErrorScreen({ incompatible }: { incompatible: boolean }) {
  const detail = useAppStore((state) => state.connectionDetail);
  const retryConnection = useAppStore((state) => state.retryConnection);
  const restartDaemon = useAppStore((state) => state.restartDaemon);

  return (
    <main className={styles.main}>
      <div className={styles.content}>
        <span className={styles.icon}>
          <TriangleAlert size={24} strokeWidth={1.75} aria-hidden="true" />
        </span>
        <div className={styles.text}>
          <h1 className={styles.title}>
            {incompatible
              ? 'O daemon em execução é de outra versão'
              : 'Não foi possível conectar ao daemon'}
          </h1>
          <p className={styles.description}>
            {incompatible
              ? 'Depois de uma atualização, o daemon antigo continua rodando e não entende esta versão do Cohors. Reinicie o daemon para continuar.'
              : 'O Cohors precisa do daemon para manter suas sessões rodando. Ele não respondeu neste computador.'}
          </p>
        </div>
        <div className={styles.actions}>
          {!incompatible && (
            <Button
              variant="primary"
              size="large"
              icon={<RotateCcw size={16} aria-hidden="true" />}
              onClick={retryConnection}
            >
              Tentar de novo
            </Button>
          )}
          <Button
            variant={incompatible ? 'primary' : 'secondary'}
            size="large"
            icon={<Power size={16} aria-hidden="true" />}
            disabled={!canManageDaemon}
            onClick={restartDaemon}
          >
            Reiniciar daemon
          </Button>
        </div>
        <p className={styles.note}>
          Reiniciar o daemon interrompe as sessões em andamento. Você poderá retomá-las depois.
        </p>
        {detail !== null && <div className={styles.detail}>{detail}</div>}
      </div>
    </main>
  );
}
