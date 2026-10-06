import { defineConfig } from '@playwright/test';

// E2E PRAWDZIWEJ aplikacji (PLAN §4.4): Playwright łączy się przez CDP z WebView2 uruchomionej
// `alfa-desktop.exe` (build z cechą `e2e`) — bez serwera WWW i bez przeglądarki Playwrighta.
// Uruchamia `e2e-app/run.ps1` (start aplikacji, czekanie na port CDP, sprzątanie). Testy na
// atrapie backendu w Chromium: `playwright.config.ts` (`e2e/`).
const out = process.env['ALFA_E2E_OUT'] ?? 'e2e-app/out';

export default defineConfig({
  testDir: './e2e-app',
  globalSetup: './e2e-app/global-setup.ts',
  outputDir: `${out}/test-results`,
  // Jedna aplikacja, jeden zestaw okien — testy po kolei w jednym procesie roboczym.
  workers: 1,
  fullyParallel: false,
  retries: 0,
  timeout: 120_000,
  expect: { timeout: 15_000 },
  reporter: [
    ['list'],
    ['html', { outputFolder: `${out}/report`, open: 'never' }],
    ['json', { outputFile: `${out}/results.json` }],
  ],
});
