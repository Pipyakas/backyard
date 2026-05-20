import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
  root: '.',
  build: {
    rollupOptions: {
      input: {
        index: resolve(__dirname, 'templates/index.html'),
        auth: resolve(__dirname, 'templates/auth.html'),
      },
    },
    outDir: 'dist',
    emptyOutDir: true,
  },
});
