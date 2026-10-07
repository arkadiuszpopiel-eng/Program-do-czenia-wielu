import { defineConfig, devices } from '@playwright/test';

// Testy E2E na zbudowanym UI (`pnpm build`) z atrapą backendu (`?scenario=…`):
// axe (WCAG 2.2 AA — 0 naruszeń critical/serious), klawiatura, budżety klatek.
export default defineConfig({
  testDir: './e2e',
  timeout: 30_000,
  fullyParallel: true,
  reporter: [['list']],
  use: {
    baseURL: 'http://127.0.0.1:4173',
    viewport: { width: 1280, height: 800 },
    locale: 'pl-PL',
  },
  projects: [
    {
      name: 'chromium',
      // PW_CHANNEL=msedge: systemowy Edge (silnik WebView2) zamiast pobranej przeglądarki Playwrighta.
      use: {
        ...devices['Desktop Chrome'],
        viewport: { width: 1280, height: 800 },
        ...(process.env['PW_CHANNEL'] ? { channel: process.env['PW_CHANNEL'] } : {}),
      },
    },
  ],
  webServer: {
    command: 'pnpm exec vite preview --port 4173 --strictPort --host 127.0.0.1',
    url: 'http://127.0.0.1:4173',
    reuseExistingServer: !process.env['CI'],
    timeout: 60_000,
  },
});
