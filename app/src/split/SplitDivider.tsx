import type { CSSProperties, KeyboardEvent, PointerEvent } from 'react';
import { clampShare } from '../state/layout.ts';
import styles from './SplitDivider.module.css';

/** Arrow keys move the divider by this share. */
const KEY_STEP = 0.05;
/** Both sides the same size. */
const EVEN_SHARE = 0.5;

interface SplitDividerProps {
  /** "vertical" splits columns, "horizontal" splits rows. */
  orientation: 'vertical' | 'horizontal';
  label: string;
  /** The first column's or row's share of the space. */
  share: number;
  /** The grid the share is measured against. */
  container: HTMLElement | null;
  style: CSSProperties;
  onChange: (share: number) => void;
}

/**
 * Draggable divider between split panes. Keyboard: arrows move it, Home and
 * End go to the limits, Enter evens it out, as does a double click.
 */
export function SplitDivider({
  orientation,
  label,
  share,
  container,
  style,
  onChange,
}: SplitDividerProps) {
  const vertical = orientation === 'vertical';

  const shareAt = (event: PointerEvent) => {
    if (container === null) return share;
    const rect = container.getBoundingClientRect();
    return vertical
      ? (event.clientX - rect.left) / rect.width
      : (event.clientY - rect.top) / rect.height;
  };

  const onKeyDown = (event: KeyboardEvent) => {
    const back = vertical ? 'ArrowLeft' : 'ArrowUp';
    const forward = vertical ? 'ArrowRight' : 'ArrowDown';
    const next = {
      [back]: share - KEY_STEP,
      [forward]: share + KEY_STEP,
      Home: 0,
      End: 1,
      Enter: EVEN_SHARE,
    }[event.key];
    if (next === undefined) return;
    event.preventDefault();
    onChange(clampShare(next));
  };

  return (
    <div
      role="separator"
      tabIndex={0}
      aria-label={label}
      aria-orientation={orientation}
      aria-valuemin={20}
      aria-valuemax={80}
      aria-valuenow={Math.round(share * 100)}
      className={styles.divider}
      data-orientation={orientation}
      style={style}
      onKeyDown={onKeyDown}
      onDoubleClick={() => onChange(EVEN_SHARE)}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={(event) => {
        if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
        onChange(clampShare(shareAt(event)));
      }}
    />
  );
}
