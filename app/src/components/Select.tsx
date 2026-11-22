import { useCallback, useEffect, useId, useRef, useState, type KeyboardEvent } from 'react';
import { Check, ChevronDown } from 'lucide-react';
import styles from './Select.module.css';
import { nextIndex, usePopover } from './usePopover.ts';

export interface SelectOption {
  value: string;
  label: string;
}

interface SelectProps {
  id: string;
  /** Id of the visible label; the button is named by it plus the value. */
  labelId: string;
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
  'aria-describedby'?: string;
}

/**
 * A select that looks like the text inputs. Keyboard: arrows and Home/End
 * move, Enter or Space picks, Esc closes without leaving the dialog.
 */
export function Select({ id, labelId, value, options, onChange, ...rest }: SelectProps) {
  const listId = useId();
  const valueId = useId();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const selectedIndex = Math.max(
    options.findIndex((option) => option.value === value),
    0,
  );
  const selected = options[selectedIndex];

  const openList = () => {
    setActive(selectedIndex);
    setOpen(true);
  };

  const close = useCallback((focusTrigger: boolean) => {
    setOpen(false);
    if (focusTrigger) triggerRef.current?.focus();
  }, []);
  const dismiss = useCallback(() => close(false), [close]);
  const position = usePopover(open, options.length, triggerRef, listRef, dismiss);

  const pick = (index: number) => {
    const option = options[index];
    if (option !== undefined) onChange(option.value);
    close(true);
  };

  useEffect(() => {
    if (open) listRef.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: 'nearest' });
  }, [open, active]);

  const onTriggerKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      openList();
    }
  };

  const onListKeyDown = (event: KeyboardEvent<HTMLUListElement>) => {
    const next = nextIndex(event.key, active, options.length);
    if (next !== null) {
      event.preventDefault();
      setActive(next);
    } else if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      pick(active);
    } else if (event.key === 'Escape') {
      // Keeps the surrounding <dialog> from closing too.
      event.preventDefault();
      event.stopPropagation();
      close(true);
    } else if (event.key === 'Tab') {
      close(false);
    }
  };

  return (
    <>
      <button
        ref={triggerRef}
        id={id}
        type="button"
        className={styles.trigger}
        aria-haspopup="listbox"
        aria-labelledby={`${labelId} ${valueId}`}
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        onClick={() => (open ? close(true) : openList())}
        onKeyDown={onTriggerKeyDown}
        {...rest}
      >
        <span id={valueId} className={styles.value}>
          {selected?.label ?? ''}
        </span>
        <ChevronDown size={16} className={styles.chevron} aria-hidden="true" />
      </button>
      {open && (
        <ul
          ref={listRef}
          id={listId}
          role="listbox"
          tabIndex={-1}
          aria-labelledby={labelId}
          aria-activedescendant={`${listId}-${active}`}
          className={styles.listbox}
          style={{ left: position.left, top: position.top, width: position.width }}
          onKeyDown={onListKeyDown}
        >
          {options.map((option, index) => (
            <li
              key={option.value}
              id={`${listId}-${index}`}
              data-index={index}
              role="option"
              aria-selected={index === selectedIndex}
              data-active={index === active}
              className={styles.option}
              onPointerEnter={() => setActive(index)}
              onClick={() => pick(index)}
            >
              <span className={styles.optionLabel}>{option.label}</span>
              {index === selectedIndex && (
                <Check size={16} className={styles.check} aria-hidden="true" />
              )}
            </li>
          ))}
        </ul>
      )}
    </>
  );
}
