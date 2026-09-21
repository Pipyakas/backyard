import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
  root: '.',
  // Do NOT use publicDir: server serves ./static at /static directly
  // (both in dev via manifest dir and in the image via dist/static copy).
  publicDir: false,
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
