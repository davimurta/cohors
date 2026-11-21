// TUIs like Claude Code draw in the alternate screen, which a terminal throws
// away when the program exits, and clear the screen as they redraw. Replaying
// the raw transcript would then show little more than the last frame, so the
// replay stays in the normal buffer and every clear first scrolls the
// current screen into the scrollback, where it can still be read.

const ESC = String.fromCharCode(0x1b);
// ESC [ ? 47 / 1047 / 1049 h|l (alternate screen) and ESC [ 3 J (clear scrollback).
const DROPPED = new RegExp(`${ESC}\\[\\?(?:47|1047|1049)[hl]|${ESC}\\[3J`, 'g');
const CLEAR_SCREEN = `${ESC}[2J`;

export function flattenTranscript(data: string, rows: number): string {
  const scrollAway = `${ESC}[${rows};1H${'\n'.repeat(rows)}`;
  return data.replace(DROPPED, '').replaceAll(CLEAR_SCREEN, scrollAway + CLEAR_SCREEN);
}
