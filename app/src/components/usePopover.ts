import { useEffect, useLayoutEffect, useState, type RefObject } from 'react';

const GAP = 4;
const ITEM_HEIGHT = 36;
const MAX_HEIGHT = 240;

export interface PopoverPosition {
  left: number;
  top: number;
  width: number;
}

/**
 * Places a list under its trigger (above when there is no room) with fixed
 * positioning, so dialogs that scroll or clip do not cut it, and closes it
 * on a click outside both.
 */
export function usePopover(
  open: boolean,
  itemCount: number,
  triggerRef: RefObject<HTMLElement | null>,
  listRef: RefObject<HTMLElement | null>,
  onDismiss: () => void,
  minWidth = 0,
): PopoverPosition {
  const [position, setPosition] = useState<PopoverPosition>({ left: 0, top: 0, width: 0 });

  useLayoutEffect(() => {
    const trigger = triggerRef.current;
    if (!open || trigger === null) return;
    const rect = trigger.getBoundingClientRect();
    const height = Math.min(MAX_HEIGHT, itemCount * ITEM_HEIGHT + 10);
    const width = Math.max(rect.width, minWidth);
    const fitsBelow = rect.bottom + GAP + height <= window.innerHeight;
    setPosition({
      left: Math.min(rect.left, window.innerWidth - width - GAP),
      width,
      top: fitsBelow ? rect.bottom + GAP : rect.top - GAP - height,
    });
    listRef.current?.focus();
  }, [open, itemCount, minWidth, triggerRef, listRef]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!listRef.current?.contains(target) && !triggerRef.current?.contains(target)) onDismiss();
    };
    document.addEventListener('pointerdown', onPointerDown);
    return () => document.removeEventListener('pointerdown', onPointerDown);
  }, [open, onDismiss, triggerRef, listRef]);

  return position;
}

/** Keyboard moves shared by lists: arrows, Home and End. */
export function nextIndex(key: string, current: number, count: number): number | null {
  const last = count - 1;
  switch (key) {
    case 'ArrowDown':
      return Math.min(current + 1, last);
    case 'ArrowUp':
      return Math.max(current - 1, 0);
    case 'Home':
      return 0;
    case 'End':
      return last;
    default:
      return null;
  }
}
