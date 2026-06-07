import { defineConfig } from 'vite';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const rootDir = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  root: '.',
  server: {
    port: 5173,
    open: true,
    fs: {
      allow: [rootDir, path.resolve(rootDir, '..')],
    },
  },
  worker: {
    format: 'es',
  },
});
