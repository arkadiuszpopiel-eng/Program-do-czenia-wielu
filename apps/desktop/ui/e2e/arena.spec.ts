import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, pickOption, waitForChat } from './helpers';

// Arena (P4, 0.0.65): ta sama wiadomość do dwóch modeli obok siebie — wybór modeli, dwie
// odpowiedzi jako nowe gałęzie (po kolei), czas / tokeny / koszt, „Wybierz tę odpowiedź”, błąd
// tury (limit kosztów), za mało modeli — każdy stan w każdym stylu i motywie.
// Atrapa: Anthropic (Claude) · claude-opus-5-5 odpowiada dłużej (wolniej i drożej), OpenAI · gpt-6-sol krócej.

const LONG_START = 'Rozpisałam to krok po kroku.';
const SHORT_START = 'W skrócie:';

async function open(
  page: Page,
  opts: { skin?: string; theme?: 'light' | 'dark'; scenario?: string } = {},
) {
  const theme = opts.theme ?? 'light';
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  const scenario = opts.scenario ? `&scenario=${opts.scenario}` : '';
  await page.goto(`/?style=${opts.skin ?? 'classic'}&theme=${theme}${scenario}`);
  await waitForChat(page);
}

async function openFromPalette(page: Page) {
  await page.keyboard.press('Control+k');
  await expect(page.locator('dialog[open] input')).toBeFocused();
  await page.keyboard.type('Porównaj modele');
  await expect(page.locator('dialog [role=option][data-selected]')).toContainText(
    'Porównaj modele (Arena)',
  );
  await page.keyboard.press('Enter');
  const dialog = page.getByRole('dialog', { name: 'Porównaj modele (Arena)' });
  await expect(dialog).toBeVisible();
  return dialog;
}

test('paleta → Arena: dwie odpowiedzi obok siebie, miary, wybór gałęzi', async ({ page }) => {
  await open(page);
  const dialog = await openFromPalette(page);
  const a = dialog.getByRole('combobox', { name: 'Model A' });
  const b = dialog.getByRole('combobox', { name: 'Model B' });
  await expect(a).toHaveAttribute('data-value', 'anthropic:claude-opus-5-5');
  await expect(b).toHaveAttribute('data-value', 'openai:gpt-6-sol');
  await expect(dialog.getByText(/liczą się do kosztów i limitów/)).toBeVisible();
  await dialog.getByRole('button', { name: 'Porównaj' }).click();

  const statuses = dialog.getByTestId('arena-status');
  await expect(statuses).toHaveCount(2);
  await expect(statuses.nth(0)).toHaveText('Gotowe', { timeout: 15_000 });
  await expect(statuses.nth(1)).toHaveText('Gotowe', { timeout: 15_000 });
  const first = dialog.getByRole('region', {
    name: 'Odpowiedź modelu Anthropic (Claude) · claude-opus-5-5',
  });
  const second = dialog.getByRole('region', { name: 'Odpowiedź modelu OpenAI · gpt-6-sol' });
  await expect(first).toContainText(LONG_START);
  await expect(second).toContainText(SHORT_START);
  await expect(dialog.getByText('Czas odpowiedzi')).toHaveCount(2);
  await expect(dialog.getByText('Tokeny (wejście / wyjście)')).toHaveCount(2);
  await expect(dialog.getByText('Szybsza')).toHaveCount(1);
  await expect(dialog.getByText('Tańsza')).toHaveCount(1);

  await dialog
    .getByRole('button', { name: 'Wybierz odpowiedź modelu Anthropic (Claude) · claude-opus-5-5' })
    .click();
  await expect(dialog).toBeHidden();
  await expect(
    page.getByText('Wybrano odpowiedź modelu Anthropic (Claude) · claude-opus-5-5.'),
  ).toBeVisible();
  await expect(page.locator('article').last()).toContainText(LONG_START);
});

test('menu wiadomości → Arena: ta wiadomość; dwa razy ten sam model — blokada', async ({
  page,
}) => {
  await open(page);
  const mine = page.getByRole('article', { name: /^Twoja wiadomość/ }).last();
  const text = (await mine.innerText()).split('\n').find((line) => line.trim().length > 20) ?? '';
  await mine.hover();
  await mine.getByRole('button', { name: 'Więcej akcji' }).click();
  await page.getByRole('menuitem', { name: 'Porównaj modele (Arena)' }).click();
  const dialog = page.getByRole('dialog', { name: 'Porównaj modele (Arena)' });
  await expect(dialog.locator('blockquote')).toContainText(text.trim().slice(0, 20));
  await pickOption(dialog.getByRole('combobox', { name: 'Model B' }), 'anthropic:claude-opus-5-5');
  await expect(dialog.getByRole('alert')).toHaveText('Wybierz dwa różne modele.');
  await expect(dialog.getByRole('button', { name: 'Porównaj' })).toBeDisabled();
  await dialog.getByRole('button', { name: 'Anuluj' }).click();
  await expect(dialog).toBeHidden();
});

