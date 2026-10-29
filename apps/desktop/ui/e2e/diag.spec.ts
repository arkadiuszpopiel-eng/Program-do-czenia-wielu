import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// Ustawienia → Diagnostyka komputera: ocena stanu, zapis parametrów, testy lekkie i wybrane dyski,
// wyniki z miarami, historia, raport. Strona z wynikami w każdym stylu i motywie.

async function openDiag(page: Page, skin = 'classic', theme: 'light' | 'dark' = 'light') {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?style=${skin}&theme=${theme}`);
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Diagnostyka komputera' })
    .click();
  await expect(
    page.getByRole('heading', { name: 'Diagnostyka komputera', level: 2 }),
  ).toBeVisible();
}

test('ocena stanu i parametry z ostatniego zapisu', async ({ page }) => {
  await openDiag(page);
  const assessment = page.getByRole('region', { name: /Ocena stanu/ });
  await expect(assessment.getByText('Wykryto problemy')).toBeVisible();
  await expect(assessment.getByText(/0x116 VIDEO_TDR_FAILURE/)).toBeVisible();
  await expect(assessment.getByText(/Mostek USB NVMe \(E:\): 200 błędów/)).toBeVisible();
  const params = page.getByRole('region', { name: 'Parametry' });
  await expect(params.getByText('Procesor 10C/16T')).toBeVisible();
  await expect(params.getByText('200 błędów w dzienniku')).toBeVisible();
  await expect(params.getByText(/Niebieskie ekrany: 2/)).toBeVisible();
  await expect(params.getByText(/liczniki SMART/)).toBeVisible();
  await page.getByRole('button', { name: 'Zapisz parametry teraz' }).click();
  await expect(page.getByText(/w historii: 1 zapisów/)).toBeVisible();
});

test('testy lekkie, wybrany dysk, historia i raport', async ({ page }) => {
  await openDiag(page);
  const tests = page.getByRole('region', { name: 'Testy' });
  await tests.getByRole('button', { name: 'Uruchom testy lekkie' }).click();
  await expect(tests.getByText(/Ostatnie testy .* — Wykryto problemy/)).toBeVisible();
  await expect(tests.getByText(/Kopiowanie: 11,2 GB\/s/)).toBeVisible();
  const run = tests.getByRole('button', { name: 'Uruchom wybrane' });
  await expect(run).toBeDisabled();
  await tests.getByRole('checkbox', { name: 'Dysk E: — zapis, odczyt, weryfikacja' }).check();
  await run.click();
  await expect(tests.getByText(/Zapis sekwencyjny: 140 MB\/s/)).toBeVisible();
  await expect(tests.getByText(/Ostatnie testy .* — Wymaga uwagi/)).toBeVisible();
  const history = page.getByRole('region', { name: 'Historia testów' });
  await expect(history.getByRole('listitem')).toHaveCount(2);
  await page.getByRole('button', { name: 'Zapisz raport do pliku' }).click();
  await expect(page.getByText(/Raport zapisano: .*Diagnostyka/)).toBeVisible();
});

for (const skin of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`strona z wynikami: ${skin}, ${theme}`, async ({ page }) => {
      await openDiag(page, skin, theme);
      await page
        .getByRole('region', { name: 'Testy' })
        .getByRole('button', { name: 'Uruchom testy lekkie' })
        .click();
      await expect(page.getByText(/Ostatnie testy/)).toBeVisible();
      await expectAccessible(page, `diagnostyka ${skin} ${theme}`);
      await expectReadable(page, `diagnostyka ${skin} ${theme}`);
    });
  }
}
