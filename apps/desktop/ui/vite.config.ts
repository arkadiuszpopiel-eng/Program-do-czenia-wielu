import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const host = process.env['TAURI_DEV_HOST'];

// CSP wydania (`app.security.csp` z tauri.conf.json) jako nagłówek `vite preview` — E2E
// (Playwright) działa pod tą samą polityką co okno Tauri, więc regresja Trusted Types albo
// stylów inline wychodzi w CI, a nie dopiero na Windows (PT-33).
function releaseCsp(): string {
  const conf = JSON.parse(
    readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8'),
  ) as { app: { security: { csp: Record<string, string> } } };
  return Object.entries(conf.app.security.csp)
    .map(([directive, value]) => `${directive} ${value}`)
    .join('; ');
}

// Czysty CSR pod Tauri 2: brak SSR, stały port, bez czyszczenia ekranu (logi Rust w tej samej konsoli).
export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: {
      $lib: fileURLToPath(new URL('./src/lib', import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host ?? false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  preview: {
    headers: { 'Content-Security-Policy': releaseCsp() },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    // WebView2 = Chromium; celem jest Windows 11 → nowoczesny target bez polyfilli.
    target: 'chrome120',
    minify: 'esbuild',
    sourcemap: false,
    cssMinify: true,
    reportCompressedSize: true,
    // Ostrzeżenie Vite liczy rozmiar przed gzip; twardy budżet gzip egzekwuje scripts/bundle-size.mjs.
    chunkSizeWarningLimit: 600,
    // Manifest: skrypt budżetu liczy statyczny graf importów każdego punktu wejścia.
    manifest: true,
    // Bez wstępnego ładowania dynamicznych modułów przez helper Vite — leniwe znaczy leniwe.
    modulePreload: { polyfill: false },
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL('./index.html', import.meta.url)),
        quick: fileURLToPath(new URL('./quick.html', import.meta.url)),
        pill: fileURLToPath(new URL('./pill.html', import.meta.url)),
      },
    },
  },
  worker: { format: 'es' },
});
