import { describe, expect, it } from 'vitest';
import { parseInline, parseMarkdown } from './markdown.ts';

describe('parseMarkdown', () => {
  it('reads the summary sections', () => {
    const blocks = parseMarkdown('## O que foi feito\n- Criou `soma`\n- Testou\n\n## Pendente\n- Nada.');
    expect(blocks.map((block) => block.kind)).toEqual(['heading', 'list', 'heading', 'list']);
    const list = blocks[1];
    expect(list?.kind === 'list' && list.items.length).toBe(2);
  });

  it('keeps plain lines as paragraphs', () => {
    expect(parseMarkdown('Texto livre')).toEqual([
      { kind: 'paragraph', content: [{ kind: 'text', text: 'Texto livre' }] },
    ]);
  });
});

describe('parseInline', () => {
  it('splits code and bold', () => {
    expect(parseInline('usa `seq` e **ring buffer**.')).toEqual([
      { kind: 'text', text: 'usa ' },
      { kind: 'code', text: 'seq' },
      { kind: 'text', text: ' e ' },
      { kind: 'strong', text: 'ring buffer' },
      { kind: 'text', text: '.' },
    ]);
  });

  it('leaves unmatched markers as text', () => {
    expect(parseInline('a ` b')).toEqual([{ kind: 'text', text: 'a ` b' }]);
  });
});
