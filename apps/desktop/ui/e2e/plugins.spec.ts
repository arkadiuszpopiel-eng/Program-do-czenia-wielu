import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

/** Najmniejszy nagłówek komponentu Wasm (`\0asm`, warstwa komponentu). */
const COMPONENT = Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00]);
/** Moduł rdzeniowy (np. z importami WASI) — nie komponent. */
const CORE_MODULE = Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

const MANIFEST = JSON.stringify({
  id: 'notatnik',
  version: '1.0.0',
  author: 'Właściciel',
  description: 'Dopisuje notatki do pliku.',
  wasm_sha256: '0'.repeat(64),
  capabilities: [
    { cap: 'fs.write', scope: { path: 'C:\\Users\\Ty\\Documents\\Notatki', subtree: true } },
  ],
  tools: [
    {
      name: 'append_note',
      title: 'Dopisz notatkę',
      description: 'Dopisuje tekst na końcu pliku notatek.',
      input_schema: { type: 'object', additionalProperties: false },
      output_schema: { type: 'object' },
    },
  ],
});

async function openPlugins(page: Page, theme: 'light' | 'dark') {
  await openApp(page, { theme });
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Wtyczki' })
    .click();
  await expect(page.getByRole('heading', { name: 'Wtyczki', level: 2 })).toBeVisible();
}

for (const theme of ['light', 'dark'] as const) {
  test.describe(`wtyczki — motyw ${theme}`, () => {
    test('karta propozycji → zatwierdzenie kliknięciem → wyłącz/włącz z hashem', async ({
      page,
    }) => {
      await openPlugins(page, theme);
      await expect(page.getByRole('heading', { name: 'Wtyczki Wasm' })).toBeVisible();
      await expect(page.getByText('kurs-walut 0.2.0')).toBeVisible();
      await expect(
        page.getByRole('region', { name: 'Zainstalowane' }).getByText('licznik-slow 1.0.0'),
      ).toBeVisible();
      await expect(page.getByText('Przerwana przez piaskownicę')).toBeVisible();
      await expectAccessible(page, `wtyczki ${theme}`);

      await page.getByRole('button', { name: 'Przejrzyj: kurs-walut 0.2.0' }).click();
      const card = page.getByRole('region', { name: 'Przegląd wtyczki kurs-walut 0.2.0' });
      await expect(card).toBeVisible();
      await expect(card.getByText('Połączenia z hostem')).toBeVisible();
      await expect(card.getByText('api.nbp.pl', { exact: true })).toBeVisible();
      await expect(card.getByText('16 MiB')).toBeVisible();
      await expect(card.getByText(/plugins\.kurs_walut\.version/)).toBeVisible();
      await expect(card.getByTestId('review-hash')).toHaveText(/^[0-9a-f]{64}$/);
      await expectAccessible(page, `karta ${theme}`);
      await card.getByRole('button', { name: 'Zainstaluj' }).click();
      await expect(
        page.getByText('Wtyczka zainstalowana — agentki widzą jej narzędzia.'),
      ).toBeVisible();
      await expect(page.getByText('Brak propozycji.')).toBeVisible();

      await page.getByRole('button', { name: 'Wyłącz: licznik-slow 1.0.0' }).click();
      await expect(page.getByRole('button', { name: 'Włącz…: licznik-slow 1.0.0' })).toBeVisible();
      await page.getByRole('button', { name: 'Włącz…: licznik-slow 1.0.0' }).click();
      const again = page.getByRole('region', { name: 'Ponowne włączenie: licznik-slow 1.0.0' });
      await again.getByRole('button', { name: 'Włącz ponownie' }).click();
      await expect(page.getByRole('button', { name: 'Wyłącz: licznik-slow 1.0.0' })).toBeVisible();
    });

    test('Zdrowie systemu: karta R2 i problemy wtyczek → strona „Wtyczki”', async ({ page }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await page.keyboard.press('Control+,');
      await page
        .getByRole('navigation', { name: 'Sekcje ustawień' })
        .getByRole('button', { name: 'Zdrowie systemu' })
        .click();
      const card = page.getByRole('region', { name: 'Wtyczki: propozycje i problemy' });
      await expect(card).toBeVisible();
      await expect(card.getByText(/plugins\.kurs_walut\.version/)).toBeVisible();
      await expect(card.getByText('1 problem wtyczki w ostatnim czasie.')).toBeVisible();
      await expectAccessible(page, `zdrowie wtyczki ${theme}`);
      await card.getByRole('button', { name: 'Przejrzyj w „Wtyczkach”' }).click();
      await expect(page.getByRole('heading', { name: 'Wtyczki', level: 2 })).toBeVisible();
    });

    test('dodanie: kontrola modułu → propozycja → odrzucenie; moduł rdzeniowy odrzucony', async ({
      page,
    }) => {
      await openPlugins(page, theme);
      const module = page.getByLabel('Moduł (.wasm)');
      await module.setInputFiles({
        name: 'zly.wasm',
        mimeType: 'application/wasm',
        buffer: CORE_MODULE,
      });
      await expect(page.getByText(/Moduł odrzucony: to nie jest komponent Wasm/)).toBeVisible();
      await expect(page.getByRole('button', { name: 'Zaproponuj' })).toBeDisabled();

      await module.setInputFiles({
        name: 'notatnik.wasm',
        mimeType: 'application/wasm',
        buffer: COMPONENT,
      });
      await expect(page.getByText(/Moduł jest poprawnym komponentem/)).toBeVisible();
      await page.getByLabel('Manifest (JSON)').fill(MANIFEST);
      await expectAccessible(page, `dodawanie ${theme}`);
      await page.getByRole('button', { name: 'Zaproponuj' }).click();
      const card = page.getByRole('region', { name: 'Przegląd wtyczki notatnik 1.0.0' });
      await expect(card).toBeVisible();
      await expect(card.getByText('Zapis plików')).toBeVisible();
      await expect(card.getByText(/może zapisywać pliki/)).toBeVisible();
      await card.getByRole('button', { name: 'Odrzuć' }).click();
      await expect(page.getByText('notatnik 1.0.0 — odrzucona')).toBeVisible();
    });
  });
}
