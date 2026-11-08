import { expect, type Page, test } from '@playwright/test';
import { openApp, waitForChat } from './helpers';

// katalog-przeglad: akcja/paleta
// Przegląd ogólny katalogu funkcji (docs/plan/45-testy-ai.md §4): KAŻDE polecenie palety Ctrl+K —
// także dodane po napisaniu tego testu — uruchamiane na świeżym oknie z atrapą. Lista pochodzi
// z samej palety (pozycje `cmd:*`), nie z kopii w teście. Polecenie nie może wywołać wyjątku
// strony ani nieobsłużonego odrzucenia, a okno ma po nim nadal przyjmować klawiaturę (Esc wraca,
// paleta otwiera się ponownie). Działanie konkretnych poleceń sprawdzają testy szczegółowe.

const UNHANDLED = '[alfa] nieobsłużone odrzucenie';
/**
 * Polecenia otwierające przebieg, w którym paleta jest celowo niedostępna (sprawdzamy tylko brak
 * błędów strony). Kreator pierwszego uruchomienia dopuszcza wyłącznie `ONBOARDING_ALLOWED`.
 */
const MODAL_FLOWS: Record<string, string> = {
  'cmd:action.onboarding': 'kreator pierwszego uruchomienia',
};
/** Ile przebiegów dzieli listę (każdy przebieg = osobny test; równoległe w CI). */
const PARTS = 4;

test.describe.configure({ timeout: 300_000 });

async function openPalette(page: Page) {
  // Po klasie, nie po nazwie: `action.language` przełącza język interfejsu.
  const dialog = page.locator('dialog.alfa-palette');
  await expect(async () => {
    if (!(await dialog.isVisible())) await page.keyboard.press('Control+k');
    await expect(dialog).toBeVisible({ timeout: 2_000 });
  }).toPass({ timeout: 15_000 });
  return dialog;
}

async function paletteCommands(page: Page): Promise<string[]> {
  const dialog = await openPalette(page);
  const values = await dialog
    .locator('.alfa-palette-item')
    .evaluateAll((items) => items.map((i) => i.getAttribute('data-value') ?? ''));
  await page.keyboard.press('Escape');
  return values.filter((v) => v.startsWith('cmd:'));
}

async function fresh(page: Page) {
  await openApp(page);
  await waitForChat(page);
}

for (let part = 0; part < PARTS; part++) {
  test(`każde polecenie palety działa bez błędu — część ${part + 1}/${PARTS}`, async ({ page }) => {
    const crashes: string[] = [];
    page.on('pageerror', (e) => crashes.push(e.message));
    page.on('console', (m) => {
      if (m.text().startsWith(UNHANDLED)) crashes.push(m.text());
    });
    await fresh(page);
    const all = await paletteCommands(page);
    // Paleta pokazuje polecenia aplikacji (bez klawiszy pola pisania) i akcje dodatkowe.
    expect(all.length, 'polecenia w palecie').toBeGreaterThan(40);
    const mine = all.filter((_, i) => i % PARTS === part);
    const failed: string[] = [];
    for (const value of mine) {
      crashes.length = 0;
      await fresh(page);
      const dialog = await openPalette(page);
      await dialog.locator(`.alfa-palette-item[data-value="${value}"]`).click();
      await page.waitForTimeout(300);
      // Powrót: Esc zamyka nakładki i okna; potem paleta musi się znów otworzyć.
      for (let i = 0; i < 3; i++) await page.keyboard.press('Escape');
      const again =
        value in MODAL_FLOWS ||
        (await openPalette(page).then(
          () => true,
          () => false,
        ));
      if (crashes.length > 0 || !again) {
        failed.push(`${value}: ${crashes.join(' | ') || 'paleta nie otwiera się ponownie'}`);
      }
    }
    expect(failed, 'polecenia z błędem').toEqual([]);
  });
}
