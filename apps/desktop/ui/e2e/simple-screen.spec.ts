import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// Uproszczony ekran (`ui.simple`, etap K.3): mniej przycisków dla dziecka. Niezmiennik
// bezpieczeństwa: wskaźnik mikrofonu, tryb pracy w pasku pisania, znak „Pełny dostęp” w belce
// i Ustawienia są widoczne zawsze. To wygoda, nie blokada — skróty klawiszowe dalej działają.

async function open(page: Page, query: string, theme: 'light' | 'dark' = 'light') {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?${query}&theme=${theme}`);
  await waitForChat(page);
}

const agentMessage = (page: Page) => page.locator('article.msg:not(.user)').last();

test('uproszczony ekran: mniej przycisków, wskaźniki bezpieczeństwa zostają', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await open(page, 'simple=1&style=junior&autonomy=L4');
  // Schowane: przełączniki paneli, panele i ich znaczniki, Pipeline'y, wybór agentki.
  for (const name of [
    'Pokaż panel Sesje',
    'Ukryj panel Sesje',
    'Pokaż panel prawy',
    'Ukryj panel prawy',
  ]) {
    await expect(page.getByRole('button', { name, exact: true })).toHaveCount(0);
  }
  await expect(page.locator('aside.left, aside.right')).toHaveCount(0);
  await expect(page.locator('.edge-tab')).toHaveCount(0);
  const composer = page.locator('form.composer');
  await expect(composer.getByRole('button', { name: 'Pipeline’y modeli' })).toHaveCount(0);
  await expect(composer.getByRole('button', { name: /^Adresatka:/ })).toHaveCount(0);
  // Akcje wiadomości agentki: tylko „Kopiuj” i „Przeczytaj na głos”.
  const message = agentMessage(page);
  await message.hover();
  const actions = message.getByRole('toolbar', { name: 'Akcje wiadomości' });
  await expect(actions.getByRole('button')).toHaveText(['', '']);
  await expect(actions.getByRole('button', { name: 'Kopiuj (Markdown)' })).toBeVisible();
  await expect(actions.getByRole('button', { name: 'Przeczytaj na głos' })).toBeVisible();
  for (const name of ['Ponów (nowy wariant)', 'Więcej akcji']) {
    await expect(actions.getByRole('button', { name })).toHaveCount(0);
  }
  // Zawsze widoczne: mikrofon, tryb pracy (także znak Pełnego dostępu), Ustawienia.
  await expect(page.getByRole('button', { name: /^Mikrofon:/ })).toBeVisible();
  await expect(composer.getByRole('button', { name: /^Tryb pracy: Pełny dostęp/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /^Włączony Pełny dostęp/ })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Ustawienia (Ctrl+,)' })).toBeVisible();
  // Wygoda, nie blokada: skrót otwiera panel Sesje mimo schowanych przycisków.
  await page.keyboard.press('Control+b');
  await expect(page.getByRole('complementary', { name: 'Sesje', exact: true })).toBeVisible();
  expect(errors).toEqual([]);
});

test('uproszczony ekran: przełącznik w Wyglądzie przywraca wszystkie przyciski', async ({
  page,
}) => {
  await open(page, 'simple=1');
  await expect(page.getByRole('button', { name: 'Ukryj panel Sesje' })).toHaveCount(0);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Wygląd' })
    .click();
  const toggle = page.getByRole('switch', { name: 'Uproszczony ekran' });
  await expect(toggle).toBeChecked();
  await toggle.click();
  await expect(toggle).not.toBeChecked();
  await page.keyboard.press('Control+,');
  const composer = page.locator('form.composer');
  await expect(composer.getByRole('button', { name: /^Adresatka:/ })).toBeVisible();
  const message = agentMessage(page);
  await message.hover();
  await expect(message.getByRole('button', { name: 'Więcej akcji' })).toBeVisible();
});

for (const theme of ['light', 'dark'] as const) {
  test(`uproszczony ekran Junior ${theme}: axe bez naruszeń, czytelny`, async ({ page }) => {
    await open(page, 'simple=1&style=junior', theme);
    await agentMessage(page).hover();
    await page.screenshot({ path: `test-results/junior-uproszczony-${theme}.png` });
    await expectAccessible(page, `uproszczony ekran junior ${theme}`);
    await expectReadable(page, `uproszczony ekran junior ${theme}`);
  });
}