test('Arena: limit kosztów jednego modelu — błąd przy nim, drugi odpowiada', async ({ page }) => {
  await open(page, { scenario: 'arena-budget' });
  const dialog = await openFromPalette(page);
  await dialog.getByRole('button', { name: 'Porównaj' }).click();
  const statuses = dialog.getByTestId('arena-status');
  await expect(statuses.nth(1)).toHaveText('Błąd', { timeout: 15_000 });
  await expect(statuses.nth(0)).toHaveText('Gotowe', { timeout: 15_000 });
  await expect(dialog.getByRole('alert')).toContainText('Limit kosztów osiągnięty');
  await expect(
    dialog.getByRole('button', { name: 'Wybierz odpowiedź modelu OpenAI · gpt-6-sol' }),
  ).toBeDisabled();
});

test('Arena: Zatrzymaj w trakcie A — A zatrzymana, B nie startuje, bez „Szybsza”', async ({
  page,
}) => {
  await open(page, { scenario: 'slow-stream' });
  const dialog = await openFromPalette(page);
  await dialog.getByRole('button', { name: 'Porównaj' }).click();
  const statuses = dialog.getByTestId('arena-status');
  await expect(statuses.nth(0)).toHaveText('Odpowiada…');
  await expect(statuses.nth(1)).toHaveText('Czeka na swoją kolej');
  const stop = dialog.getByRole('button', { name: 'Zatrzymaj' });
  await stop.click();
  await expect(stop).toBeHidden({ timeout: 15_000 });
  await expect(statuses.nth(0)).toHaveText('Zatrzymano');
  await expect(statuses.nth(1)).toHaveText('Nie uruchomiono');
  await expect(dialog.getByText('Szybsza')).toHaveCount(0);
  await expect(dialog.getByText('Tańsza')).toHaveCount(0);
  await expect(
    dialog.getByRole('button', {
      name: 'Wybierz odpowiedź modelu Anthropic (Claude) · claude-opus-5-5',
    }),
  ).toBeDisabled();
});

test('Arena: zwykła odpowiedź w toku — „Porównaj” czeka, nie przerywa jej', async ({ page }) => {
  await open(page, { scenario: 'slow-stream' });
  await page.locator('#alfa-composer').fill('Zaplanuj mi tydzień pracy.');
  await page.keyboard.press('Enter');
  const dialog = await openFromPalette(page);
  await expect(dialog.getByRole('status')).toHaveText(
    'Poczekaj na koniec bieżącej odpowiedzi — porównanie przerwałoby ją.',
  );
  await expect(dialog.getByRole('button', { name: 'Porównaj' })).toBeDisabled();
  await expectAccessible(page, 'arena odpowiedz w toku');
  await expectReadable(page, 'arena odpowiedz w toku');
  await dialog.getByRole('button', { name: 'Anuluj' }).click();
  await expect(dialog).toBeHidden();
});

test('Arena: brak modeli i brak wiadomości', async ({ page }) => {
  await open(page, { scenario: 'no-keys' });
  let dialog = await openFromPalette(page);
  await expect(dialog.getByText(/co najmniej dwa modele/)).toBeVisible();
  await dialog.getByRole('button', { name: 'Zamknij' }).click();

  await open(page, { scenario: 'empty' });
  dialog = await openFromPalette(page);
  await expect(dialog.getByText(/Najpierw wyślij wiadomość/)).toBeVisible();
});

for (const skin of ['classic', 'glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`Arena — wybór, wyniki, błąd, za mało modeli: ${skin}, ${theme}`, async ({ page }) => {
      await open(page, { skin, theme });
      let dialog = await openFromPalette(page);
      await expectAccessible(page, `arena wybor ${skin} ${theme}`);
      await expectReadable(page, `arena wybor ${skin} ${theme}`);
      await dialog.getByRole('button', { name: 'Porównaj' }).click();
      await expect(dialog.getByText('Tańsza')).toBeVisible({ timeout: 15_000 });
      await expectAccessible(page, `arena wyniki ${skin} ${theme}`);
      await expectReadable(page, `arena wyniki ${skin} ${theme}`);

      await open(page, { skin, theme, scenario: 'arena-budget' });
      dialog = await openFromPalette(page);
      await dialog.getByRole('button', { name: 'Porównaj' }).click();
      const statuses = dialog.getByTestId('arena-status');
      await expect(statuses.nth(0)).toHaveText('Gotowe', { timeout: 15_000 });
      await expect(statuses.nth(1)).toHaveText('Błąd', { timeout: 15_000 });
      await expectAccessible(page, `arena blad ${skin} ${theme}`);
      await expectReadable(page, `arena blad ${skin} ${theme}`);

      await open(page, { skin, theme, scenario: 'no-keys' });
      dialog = await openFromPalette(page);
      await expect(dialog.getByText(/co najmniej dwa modele/)).toBeVisible();
      await expectAccessible(page, `arena brak modeli ${skin} ${theme}`);
      await expectReadable(page, `arena brak modeli ${skin} ${theme}`);
    });
  }
}
