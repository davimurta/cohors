// The subset of Markdown the summary prompt asks for: headings, "-" lists
// and paragraphs, with inline `code` and **bold**.

export type Inline = { kind: 'text' | 'code' | 'strong'; text: string };

export type Block =
  | { kind: 'heading'; content: Inline[] }
  | { kind: 'list'; items: Inline[][] }
  | { kind: 'paragraph'; content: Inline[] };

const HEADING = /^#{1,6}\s+(.*)$/;
const LIST_ITEM = /^[-*]\s+(.*)$/;
const INLINE = /(`[^`]+`|\*\*[^*]+\*\*)/;

export function parseMarkdown(source: string): Block[] {
  const blocks: Block[] = [];
  for (const rawLine of source.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line === '') continue;
    const heading = HEADING.exec(line);
    const item = LIST_ITEM.exec(line);
    const last = blocks.at(-1);
    if (heading !== null) {
      blocks.push({ kind: 'heading', content: parseInline(heading[1] ?? '') });
    } else if (item !== null) {
      const content = parseInline(item[1] ?? '');
      if (last?.kind === 'list') last.items.push(content);
      else blocks.push({ kind: 'list', items: [content] });
    } else {
      blocks.push({ kind: 'paragraph', content: parseInline(line) });
    }
  }
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
