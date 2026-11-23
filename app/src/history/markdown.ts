// The subset of Markdown that summaries and Claude answers use most:
// headings, "-" and "1." lists, fenced code blocks and paragraphs, with
// inline `code` and **bold**.

export type Inline = { kind: 'text' | 'code' | 'strong'; text: string };

export type Block =
  | { kind: 'heading'; content: Inline[] }
  | { kind: 'list'; ordered: boolean; items: Inline[][] }
  | { kind: 'code'; text: string }
  | { kind: 'paragraph'; content: Inline[] };

const HEADING = /^#{1,6}\s+(.*)$/;
const LIST_ITEM = /^[-*]\s+(.*)$/;
const ORDERED_ITEM = /^\d+[.)]\s+(.*)$/;
const FENCE = /^```/;
const INLINE = /(`[^`]+`|\*\*[^*]+\*\*)/;

export function parseMarkdown(source: string): Block[] {
  const blocks: Block[] = [];
  // Lines of the open code block, null outside one.
  let code: string[] | null = null;
  for (const rawLine of source.split(/\r?\n/)) {
    if (code !== null) {
      if (FENCE.test(rawLine.trim())) {
        blocks.push({ kind: 'code', text: code.join('\n') });
        code = null;
      } else {
        code.push(rawLine);
      }
      continue;
    }
    const line = rawLine.trim();
    if (line === '') continue;
    if (FENCE.test(line)) {
      code = [];
      continue;
    }
    const heading = HEADING.exec(line);
    const item = LIST_ITEM.exec(line);
    const ordered = ORDERED_ITEM.exec(line);
    const last = blocks.at(-1);
    if (heading !== null) {
      blocks.push({ kind: 'heading', content: parseInline(heading[1] ?? '') });
    } else if (item !== null || ordered !== null) {
      const isOrdered = item === null;
      const content = parseInline((item ?? ordered)?.[1] ?? '');
      if (last?.kind === 'list' && last.ordered === isOrdered) last.items.push(content);
      else blocks.push({ kind: 'list', ordered: isOrdered, items: [content] });
    } else {
      blocks.push({ kind: 'paragraph', content: parseInline(line) });
    }
  }
  // An answer cut off inside a block still shows its code.
  if (code !== null) blocks.push({ kind: 'code', text: code.join('\n') });
  return blocks;
}

export function parseInline(text: string): Inline[] {
  return text
    .split(INLINE)
    .filter((part) => part !== '')
    .map((part) => {
      if (part.startsWith('`') && part.endsWith('`') && part.length > 2) {
        return { kind: 'code', text: part.slice(1, -1) };
      }
      if (part.startsWith('**') && part.endsWith('**') && part.length > 4) {
        return { kind: 'strong', text: part.slice(2, -2) };
      }
      return { kind: 'text', text: part };
    });
}
