import { parseMarkdown, type Inline } from './markdown.ts';
import styles from './SummaryText.module.css';

export function SummaryText({ markdown }: { markdown: string }) {
  return (
    <div className={styles.summary}>
      {parseMarkdown(markdown).map((block, index) => {
        switch (block.kind) {
          case 'heading':
            return (
              <h3 key={index} className={styles.heading}>
                <InlineText parts={block.content} />
              </h3>
            );
          case 'list': {
            const List = block.ordered ? 'ol' : 'ul';
            return (
              <List key={index} className={styles.list}>
                {block.items.map((item, itemIndex) => (
                  <li key={itemIndex}>
                    <InlineText parts={item} />
                  </li>
                ))}
              </List>
            );
          }
          case 'code':
            return (
              <pre key={index} className={styles.codeBlock}>
                <code>{block.text}</code>
              </pre>
            );
          case 'paragraph':
            return (
              <p key={index} className={styles.paragraph}>
                <InlineText parts={block.content} />
              </p>
            );
        }
      })}
    </div>
  );
}

function InlineText({ parts }: { parts: Inline[] }) {
  return parts.map((part, index) => {
    if (part.kind === 'code') {
      return (
        <code key={index} className={styles.code}>
          {part.text}
        </code>
      );
    }
    if (part.kind === 'strong') return <strong key={index}>{part.text}</strong>;
    return part.text;
  });
}
