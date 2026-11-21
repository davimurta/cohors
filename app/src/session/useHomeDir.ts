import { useEffect, useState } from 'react';
import { homeDir } from '@tauri-apps/api/path';

let cached: Promise<string | null> | null = null;

export function useHomeDir(): string | null {
  const [home, setHome] = useState<string | null>(null);
  useEffect(() => {
    cached ??= homeDir().catch(() => null);
    let active = true;
    void cached.then((value) => {
      if (active) setHome(value);
    });
    return () => {
      active = false;
    };
  }, []);
  return home;
}
