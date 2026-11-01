// Eksport .alfa przez systemowe okno „Zapisz jako” (UI Automation Windows) w PRAWDZIWEJ aplikacji. Okno
// systemowe wychodzi na pierwszy plan, więc test biegnie tylko z oknami na ekranie (`ALFA_E2E_VISIBLE=1`,
// gdy właściciel nie pracuje); bez tego jest pomijany (instancja testowa poza ekranem, docs/plan/45 §6.3a).
// Wydzielony z `smoke.spec.ts` 1.11 (limit rozmiaru pliku).
import { mkdirSync, readFileSync, rmSync } from 'node:fs';
import { basename, join } from 'node:path';
import type { Page } from '@playwright/test';
import { OUT, driveSaveDialog, expect, HIDDEN, shot, test } from './alfa';

test.beforeEach(({ alfa }, info) => {
  alfa.log.phase = info.title;
});

/** Ustawienia → Import i eksport (z rozmowy albo innej strony Ustawień). */
async function openExport(page: Page): Promise<void> {
  const back = page.getByRole('button', { name: 'Wróć do rozmowy' });
  await expect(page.locator('#alfa-composer').or(back).first()).toBeVisible({ timeout: 60_000 });
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  if (!(await nav.isVisible())) await page.keyboard.press('Control+,');
  await nav.getByRole('button', { name: 'Import i eksport', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Import i eksport', level: 2 })).toBeVisible();
}

// Okno systemowe obsługuje UI Automation i potrzebuje pierwszego planu: gdy ktoś w tej chwili
// pracuje na pulpicie, pierwsza próba bywa nieudana — powtórka jest raportowana jako „flaky”.
test.describe('eksport przez okno systemowe', () => {
  test.describe.configure({ retries: 2 });
  test('eksport .alfa do katalogu tymczasowego (natywne okno „Zapisz jako”)', async ({ alfa }) => {
    test.skip(process.platform !== 'win32', 'okno „Zapisz jako” obsługuje UI Automation Windows');
    test.skip(HIDDEN, 'okno systemowe wychodzi na pierwszy plan — tylko z ALFA_E2E_VISIBLE=1');
    const { main } = alfa;
    await openExport(main);
    const dir = process.env['ALFA_E2E_EXPORT_DIR'] ?? join(OUT, 'export');
    mkdirSync(dir, { recursive: true });
    const target = join(dir, `alfa-e2e-${Date.now()}.alfa`);
    const dialog = driveSaveDialog(target);
    await main.getByRole('button', { name: 'Eksportuj…', exact: true }).click();
    const result = await dialog;
    await test
      .info()
      .attach('okno-zapisz-jako.log', { body: result.output, contentType: 'text/plain' });
    try {
      expect(result.code, `UI Automation okna „Zapisz jako”:\n${result.output}`).toBe(0);
      const saved = main.getByRole('status').filter({ hasText: /^Zapisano / });
      await expect(saved).toBeVisible({ timeout: 60_000 });
      // Ścieżka z komunikatu rdzenia („Zapisano <ścieżka> · …”) — ta, którą zwróciło okno systemowe.
      const status = (await saved.textContent()) ?? '';
      const written = /^Zapisano (.+?\.alfa)/.exec(status)?.[1] ?? '';
      expect(written, `komunikat: ${status}`).not.toBe('');
      if (basename(written) !== basename(target)) {
        // Okno przyjęło inną nazwę niż wpisana (np. inne wydanie Windows) — eksport i tak sprawdzamy.
        test.info().annotations.push({
          type: 'warning',
          description: `okno zapisu zwróciło ${written} zamiast ${target}`,
        });
      }
      const head = readFileSync(written).subarray(0, 2).toString('latin1');
      expect(head, 'paczka .alfa to archiwum ZIP').toBe('PK');
      if (written !== target) rmSync(written, { force: true });
    } finally {
      // Okno, którego nie udało się obsłużyć, nie może zostać otwarte dla kolejnych testów.
      if (result.code !== 0) await driveSaveDialog(null, 5);
    }
    await shot(alfa, '07-eksport');
  });
});
