import { useCallback, useId, useRef, useState, type KeyboardEvent } from 'react';
import { Columns2 } from 'lucide-react';
import { Button } from '../components/Button.tsx';
import list from '../components/Select.module.css';
import { nextIndex, usePopover } from '../components/usePopover.ts';
import { isLive } from '../session/status.ts';
import { MAX_PANES } from '../state/layout.ts';
import { useAppStore } from '../state/store.ts';

const MENU_WIDTH = 240;

/** "Dividir": picks another live session to show next to the current ones. */
export function SplitMenu() {
  const sessions = useAppStore((state) => state.sessions);
  const panes = useAppStore((state) => state.panes);
  const toggleSplit = useAppStore((state) => state.toggleSplit);
  const menuId = useId();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const choices = sessions.filter(
    (session) => isLive(session.status) && !panes.includes(session.id),
  );
  const full = panes.length >= MAX_PANES;

  const close = useCallback((focusTrigger: boolean) => {
    setOpen(false);
    if (focusTrigger) triggerRef.current?.focus();
  }, []);
  const dismiss = useCallback(() => close(false), [close]);
  const position = usePopover(open, choices.length, triggerRef, listRef, dismiss, MENU_WIDTH);

  const pick = (index: number) => {
    const choice = choices[index];
    setOpen(false);
    if (choice !== undefined) toggleSplit(choice.id);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLUListElement>) => {
    const next = nextIndex(event.key, active, choices.length);
    if (next !== null) {
      event.preventDefault();
      setActive(next);
    } else if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      pick(active);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      close(true);
    } else if (event.key === 'Tab') {
      close(false);
    }
  };

  return (
    <>
      <Button
        ref={triggerRef}
        icon={<Columns2 size={16} aria-hidden="true" />}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        disabled={choices.length === 0 || full}
        title={full ? `Até ${MAX_PANES} terminais lado a lado` : undefined}
        onClick={() => {
          setActive(0);
          setOpen((value) => !value);
        }}
      >
        Dividir
      </Button>
      {open && (
        <ul
          ref={listRef}
          id={menuId}
          role="menu"
          tabIndex={-1}
          aria-label="Mostrar ao lado"
          aria-activedescendant={`${menuId}-${active}`}
          className={list.listbox}
          style={{ left: position.left, top: position.top, width: position.width }}
          onKeyDown={onKeyDown}
        >
          {choices.map((session, index) => (
            <li
              key={session.id}
              id={`${menuId}-${index}`}
              role="menuitem"
              data-active={index === active}
              className={list.option}
              onPointerEnter={() => setActive(index)}
              onClick={() => pick(index)}
            >
              <span className={list.optionLabel}>{session.name}</span>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}
