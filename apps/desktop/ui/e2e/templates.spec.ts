import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// Szablony promptów: dodanie na stronie ustawień (zmienne widoczne), błąd niedomkniętej
// zmiennej, okno „Wstaw szablon” z palety — pola zmiennych, podgląd, wstawienie do pola
// wpisywania (bez wysyłania); okno otwarte w każdym stylu i motywie.

async function open(page: Page, skin = 'classic', theme: 'light' | 'dark' = 'light') {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?style=${skin}&theme=${theme}`);
  await waitForChat(page);
}

async function palette(page: Page) {
  await page.keyboard.press('Control+k');
  await expect(page.locator('dialog[open] input')).toBeFocused();
  await page.keyboard.type('Wstaw szablon promptu');
  await expect(page.locator('dialog [role=option][data-selected]')).toContainText(
    'Wstaw szablon promptu',
  );
  await page.keyboard.press('Enter');
  await expect(page.getByRole('dialog', { name: 'Wstaw szablon' })).toBeVisible();
}

test('strona szablonów: dodanie, zmienne, błąd, usunięcie', async ({ page }) => {
  await open(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Szablony promptów' })
    .click();
  const list = page.getByRole('list', { name: 'Szablony' });
  await expect(list.getByRole('listitem')).toHaveCount(2);
  await page.getByRole('button', { name: 'Dodaj szablon' }).click();
  await page.getByLabel('Nazwa', { exact: true }).fill('Plan dnia');
  await page.getByLabel('Treść', { exact: true }).fill('Plan na {{dzień}}: {{zadania}}');
  await expect(page.getByText('zmienne: dzień, zadania')).toBeVisible();
  await page.getByRole('button', { name: 'Zapisz', exact: true }).click();
  await expect(list.getByRole('listitem')).toHaveCount(3);

  await list
    .getByRole('listitem')
    .filter({ hasText: 'Plan dnia' })
    .getByRole('button', { name: 'Edytuj' })
    .click();
  await page.getByLabel('Treść', { exact: true }).fill('Plan na {{dzień');
  await page.getByRole('button', { name: 'Zapisz', exact: true }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'niedomknięta' })).toBeVisible();
  await page.getByRole('button', { name: 'Anuluj', exact: true }).click();

  const item = list.getByRole('listitem').filter({ hasText: 'Plan dnia' });
  await item.getByRole('button', { name: 'Usuń' }).click();
  await item.getByRole('button', { name: 'Tak, usuń' }).click();
  await expect(list.getByRole('listitem')).toHaveCount(2);
});

test('okno z palety wypełnia zmienne i wstawia tekst do pola wpisywania', async ({ page }) => {
  await open(page);
  await palette(page);
  const dialog = page.getByRole('dialog', { name: 'Wstaw szablon' });
  await dialog.getByLabel('adresat').fill('Pani Anny');
  await expect(dialog.getByTestId('tpl-preview')).toContainText(
    'Napisz uprzejmy e-mail do Pani Anny w sprawie: {{temat}}',
  );
  await dialog.getByLabel('temat').fill('spotkania w piątek');
  await dialog.getByRole('button', { name: 'Wstaw do rozmowy' }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole('textbox', { name: /Wiadomość/ }).first()).toHaveValue(
    'Napisz uprzejmy e-mail do Pani Anny w sprawie: spotkania w piątek. Krótko, po polsku.',
  );
});

for (const skin of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`okno szablonu: ${skin}, ${theme}`, async ({ page }) => {
      await open(page, skin, theme);
      await palette(page);
      await page.getByRole('dialog', { name: 'Wstaw szablon' }).getByLabel('adresat').fill('Jana');
      await expectAccessible(page, `szablon ${skin} ${theme}`);
      await expectReadable(page, `szablon ${skin} ${theme}`);
    });
  }
}
