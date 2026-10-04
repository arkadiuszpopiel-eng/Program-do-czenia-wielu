import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

async function openModels(page: Page) {
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Modele i silniki' })
    .click();
  await expect(page.getByRole('heading', { name: 'Modele i silniki', level: 2 })).toBeVisible();
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
  await page.getByLabel('Rodzaj', { exact: true }).selectOption('vad');
  await expect(page.getByText('1 pozycja')).toBeVisible();
  const vad = row(page, 'Silero VAD 6.2.3 (op18, bez If)');
  await expect(vad.getByText('SHA-256 przypięty')).toBeVisible();
  await vad.getByRole('button', { name: 'Pobierz' }).click();
  await vad.getByRole('button', { name: 'Przerwij' }).click();
  await expect(vad.getByText('Wstrzymano — możesz wznowić')).toBeVisible();
  await vad.getByRole('button', { name: 'Wznów' }).click();
  await expect(vad.getByText('Zainstalowano i sprawdzono')).toBeVisible({ timeout: 10_000 });
  await page.getByLabel('Stan', { exact: true }).selectOption('installed');
  await expect(page.getByText('1 pozycja')).toBeVisible();
  await vad.getByRole('button', { name: 'Usuń' }).click();
  const confirm = page.getByRole('alertdialog', {
    name: 'Usunąć „Silero VAD 6.2.3 (op18, bez If)”?',
  });
  await confirm.getByRole('button', { name: 'Usuń' }).click();
  await expect(page.getByText('Brak pozycji dla wybranych filtrów.')).toBeVisible();
});

test('offline: pobieranie kończy się czytelnym błędem', async ({ page }) => {
  await openApp(page, { scenario: 'offline' });
  await waitForChat(page);
  await openModels(page);
  const vad = row(page, 'Silero VAD 6.2.3 (op18, bez If)');
  await vad.getByRole('button', { name: 'Pobierz' }).click();
  await expect(vad.getByText('sieć: brak połączenia (atrapa)')).toBeVisible();
});
