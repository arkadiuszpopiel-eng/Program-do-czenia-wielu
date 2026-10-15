import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// Seria 3 (0.0.62): F1 → instrukcja, trafienia z instrukcji w wyszukiwaniu Ustawień, szablony
// i polecenia jako JSON, statystyki rozmowy, wyszukiwanie na Tablicy, nowe polecenia bez modelu
// w katalogu — każdy stan otwarty w każdym stylu i motywie (dostępność i kontrast).

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

async function palette(page: Page, command: string) {
  await page.keyboard.press('Control+k');
  await expect(page.locator('dialog[open] input')).toBeFocused();
  await page.keyboard.type(command);
  await expect(page.locator('dialog [role=option][data-selected]')).toContainText(command);
  await page.keyboard.press('Enter');
}

const JSON_FIELD = 'JSON do dodania (lista albo jeden obiekt)';

test('F1 otwiera instrukcję; wyszukiwanie w Ustawieniach pokazuje trafienia z instrukcji', async ({
  page,
}) => {
  await open(page);
  await page.keyboard.press('F1');
  await expect(page.getByRole('heading', { name: 'Instrukcja', level: 2 })).toBeVisible();
  await page.getByRole('searchbox', { name: 'Szukaj ustawień' }).fill('Broker');
  const hits = page.getByRole('region', { name: 'W instrukcji' });
  await expect(hits).toBeVisible();
  await hits.getByRole('button').first().click();
  await expect(page.getByRole('heading', { name: 'Instrukcja', level: 2 })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Bezpieczeństwo', level: 1 })).toBeVisible();
});

test('szablony: kopiuj jako JSON i dodaj z wklejonego JSON (z błędem kształtu)', async ({
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page);
  await settings(page, 'Szablony promptów');
  const list = page.getByRole('list', { name: 'Szablony' });
  const before = await list.getByRole('listitem').count();
  await page.getByRole('button', { name: 'Kopiuj jako JSON' }).click();
  await expect(page.getByText(/Skopiowano \d+ poz\. jako JSON/)).toBeVisible();
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied).not.toContain('"id"');
  await page.getByRole('button', { name: 'Wklej z JSON…' }).click();
  const field = page.getByLabel(JSON_FIELD);
  await field.fill('[{"name":"Bez treści"}]');
  await page.getByRole('button', { name: 'Dodaj z JSON' }).click();
  await expect(page.getByRole('alert')).toContainText('Pozycja 1 nie ma wymaganych pól');
  await field.fill('{"name":"Z JSON","text":"Streść {{temat}} w trzech zdaniach"}');
  await page.getByRole('button', { name: 'Dodaj z JSON' }).click();
  await expect(list.getByRole('listitem')).toHaveCount(before + 1);
  await expect(list.getByText('Z JSON', { exact: true })).toBeVisible();
});

test('polecenia bez modelu: dodanie z JSON i nowe polecenia wbudowane w katalogu', async ({
  page,
}) => {
  await open(page);
  await settings(page, 'Polecenia bez modelu');
  await page.getByRole('button', { name: 'Wklej z JSON…' }).click();
  await page.getByLabel(JSON_FIELD).fill('[{"name":"Kawa","phrases":["kawa"],"reply":"Parzę!"}]');
  await page.getByRole('button', { name: 'Dodaj z JSON' }).click();
  await expect(page.getByText(/Dodano 1 poz\. z JSON/)).toBeVisible();
  const own = page.getByRole('region', { name: 'Własne polecenia' });
  await expect(own.getByText('Kawa', { exact: true })).toBeVisible();
  const builtin = page.getByRole('region', { name: 'Polecenia wbudowane' });
  await expect(builtin.getByText(/Rachunki na datach/)).toBeVisible();
  await expect(builtin.getByText(/Przeliczanie jednostek/)).toBeVisible();
});

test('statystyki rozmowy: liczby ścieżki i kopia jako Markdown', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page);
  await palette(page, 'Statystyki rozmowy');
  const dialog = page.getByRole('dialog', { name: 'Statystyki rozmowy' });
  const list = dialog.getByLabel('Statystyki', { exact: true });
  await expect(list).toContainText('Wiadomości');
  await expect(list).toContainText('Czas czytania');
  await dialog.getByRole('button', { name: 'Kopiuj jako Markdown' }).click();
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied).toContain('## Statystyki rozmowy');
  await dialog.getByRole('button', { name: 'Zamknij' }).click();
  await expect(dialog).toHaveCount(0);
});

test('Tablica: wyszukiwanie i kopia wszystkiego', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page);
  await palette(page, 'Pokaż Tablicę');
  const dialog = page.getByRole('dialog', { name: 'Tablica' });
  const list = dialog.getByRole('list', { name: 'Przypięte odpowiedzi' });
  await expect(list.getByRole('listitem').first()).toBeVisible();
  const all = await list.getByRole('listitem').count();
  await dialog.getByLabel('Szukaj na Tablicy').fill('zzzqqq');
  await expect(dialog.getByText('Nic nie pasuje do „zzzqqq”.')).toBeVisible();
  await expect(list.getByRole('listitem')).toHaveCount(0);
  await dialog.getByLabel('Szukaj na Tablicy').fill('');
  await expect(list.getByRole('listitem')).toHaveCount(all);
  await dialog.getByRole('button', { name: 'Kopiuj wszystko (Markdown)' }).click();
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied.startsWith('# Tablica')).toBe(true);
});

test('tryb skupienia: F11 włącza, Esc wyłącza', async ({ page }) => {
  await open(page);
  await page.locator('main').click({ position: { x: 5, y: 5 } });
  await page.keyboard.press('F11');
  await expect(page.locator('.shell.focus')).toHaveCount(1);
  await page.keyboard.press('Escape');
  await expect(page.locator('.shell.focus')).toHaveCount(0);
});

for (const skin of ['classic', 'glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`seria 3 — okna i strony: ${skin}, ${theme}`, async ({ page }) => {
      await open(page, skin, theme);
      await palette(page, 'Statystyki rozmowy');
      await expect(page.getByRole('dialog', { name: 'Statystyki rozmowy' })).toBeVisible();
      await expectAccessible(page, `statystyki ${skin} ${theme}`);
      await expectReadable(page, `statystyki ${skin} ${theme}`);
      await page.keyboard.press('Escape');
      await palette(page, 'Pokaż Tablicę');
      await page.getByLabel('Szukaj na Tablicy').fill('zzzqqq');
      await expectAccessible(page, `tablica-szukaj ${skin} ${theme}`);
      await expectReadable(page, `tablica-szukaj ${skin} ${theme}`);
      await page.keyboard.press('Escape');
      await settings(page, 'Szablony promptów');
      await page.getByRole('button', { name: 'Wklej z JSON…' }).click();
      await page.getByLabel(JSON_FIELD).fill('{');
      await page.getByRole('button', { name: 'Dodaj z JSON' }).click();
      await expect(page.getByRole('alert')).toBeVisible();
      await expectAccessible(page, `json ${skin} ${theme}`);
      await expectReadable(page, `json ${skin} ${theme}`);
      await page.getByRole('searchbox', { name: 'Szukaj ustawień' }).fill('Broker');
      await expect(page.getByRole('region', { name: 'W instrukcji' })).toBeVisible();
      await expectAccessible(page, `ustawienia-instrukcja ${skin} ${theme}`);
      await expectReadable(page, `ustawienia-instrukcja ${skin} ${theme}`);
    });
  }
}
