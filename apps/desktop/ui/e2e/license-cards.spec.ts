// Karty licencji (zadanie C5.7) w katalogu „Modele lokalne i silniki”: rozwijana karta z licencją,
// autorem, użyciem komercyjnym, wymogiem wskazania autora, uwagami (znak wodny mowy syntetycznej —
// decyzja właściciela w toku) i głosami pozycji — otwarta w każdym stylu i motywie, także po angielsku.
import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, openApp, waitForChat } from './helpers';

async function openModels(page: Page, nav: string, section: string, heading: string) {
  await page.keyboard.press('Control+,');
  await page.getByRole('navigation', { name: nav }).getByRole('button', { name: section }).click();
  await expect(page.getByRole('heading', { name: heading })).toBeVisible();
}

const row = (page: Page, name: string) =>
  page
    .getByRole('listitem')
    .filter({ has: page.getByRole('heading', { name, level: 4, exact: true }) });

for (const style of ['classic', 'glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`karta licencji: ${style}, ${theme}`, async ({ page }) => {
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      await openModels(page, 'Sekcje ustawień', 'Modele lokalne i silniki', 'Pozycje katalogu');

      const toggle = row(page, 'Supertonic 3').getByRole('button', { name: 'Karta licencji' });
      await expect(toggle).toHaveAttribute('aria-expanded', 'false');
      await toggle.click();
      await expect(toggle).toHaveAttribute('aria-expanded', 'true');
      const voice = page.getByRole('region', { name: 'Karta licencji: Supertonic 3' });
      for (const label of ['Licencja', 'Identyfikator', 'Autor i źródło', 'Użycie komercyjne']) {
        await expect(voice.getByText(label, { exact: true })).toBeVisible();
      }
      await expect(
        voice.getByText('Licencja z zapisem sprawdzenia w dokumentacji projektu'),
      ).toBeVisible();
      await expect(voice.getByText('Źródło licencji', { exact: true })).toBeVisible();
      await expect(voice.getByText('Tak, pod warunkami', { exact: true })).toBeVisible();
      await expect(voice.getByText(/udzielanie porad medycznych/)).toBeVisible();
      await expect(
        voice.getByText(/Znak wodny w mowie syntetycznej: decyzja właściciela w toku/),
      ).toBeVisible();
      await expect(voice.getByRole('heading', { name: 'Głosy (10)', level: 5 })).toBeVisible();
      await expect(voice.getByRole('listitem')).toHaveCount(10);
      await expect(voice.getByText('F1 — kobiecy, ciepły (domyślnie: Alfa)')).toBeVisible();
      await expect(voice.getByText('Zapis faktów z licencji, nie porada prawna.')).toBeVisible();

      // Sprawa otwarta: biblioteki NVIDIA — słowem, nie tylko kolorem.
      await row(page, 'llama-server (cuda)')
        .getByRole('button', { name: 'Karta licencji' })
        .click();
      const cuda = page.getByRole('region', { name: 'Karta licencji: llama-server (cuda)' });
      await expect(cuda.getByText('Licencja do potwierdzenia przez człowieka')).toBeVisible();
      await expect(cuda.getByText(/NVIDIA CUDA Toolkit EULA/)).toBeVisible();
      await expect(cuda.getByRole('heading', { level: 5 })).toHaveCount(0);

      await expectAccessible(page, `karta licencji ${style} ${theme}`);
      await expectReadable(page, `karta licencji ${style} ${theme}`);

      await toggle.click();
      await expect(toggle).toHaveAttribute('aria-expanded', 'false');
      await expect(voice).toHaveCount(0);
    });
  }
}

test('karta licencji po angielsku', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await page.keyboard.press('Control+k');
  await expect(page.locator('dialog[open] input')).toBeFocused();
  await page.keyboard.type('angielski');
  await expect(page.locator('dialog [role=option][data-selected]')).toContainText('angielski');
  await page.keyboard.press('Enter');
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  await openModels(page, 'Settings sections', 'Local models and engines', 'Catalog items');

  await row(page, 'Supertonic 3').getByRole('button', { name: 'Licence card' }).click();
  const voice = page.getByRole('region', { name: 'Licence card: Supertonic 3' });
  await expect(voice.getByText('Yes, under conditions', { exact: true })).toBeVisible();
  await expect(voice.getByRole('heading', { name: 'Voices (10)', level: 5 })).toBeVisible();
  await expect(voice.getByText(/Synthetic-speech watermark: owner decision pending/)).toBeVisible();
  await expectAccessible(page, 'karta licencji EN');
});

test('karta licencji na stronie „Głos”: modele mowy i ich barwy', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Głos', exact: true })
    .click();
  const licenses = page.locator('section[aria-labelledby="voice-licenses"]');
  await expect(licenses.getByRole('heading', { name: 'Licencje głosów' })).toBeVisible();
  await licenses
    .getByRole('listitem')
    .filter({ hasText: 'Supertonic 3' })
    .first()
    .getByRole('button', { name: 'Karta licencji' })
    .click();
  const card = page.getByRole('region', { name: 'Karta licencji: Supertonic 3' });
  await expect(card.getByRole('heading', { name: 'Głosy (10)', level: 5 })).toBeVisible();
  await expect(card.getByText('Tak, pod warunkami', { exact: true })).toBeVisible();
  await expectAccessible(page, 'licencje głosów');
});
