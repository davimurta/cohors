import { describe, expect, it } from 'vitest';
import { parseInline, parseMarkdown } from './markdown.ts';

describe('parseMarkdown', () => {
  it('reads the summary sections', () => {
    const blocks = parseMarkdown('## O que foi feito\n- Criou `soma`\n- Testou\n\n## Pendente\n- Nada.');
    expect(blocks.map((block) => block.kind)).toEqual(['heading', 'list', 'heading', 'list']);
    const list = blocks[1];
    expect(list?.kind === 'list' && list.items.length).toBe(2);
  });

  it('keeps code blocks verbatim, even when cut off', () => {
    expect(parseMarkdown('Rode:\n```sh\n  npm test\n# done\n```\nOk')).toEqual([
      { kind: 'paragraph', content: [{ kind: 'text', text: 'Rode:' }] },
      { kind: 'code', text: '  npm test\n# done' },
      { kind: 'paragraph', content: [{ kind: 'text', text: 'Ok' }] },
    ]);
    expect(parseMarkdown('```\nfn main()')).toEqual([{ kind: 'code', text: 'fn main()' }]);
  });

  it('separates numbered lists from bullet lists', () => {
    const blocks = parseMarkdown('1. Um\n2) Dois\n- Solto');
    expect(blocks.map((block) => block.kind === 'list' && [block.ordered, block.items.length])).toEqual([
      [true, 2],
      [false, 1],
    ]);
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
