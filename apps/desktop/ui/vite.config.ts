// Vite builds the UI into static files that the Tauri shell serves; Vitest
// reuses the same configuration (docs/07-frontend-guidelines.md §2, §13).
import { fileURLToPath } from 'node:url';

import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: { $lib: fileURLToPath(new URL('./src/lib', import.meta.url)) },
  },
  // Tauri serves the files itself; relative paths keep the build portable.
  base: './',
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    // WebView2 is evergreen Chromium.
    target: 'chrome120',
    outDir: 'dist',
    emptyOutDir: true,
  },
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
});
