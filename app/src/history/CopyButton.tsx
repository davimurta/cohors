import { useEffect, useState } from 'react';
import { Check, Copy } from 'lucide-react';
import { Button } from '../components/Button.tsx';

const COPIED_MS = 2000;

export function CopyButton({ text, label }: { text: string; label: string }) {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), COPIED_MS);
    return () => clearTimeout(timer);
  }, [copied]);

  const copy = () => {
    void navigator.clipboard.writeText(text).then(() => setCopied(true));
  };

  return (
    <Button
      icon={copied ? <Check size={16} aria-hidden="true" /> : <Copy size={16} aria-hidden="true" />}
      onClick={copy}
      aria-live="polite"
    >
      {copied ? 'Copiado' : label}
    </Button>
  );
}
