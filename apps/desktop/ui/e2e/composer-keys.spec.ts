// Klawisze pola pisania z rejestru skrótów (funkcje katalogu `composer.send`, `composer.newline`,
// `composer.editLast`, `composer.historyPrev`, `composer.historyNext` — docs/plan/45): dług z założenia
// bramki katalogu 31.10, spłacony przez sesję testującą. Wklejanie bez formatu (Ctrl+Shift+V) zostaje
// w długu: to zachowanie samego pola tekstowego WebView2 (Alfa go nie obsługuje), a schowek przeglądarki
// testowej wymaga uprawnień niedostępnych w tym przebiegu.
import { expect, type Page, test } from '@playwright/test';
import { openApp, waitForChat } from './helpers';

async function send(page: Page, text: string) {
  const field = page.locator('#alfa-composer');
  await field.fill(text);
  await field.press('Enter');
  await expect(page.locator('main').getByText(text, { exact: true }).first()).toBeVisible();
  await expect(field).toHaveValue('');
}

test('composer.send i composer.newline: Enter wysyła, Shift+Enter dodaje wiersz bez wysyłania', async ({
  page,
}) => {
  await openApp(page);
  await waitForChat(page);
  const field = page.locator('#alfa-composer');
  await field.fill('Pierwszy wiersz');
  await field.press('Shift+Enter');
  await field.pressSequentially('drugi wiersz');
  await expect(field).toHaveValue('Pierwszy wiersz\ndrugi wiersz');
  await expect(page.locator('main').getByText('drugi wiersz')).toHaveCount(0);
  await field.press('Enter');
  await expect(field).toHaveValue('');
  await expect(
    page
      .locator('main')
      .getByText(/Pierwszy wiersz/)
      .first(),
  ).toBeVisible();
});

test('composer.historyPrev / composer.historyNext: Ctrl+↑/↓ przechodzą po wysłanych wiadomościach', async ({
  page,
}) => {
  await openApp(page);
  await waitForChat(page);
  await send(page, 'Wiadomość pierwsza');
  await send(page, 'Wiadomość druga');
  const field = page.locator('#alfa-composer');
  await field.focus();
  await field.press('Control+ArrowUp');
  await expect(field).toHaveValue('Wiadomość druga');
  await field.press('Control+ArrowUp');
  await expect(field).toHaveValue('Wiadomość pierwsza');
  await field.press('Control+ArrowDown');
  await expect(field).toHaveValue('Wiadomość druga');
});

test('composer.editLast: ↑ w pustym polu otwiera edycję ostatniej własnej wiadomości', async ({
  page,
}) => {
  await openApp(page);
  await waitForChat(page);
  await send(page, 'Do poprawienia');
  const field = page.locator('#alfa-composer');
  await field.focus();
  await field.press('ArrowUp');
  const edit = page.locator('.edit textarea');
  await expect(edit).toBeVisible();
  await expect(edit).toHaveValue('Do poprawienia');
  await expect(edit).toBeFocused();
  // Z tekstem w polu ↑ nie przejmuje edycji (porusza kursorem).
  await page.locator('.edit').getByRole('button', { name: 'Anuluj' }).click();
  await field.fill('szkic');
  await field.press('ArrowUp');
  await expect(page.locator('.edit textarea')).toHaveCount(0);
});
