// Klawisze pola pisania z rejestru skrótów (funkcje katalogu `composer.send`, `composer.newline`,
// `composer.editLast`, `composer.historyPrev`, `composer.historyNext`, `composer.pastePlain`,
// `conversation.escape` — docs/plan/45): dług z założenia bramki katalogu 31.10, spłacony przez sesję
// testującą. Wklejanie bez formatu wykonuje pole tekstowe przeglądarki — test sprawdza, że program
// przepuszcza skrót i tekst z HTML (przechwytuje tylko pliki), bez sięgania do schowka systemu.
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

test('composer.pastePlain: Ctrl+Shift+V i tekst z HTML przechodzą do pola, bez załącznika', async ({
  page,
}) => {
  await openApp(page);
  await waitForChat(page);
  const field = page.locator('#alfa-composer');
  await field.fill('Przed: ');
  // Zdarzenia tworzone w stronie — schowek systemu (właściciela) nietknięty i nieczytany.
  const result = await field.evaluate((el) => {
    const key = new KeyboardEvent('keydown', {
      key: 'V',
      code: 'KeyV',
      ctrlKey: true,
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    });
    el.dispatchEvent(key);
    const data = new DataTransfer();
    data.setData('text/html', '<b>Pogrubiony</b> <i>tekst</i>');
    data.setData('text/plain', 'Pogrubiony tekst');
    const paste = new ClipboardEvent('paste', {
      clipboardData: data,
      bubbles: true,
      cancelable: true,
    });
    el.dispatchEvent(paste);
    return { key: key.defaultPrevented, paste: paste.defaultPrevented };
  });
  // Skrót należy do pola (COMPOSER_LOCAL): program go nie przechwytuje, więc wkleja przeglądarka —
  // bez formatu. Tekst (także obok HTML) nie jest brany jako plik do załączenia.
  expect(result).toEqual({ key: false, paste: false });
  await expect(field).toHaveValue('Przed: ');
  await expect(field).toBeFocused();
});

test('conversation.escape: Esc najpierw zamyka listę podpowiedzi, dopiero następny zatrzymuje generowanie', async ({
  page,
}) => {
  await openApp(page);
  await waitForChat(page);
  const field = page.locator('#alfa-composer');
  await field.fill('Napisz szczegółowy esej');
  await field.press('Enter');
  const feed = page.getByRole('feed');
  await expect(feed).toHaveAttribute('aria-busy', 'true');
  // W czasie generowania: lista podpowiedzi @agentek w polu pisania.
  await field.pressSequentially('@de');
  await expect(page.getByRole('option', { name: /@Delta/ })).toBeVisible();
  await field.press('Escape');
  await expect(page.getByRole('option', { name: /@Delta/ })).toHaveCount(0);
  await expect(field).toHaveValue('@de');
  await expect(feed).toHaveAttribute('aria-busy', 'true');
  await expect(page.getByText('Przerwano.')).toHaveCount(0);
  // Drugi Esc (nic innego otwartego) — zatrzymanie generowania.
  await field.press('Escape');
  await expect(page.getByText('Przerwano.')).toBeVisible();
});
