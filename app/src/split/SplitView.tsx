import { useState, type CSSProperties, type DragEvent, type KeyboardEvent } from 'react';
import { FileText, GripVertical, X } from 'lucide-react';
import type { SessionMeta } from '@cohors/protocol';
import { Button } from '../components/Button.tsx';
import { StatusDot } from '../components/StatusDot.tsx';
import { isMac } from '../platform.ts';
import { FinishButton } from '../session/FinishButton.tsx';
import { STATUS_LABEL } from '../session/status.ts';
import { useAppStore } from '../state/store.ts';
import { TerminalPane } from '../terminal/TerminalPane.tsx';
import { SplitDivider } from './SplitDivider.tsx';
import { SplitMenu } from './SplitMenu.tsx';
import styles from './SplitView.module.css';

const SPLIT_CLICK = isMac ? 'Cmd+clique' : 'Ctrl+clique';
/** Drag data type, so other drags (files, text) are not taken for panes. */
const PANE_TYPE = 'application/x-cohors-pane';
/** Width of the gap tracks that hold the dividers. */
const GAP = '12px';

/**
 * Several live sessions side by side; the focused one is the selection.
 * Two panes sit in two columns; three or four use two rows too, with the
 * third spanning both columns when it is the last. Dividers sit in the gap
 * tracks between them.
 */
export function SplitView({ sessions }: { sessions: SessionMeta[] }) {
  const selectedId = useAppStore((state) => state.selectedId);
  const sizes = useAppStore((state) => state.splitSizes);
  const setSplitSizes = useAppStore((state) => state.setSplitSizes);
  const [gridElement, setGridElement] = useState<HTMLDivElement | null>(null);
  const twoRows = sessions.length > 2;
  const grid: CSSProperties = {
    gridTemplateColumns: `minmax(0, ${sizes.column}fr) ${GAP} minmax(0, ${1 - sizes.column}fr)`,
    gridTemplateRows: twoRows
      ? `minmax(0, ${sizes.row}fr) ${GAP} minmax(0, ${1 - sizes.row}fr)`
      : 'minmax(0, 1fr)',
  };

  return (
    <main className={styles.view}>
      <div className={styles.toolbar}>
        <span className={styles.hint}>
          {sessions.length} terminais lado a lado. {SPLIT_CLICK} numa sessão da lista adiciona ou
          tira um terminal.
        </span>
        <SplitMenu />
      </div>
      <div ref={setGridElement} className={styles.grid} style={grid}>
        {sessions.map((session, index) => (
          <Pane
            key={session.id}
            session={session}
            index={index}
            count={sessions.length}
            focused={session.id === selectedId}
          />
        ))}
        <SplitDivider
          orientation="vertical"
          label="Largura das colunas"
          share={sizes.column}
          container={gridElement}
          style={{ gridColumn: 2, gridRow: sessions.length === 3 ? 1 : '1 / -1' }}
          onChange={(column) => setSplitSizes({ ...sizes, column })}
        />
        {twoRows && (
          <SplitDivider
            orientation="horizontal"
            label="Altura das linhas"
            share={sizes.row}
            container={gridElement}
            style={{ gridColumn: '1 / -1', gridRow: 2 }}
            onChange={(row) => setSplitSizes({ ...sizes, row })}
          />
        )}
      </div>
    </main>
  );
}

/** Grid cell of the pane at `index`, skipping the gap tracks. */
function placement(index: number, count: number): CSSProperties {
  const row = index < 2 ? 1 : 3;
  if (count === 3 && index === 2) return { gridRow: row, gridColumn: '1 / -1' };
  return { gridRow: row, gridColumn: index % 2 === 0 ? 1 : 3 };
}

interface PaneProps {
  session: SessionMeta;
  index: number;
  count: number;
  focused: boolean;
}

function Pane({ session, index, count, focused }: PaneProps) {
  const select = useAppStore((state) => state.select);
  const closePane = useAppStore((state) => state.closePane);
  const movePane = useAppStore((state) => state.movePane);
  const openNotes = useAppStore((state) => state.openNotes);
  const [dropTarget, setDropTarget] = useState(false);

  const acceptsDrop = (event: DragEvent) => event.dataTransfer.types.includes(PANE_TYPE);

  const onDrop = (event: DragEvent) => {
    setDropTarget(false);
    const id = event.dataTransfer.getData(PANE_TYPE);
    if (id === '' || id === session.id) return;
    event.preventDefault();
    movePane(id, index);
  };

  // Arrow keys on the grip move the pane; focus follows it to its new place.
  const onGripKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    const step = { ArrowLeft: -1, ArrowUp: -1, ArrowRight: 1, ArrowDown: 1 }[event.key];
    if (step === undefined) return;
    event.preventDefault();
    const target = index + step;
    if (target < 0 || target >= count) return;
    movePane(session.id, target);
    requestAnimationFrame(() =>
      document.querySelector<HTMLElement>(`[data-grip="${session.id}"]`)?.focus(),
    );
  };

  return (
    <section
      className={styles.pane}
      style={placement(index, count)}
      data-focused={focused}
      data-drop-target={dropTarget}
      aria-label={session.name}
      // Clicking or tabbing into a pane makes it the focused session.
      onFocusCapture={() => {
        if (!focused) select(session.id);
      }}
      onDragOver={(event) => {
        if (!acceptsDrop(event)) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = 'move';
        setDropTarget(true);
      }}
      onDragLeave={() => setDropTarget(false)}
      onDrop={onDrop}
    >
      <header className={styles.paneHeader}>
        <button
          type="button"
          className={styles.grip}
          data-grip={session.id}
          draggable
          aria-label={`Mover painel de ${session.name}, posição ${index + 1} de ${count}`}
          aria-description="Arraste até outro painel ou use as setas."
          title="Arraste para trocar de lugar"
          onDragStart={(event) => {
            event.dataTransfer.setData(PANE_TYPE, session.id);
            event.dataTransfer.effectAllowed = 'move';
          }}
          onKeyDown={onGripKeyDown}
        >
          <GripVertical size={14} aria-hidden="true" />
        </button>
        <StatusDot status={session.status} />
        <span className={styles.paneName}>{session.name}</span>
        <span className={styles.paneStatus}>{STATUS_LABEL[session.status]}</span>
        <Button
          variant="ghost"
          size="small"
          icon={<FileText size={14} aria-hidden="true" />}
          aria-label={`Notas de ${session.name}`}
          title="Notas"
          onClick={() => openNotes(session.id)}
        />
        <FinishButton session={session} label="Finalizar" compact />
        <Button
          variant="ghost"
          size="small"
          icon={<X size={14} aria-hidden="true" />}
          aria-label={`Fechar painel de ${session.name}`}
          title="Fechar painel"
          onClick={() => closePane(session.id)}
        />
      </header>
      <div className={styles.terminal}>
        <TerminalPane
          sessionId={session.id}
          label={`Terminal da sessão ${session.name}`}
          focus={focused}
        />
      </div>
    </section>
  );
}
