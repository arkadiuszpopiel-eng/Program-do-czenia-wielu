import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// Seria 2 (0.0.61): instrukcja w programie, Tablica przypiętych odpowiedzi, okres na liście rozmów,
// budżet wyzwalacza, tłumaczenie odpowiedzi i styl odpowiedzi — każdy stan otwarty w każdym stylu
// i motywie (dostępność i kontrast).

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

const agentMessage = (page: Page) =>
  page.getByRole('article', { name: /^Wiadomość agentki/ }).last();

test('instrukcja: działy, treść i wyszukiwanie', async ({ page }) => {
  await open(page);
  await palette(page, 'Otwórz instrukcję');
  await expect(page.getByRole('heading', { name: 'Instrukcja', level: 2 })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Alfa — przewodnik użytkownika' })).toBeVisible();
  await page.getByLabel('Szukaj w instrukcji').fill('polecenia bez modelu');
  await expect(page.getByText(/Znalezione fragmenty: \d/)).toBeVisible();
  await page.getByRole('list', { name: 'Wyniki wyszukiwania' }).getByRole('button').first().click();
  await expect(page.getByRole('heading', { name: 'Rozmowa', level: 1 })).toBeVisible();
  await page.getByLabel('Szukaj w instrukcji').fill('zzzqqq');
  await expect(page.getByText('Nic nie znaleziono — spróbuj innego słowa.')).toBeVisible();
});

test('Tablica: przypięcie z menu wiadomości, kopia, odpięcie', async ({ page }) => {
  await open(page);
  const message = agentMessage(page);
  await message.hover();
  await message.getByRole('button', { name: 'Więcej akcji' }).click();
  await page.getByRole('menuitem', { name: 'Przypnij do tablicy' }).click();
  await expect(page.getByText(/Przypięto do Tablicy/)).toBeVisible();
  await palette(page, 'Pokaż Tablicę');
  const dialog = page.getByRole('dialog', { name: 'Tablica' });
  const list = dialog.getByRole('list', { name: 'Przypięte odpowiedzi' });
  await expect(list.getByRole('listitem')).toHaveCount(2);
  await list.getByRole('button', { name: 'Odepnij' }).first().click();
  await expect(list.getByRole('listitem')).toHaveCount(1);
  await dialog.getByRole('button', { name: 'Zamknij' }).click();
  await expect(dialog).toHaveCount(0);
});

test('archiwum: okres na liście rozmów', async ({ page }) => {
  await open(page);
  const nav = page.getByRole('navigation', { name: 'Sesje' });
  const rows = nav.locator('[data-session-row]');
  const all = await rows.count();
  expect(all).toBeGreaterThan(0);
  await expect(rows.first()).toHaveAttribute('title', /Wiadomości: \d+ · założona/);
  await nav.getByRole('combobox', { name: 'Pokaż rozmowy' }).click();
  await page.getByRole('option', { name: 'Z dzisiaj' }).click();
  await expect(async () => {
    const today = await rows.count();
    expect(today).toBeLessThanOrEqual(all);
  }).toPass();
});

test('wyzwalacz z budżetem i bezczynnością', async ({ page }) => {
  await open(page);
  await settings(page, 'Zadania w tle i wyzwalacze');
  await page.getByLabel('Nazwa', { exact: true }).fill('Porządki nocą');
  await page.getByLabel('Co ma zrobić agentka').fill('Uporządkuj pobrane pliki');
  await page.getByLabel('Tylko gdy komputer jest bezczynny').check();
  await page.getByLabel('Najwięcej kroków').fill('12');
  await page.getByLabel('Najdłużej (minuty)').fill('20');
  await page.getByLabel('Najwyższy koszt (zł)').fill('1,50');
  await page.getByRole('button', { name: 'Utwórz wyzwalacz' }).click();
  const item = page
    .getByRole('list', { name: 'Lista wyzwalaczy' })
    .getByRole('listitem')
    .filter({ hasText: 'Porządki nocą' });
  await expect(item).toContainText('do 12 kroków');
  await expect(item).toContainText('do 20 min');
  await expect(item).toContainText('tylko w bezczynności');
});

test('tłumaczenie odpowiedzi i styl odpowiedzi w ustawieniach', async ({ page }) => {
  await open(page);
  const message = agentMessage(page);
  await message.hover();
  await message.getByRole('button', { name: 'Więcej akcji' }).click();
  await expect(page.getByRole('menuitem', { name: 'Przetłumacz na angielski' })).toBeVisible();
  await page.keyboard.press('Escape');
  await settings(page, 'Ogólne');
  await expect(page.getByRole('combobox', { name: 'Styl odpowiedzi' })).toBeVisible();
  await expect(page.getByRole('combobox', { name: 'Swoboda odpowiedzi' })).toBeVisible();
});

for (const skin of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`seria 2 — okna otwarte: ${skin}, ${theme}`, async ({ page }) => {
      await open(page, skin, theme);
      await palette(page, 'Pokaż Tablicę');
      await expect(page.getByRole('dialog', { name: 'Tablica' })).toBeVisible();
      await expectAccessible(page, `tablica ${skin} ${theme}`);
      await expectReadable(page, `tablica ${skin} ${theme}`);
      await page
        .getByRole('dialog', { name: 'Tablica' })
        .getByRole('button', { name: 'Zamknij' })
        .click();
      await palette(page, 'Otwórz instrukcję');
      await expect(page.getByRole('heading', { name: 'Instrukcja', level: 2 })).toBeVisible();
      await expectAccessible(page, `instrukcja ${skin} ${theme}`);
      await expectReadable(page, `instrukcja ${skin} ${theme}`);
    });
  }
}
