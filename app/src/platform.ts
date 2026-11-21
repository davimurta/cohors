export const isMac = navigator.userAgent.includes('Mac');

export const NEW_SESSION_SHORTCUT = isMac ? '⌘N' : 'Ctrl+N';

/** Same shortcut in aria-keyshortcuts syntax. */
export const NEW_SESSION_KEYS = isMac ? 'Meta+N' : 'Control+N';
