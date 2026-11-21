import { fileURLToPath } from 'node:url';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@cohors/protocol': fileURLToPath(new URL('../protocol/index.ts', import.meta.url)),
    },
  },
  // Tauri prints its own output and expects a fixed dev server port.
  clearScreen: false,
  server: {
    host: 'localhost',
    port: 5173,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] },
  },
});
