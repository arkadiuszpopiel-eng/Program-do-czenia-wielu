import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath } from 'node:url';

// Testy logiki (store'y z runami, i18n, bufor rAF, atrapa backendu, fuzzy, skróty).
// Testy dostępności i klawiatury w prawdziwym Chromium: `pnpm test:e2e` (Playwright + axe).
export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: { $lib: fileURLToPath(new URL('./src/lib', import.meta.url)) },
    conditions: ['browser'],
  },
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
    restoreMocks: true,
  },
});
