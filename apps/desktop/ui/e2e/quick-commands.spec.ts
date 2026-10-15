import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, pickOption, waitForChat } from './helpers';

// Ustawienia → Polecenia bez modelu: tester (wbudowane, własne, model), dodanie polecenia
// z „{reszta}”, błąd wyrażenia, usunięcie z potwierdzeniem, katalog wbudowanych; każdy styl
// i motyw z otwartym formularzem.

async function openCommands(page: Page, skin = 'classic', theme: 'light' | 'dark' = 'light') {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?style=${skin}&theme=${theme}`);
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Polecenia bez modelu' })
    .click();
  await expect(page.getByRole('heading', { name: 'Polecenia bez modelu', level: 2 })).toBeVisible();
}

const result = (page: Page) => page.getByTestId('commands-test-result');

test('tester, dodanie, błąd wyrażenia, usunięcie, katalog', async ({ page }) => {
  await openCommands(page);
  const input = page.getByLabel('Wiadomość do sprawdzenia');
  await input.fill('Ile to jest 7 razy 8?');
  await expect(result(page)).toContainText('Odpowie polecenie wbudowane (rachunek):');
  await expect(result(page)).toContainText('7 × 8 = **56**');
  await input.fill('dobranoc');
  await expect(result(page)).toContainText('Odpowie własne polecenie „Dobranoc”:');
  await input.fill('Opowiedz bajkę');
  await expect(result(page)).toContainText('pójdzie do modelu');

  await page.getByRole('button', { name: 'Dodaj polecenie' }).click();
  await page.getByLabel('Nazwa', { exact: true }).fill('Zanotuj');
  await page.getByLabel('Frazy (każda w osobnym wierszu)').fill('zanotuj');
  await pickOption(page.getByRole('combobox', { name: 'Dopasowanie' }), 'starts');
  await expect(page.getByText(/dalszy tekst wstawisz w odpowiedzi jako \{reszta\}/)).toBeVisible();
  await page.getByLabel('Odpowiedź', { exact: true }).fill('Zapisuję: {reszta}');
  await page.getByRole('button', { name: 'Zapisz', exact: true }).click();
  const own = page.getByRole('region', { name: 'Własne polecenia' });
  await expect(own.getByText('Zanotuj', { exact: true })).toBeVisible();
  await input.fill('Zanotuj: kupić mleko');
  await expect(result(page)).toContainText('Zapisuję: kupić mleko');

  await own.getByRole('button', { name: 'Dodaj polecenie' }).click();
  await page.getByLabel('Nazwa', { exact: true }).fill('Zepsute');
  await page.getByLabel('Frazy (każda w osobnym wierszu)').fill('(');
  await pickOption(page.getByRole('combobox', { name: 'Dopasowanie' }), 'regex');
  await page.getByLabel('Odpowiedź', { exact: true }).fill('x');
  await page.getByRole('button', { name: 'Zapisz', exact: true }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'złe wyrażenie' })).toBeVisible();
  await page.getByRole('button', { name: 'Anuluj', exact: true }).click();

  const item = own.getByRole('listitem').filter({ hasText: 'Zanotuj' });
  await item.getByRole('button', { name: 'Usuń' }).click();
  await item.getByRole('button', { name: 'Tak, usuń' }).click();
  await expect(own.getByText('Zanotuj', { exact: true })).toHaveCount(0);

  const builtin = page.getByRole('region', { name: 'Polecenia wbudowane' });
  await expect(builtin.getByRole('listitem')).toHaveCount(5);
  await expect(page.getByRole('switch', { name: 'Odpowiadaj bez modelu' })).toBeChecked();
});

for (const skin of ['classic', 'glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`polecenia bez modelu: ${skin}, ${theme}`, async ({ page }) => {
      await openCommands(page, skin, theme);
      await page.getByLabel('Wiadomość do sprawdzenia').fill('2+2');
      await expect(result(page)).toContainText('rachunek');
      await page.getByRole('button', { name: 'Dodaj polecenie' }).click();
      await expect(page.getByRole('heading', { name: 'Polecenie', exact: true })).toBeVisible();
      await expectAccessible(page, `polecenia ${skin} ${theme}`);
      await expectReadable(page, `polecenia ${skin} ${theme}`);
    });
  }
}
