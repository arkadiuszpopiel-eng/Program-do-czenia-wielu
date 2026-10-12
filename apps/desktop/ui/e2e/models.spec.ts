import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, pickOption, waitForChat } from './helpers';

async function openModels(page: Page) {
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Modele lokalne i silniki' })
    .click();
  await expect(
    page.getByRole('heading', { name: 'Modele lokalne i silniki', level: 2 }),
  ).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Pozycje katalogu' })).toBeVisible();
}

const row = (page: Page, name: string) =>
  page.getByRole('listitem').filter({ has: page.getByRole('heading', { name, level: 4 }) });

for (const theme of ['light', 'dark'] as const) {
  test.describe(`modele i silniki — motyw ${theme}`, () => {
    test('pobierz → zgoda TOFU z hashem → zainstaluj → wyszukiwanie semantyczne', async ({
      page,
    }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await openModels(page);
      await expect(page.getByText('Używany model: Leksykalne (bez modelu)')).toBeVisible();
      await expectAccessible(page, `modele ${theme}`);

      const e5 = row(page, 'Multilingual E5 small (ONNX fp32)');
      await expect(e5.getByText('Do potwierdzenia przez człowieka')).toBeVisible();
      await e5.getByRole('button', { name: 'Pobierz' }).click();
      await expect(
        e5.getByRole('progressbar', { name: 'Pobieranie: Multilingual E5 small (ONNX fp32)' }),
      ).toBeVisible();
      await expectAccessible(page, `pobieranie ${theme}`);
      await expect(e5.getByText('Czeka na Twoją zgodę')).toBeVisible({ timeout: 10_000 });
      await expect(
        e5.getByRole('heading', { name: 'Plik bez przypiętej sumy kontrolnej' }),
      ).toBeVisible();
      await expect(e5.getByText('onnx/model.onnx', { exact: true })).toBeVisible();
      await expectAccessible(page, `zgoda ${theme}`);
      await e5.getByRole('button', { name: 'Ufam temu plikowi — zainstaluj' }).click();
      await expect(e5.getByText('Zainstalowano i sprawdzono')).toBeVisible();

      await e5.getByRole('button', { name: 'Używaj do wyszukiwania' }).click();
      await expect(
        page.getByText('Używany model: Multilingual E5 small (ONNX fp32)'),
      ).toBeVisible();
      await expect(e5.getByText('Używany w wyszukiwaniu')).toBeVisible();
      await expect(page.getByText(/Wektory są aktualne/)).toBeVisible({ timeout: 10_000 });
      await expectAccessible(page, `embedder ${theme}`);
    });
  });
}

test('filtry, przerwanie i wznowienie, usuwanie z potwierdzeniem', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await openModels(page);
  await pickOption(page.getByRole('combobox', { name: 'Rodzaj', exact: true }), 'vad');
  // Wykrywanie mowy (Silero) i koniec wypowiedzi (Smart Turn).
  await expect(page.getByText('2 pozycje')).toBeVisible();
  await expect(row(page, 'Smart Turn v3.2 (koniec wypowiedzi)')).toBeVisible();
  const vad = row(page, 'Silero VAD 6.2.3 (op18, bez If)');
  await expect(vad.getByText('SHA-256 przypięty')).toBeVisible();
  await vad.getByRole('button', { name: 'Pobierz' }).click();
  await vad.getByRole('button', { name: 'Przerwij' }).click();
  await expect(vad.getByText('Wstrzymano — możesz wznowić')).toBeVisible();
  await vad.getByRole('button', { name: 'Wznów' }).click();
  await expect(vad.getByText('Zainstalowano i sprawdzono')).toBeVisible({ timeout: 10_000 });
  await pickOption(page.getByRole('combobox', { name: 'Stan', exact: true }), 'installed');
  await expect(page.getByText('1 pozycja')).toBeVisible();
  await vad.getByRole('button', { name: 'Usuń' }).click();
  const confirm = page.getByRole('alertdialog', {
    name: 'Usunąć „Silero VAD 6.2.3 (op18, bez If)”?',
  });
  await confirm.getByRole('button', { name: 'Usuń' }).click();
  await expect(page.getByText('Brak pozycji dla wybranych filtrów.')).toBeVisible();
});

test('modele rozmowy mają ocenę dopasowania do karty graficznej', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await openModels(page);
  await pickOption(page.getByRole('combobox', { name: 'Rodzaj', exact: true }), 'llm');
  // Karta 6 GB z atrapy: mały Bielik mieści się cały, duży tylko częściowo.
  await expect(
    row(page, 'Bielik 1.5B v3.0 Instruct (Q8_0)').getByText('mieści się na karcie'),
  ).toBeVisible();
  await expect(
    row(page, 'Bielik 4.5B v3.0 Instruct (Q8_0)').getByText('częściowo na karcie — wolniej'),
  ).toBeVisible();
  // Inne rodzaje pozycji nie dostają tej oceny.
  await pickOption(page.getByRole('combobox', { name: 'Rodzaj', exact: true }), 'stt');
  await expect(page.getByText('mieści się na karcie')).toHaveCount(0);
});

test('offline: pobieranie kończy się czytelnym błędem', async ({ page }) => {
  await openApp(page, { scenario: 'offline' });
  await waitForChat(page);
  await openModels(page);
  const vad = row(page, 'Silero VAD 6.2.3 (op18, bez If)');
  await vad.getByRole('button', { name: 'Pobierz' }).click();
  await expect(vad.getByText('sieć: brak połączenia (atrapa)')).toBeVisible();
});

for (const style of ['classic', 'glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`katalog: wyszukiwanie, kolejność i miejsce na dysku — ${style} ${theme}`, async ({
      page,
    }) => {
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      await openModels(page);
      const list = page.locator('#mm-list').locator('xpath=..').locator('ul.list');
      const names = list.getByRole('heading', { level: 4 });
      const usage = page.getByTestId('models-usage');
      await expect(usage).toContainText(/Na dysku: .+ w \d+ pozycj/);
      await expect(usage).toContainText(/do pobrania: .+ w \d+ pozycj/);
      // Wyszukiwanie bez polskich znaków i wielkości liter; licznik idzie za wynikiem.
      const search = page.getByRole('searchbox', { name: 'Szukaj w katalogu' });
      await search.fill('BIELIK');
      await expect(names.first()).toContainText('Bielik');
      const found = await names.count();
      expect(found).toBeGreaterThan(0);
      for (const text of await names.allTextContents()) expect(text).toContain('Bielik');
      await search.fill('nie ma takiego modelu');
      await expect(page.getByText('Brak pozycji dla wybranych filtrów.')).toBeVisible();
      await search.fill('');
      // Kolejność: od największych — pierwsza pozycja nie jest mniejsza od drugiej.
      await pickOption(page.getByRole('combobox', { name: 'Kolejność', exact: true }), 'size_desc');
      await pickOption(page.getByRole('combobox', { name: 'Rodzaj', exact: true }), 'llm');
      const bySize = await names.allTextContents();
      await pickOption(page.getByRole('combobox', { name: 'Kolejność', exact: true }), 'size_asc');
      await expect(names.first()).toHaveText(bySize[bySize.length - 1] ?? '');
      expect(await names.allTextContents()).toEqual([...bySize].reverse());
      await pickOption(page.getByRole('combobox', { name: 'Kolejność', exact: true }), 'name');
      const byName = await names.allTextContents();
      expect(byName).toEqual([...byName].sort((a, b) => a.localeCompare(b, 'pl')));
      await expectAccessible(page, `katalog modeli ${style} ${theme}`);
    });
  }
}
