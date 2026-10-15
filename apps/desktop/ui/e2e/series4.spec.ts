import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// Seria 4 (0.0.62): podpowiedź „Bez modelu” pod polem pisania i przykłady poleceń wbudowanych
// wpisywane do testera — każdy stan w każdym stylu i motywie (dostępność i kontrast).

async function open(page: Page, skin = 'classic', theme: 'light' | 'dark' = 'light') {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?style=${skin}&theme=${theme}`);
  await waitForChat(page);
}

async function settings(page: Page, name: string) {
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name, exact: true })
    .click();
  await expect(page.getByRole('heading', { name, level: 2 })).toBeVisible();
}

test('pole pisania: dopisek „Bez modelu” tylko dla poleceń bez modelu', async ({ page }) => {
  await open(page);
  const composer = page.locator('#alfa-composer');
  await composer.fill('2+2');
  await expect(page.getByText('Bez modelu: rachunek')).toBeVisible();
  await expect(page.getByText(/Rozmawiasz ze sztuczną inteligencją/)).toBeVisible();
  await composer.fill('Opowiedz bajkę o smoku');
  await expect(page.getByText(/Bez modelu:/)).toHaveCount(0);
  await composer.fill('');
  await expect(page.getByText(/Bez modelu:/)).toHaveCount(0);
});

test('polecenia bez modelu: przykład wbudowany trafia do testera', async ({ page }) => {
  await open(page);
  await settings(page, 'Polecenia bez modelu');
  await page.getByRole('button', { name: 'Sprawdź w testerze: Ile to jest 7 razy 8?' }).click();
  await expect(page.getByLabel('Wiadomość do sprawdzenia')).toHaveValue('Ile to jest 7 razy 8?');
  await expect(page.getByTestId('commands-test-result')).toContainText('rachunek');
});

test('paleta: „Kopiuj rozmowę (Markdown)” z notą o treści AI', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page);
  await page.keyboard.press('Control+k');
  await expect(page.locator('dialog[open] input')).toBeFocused();
  await page.keyboard.type('Kopiuj rozmowę');
  await expect(page.locator('dialog [role=option][data-selected]')).toContainText(
    'Kopiuj rozmowę (Markdown)',
  );
  await page.keyboard.press('Enter');
  await expect(page.getByText('Skopiowano rozmowę jako Markdown.')).toBeVisible();
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied.startsWith('# ')).toBe(true);
  expect(copied).toContain('wygenerowała sztuczna inteligencja');
  expect(copied).toContain('### Ty · ');
});

test('menu sesji: „Statystyki rozmowy”; paleta prowadzi do „Polecenia bez modelu”', async ({ page }) => {
  await open(page);
  const row = page.getByRole('navigation', { name: /Sesje/ }).getByRole('listitem').first();
  await row.hover();
  await row.getByRole('button', { name: /Akcje|Więcej/ }).click();
  await page.getByRole('menuitem', { name: 'Statystyki rozmowy' }).click();
  const dialog = page.getByRole('dialog', { name: 'Statystyki rozmowy' });
  await expect(dialog).toBeVisible();
  await page.keyboard.press('Escape');
  await page.keyboard.press('Control+k');
  await expect(page.locator('dialog[open] input')).toBeFocused();
  await page.keyboard.type('Polecenia bez modelu');
  await expect(page.locator('dialog [role=option][data-selected]')).toContainText(
    'Polecenia bez modelu',
  );
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Polecenia bez modelu', level: 2 })).toBeVisible();
});

for (const skin of ['classic', 'glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`seria 4 — podpowiedź i przykłady: ${skin}, ${theme}`, async ({ page }) => {
      await open(page, skin, theme);
      await page.locator('#alfa-composer').fill('2+2');
      await expect(page.getByText('Bez modelu: rachunek')).toBeVisible();
      await expectAccessible(page, `podpowiedz ${skin} ${theme}`);
      await expectReadable(page, `podpowiedz ${skin} ${theme}`);
      await settings(page, 'Polecenia bez modelu');
      await page.getByRole('button', { name: 'Sprawdź w testerze: 2026 rzymskimi' }).focus();
      await expectAccessible(page, `przyklady ${skin} ${theme}`);
      await expectReadable(page, `przyklady ${skin} ${theme}`);
    });
  }
}
