import { readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import react from '@vitejs/plugin-react';
import { defineConfig, type Plugin } from 'vite';

// Matches the Rust `dirs::data_dir()` used by the daemon.
function daemonDataDir(): string {
  switch (process.platform) {
    case 'darwin':
      return join(homedir(), 'Library', 'Application Support', 'cohors');
    case 'win32':
      return join(process.env.APPDATA ?? join(homedir(), 'AppData', 'Roaming'), 'cohors');
    default:
      return join(process.env.XDG_DATA_HOME ?? join(homedir(), '.local', 'share'), 'cohors');
  }
}

// Dev only: a plain web page cannot read files, so the dev server hands it
// daemon.json. Vite's default CORS policy keeps other sites from reading it.
const daemonFile: Plugin = {
  name: 'cohors-daemon-file',
  apply: 'serve',
  configureServer(server) {
    server.middlewares.use('/daemon.json', (_request, response) => {
      readFile(join(daemonDataDir(), 'daemon.json'))
        .then((contents) => {
          response.setHeader('Content-Type', 'application/json');
          response.setHeader('Cache-Control', 'no-store');
          response.end(contents);
        })
        .catch(() => {
          response.statusCode = 503;
          response.end();
        });
    });
  },
};

export default defineConfig({
  plugins: [react(), daemonFile],
  resolve: {
    alias: {
      '@cohors/protocol': fileURLToPath(new URL('../protocol/index.ts', import.meta.url)),
    },
  },
  server: {
    host: 'localhost',
    // The daemon only accepts this origin.
    port: 5173,
    strictPort: true,
  },
});
