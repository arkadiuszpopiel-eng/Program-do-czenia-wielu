import { expect, test } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

for (const theme of ['light', 'dark'] as const) {
  test(`ściągawka skrótów: wyszukiwanie po nazwie i po klawiszach — motyw ${theme}`, async ({
    page,
  }) => {
    await openApp(page, { theme });
    await waitForChat(page);
    await page.keyboard.press('Control+/');
    const sheet = page.getByRole('dialog');
    const search = sheet.getByRole('textbox', { name: 'Szukaj skrótu (nazwa albo klawisze)' });
    await expect(search).toBeVisible();
    await expect(sheet.getByText('Mikrofon wł./wył.')).toBeVisible();
    await expectAccessible(page, `ściągawka ${theme}`);

    // Po nazwie, bez polskich znaków.
    await search.fill('pamiec');
    await expect(sheet.getByText('Znaleziono: 1')).toBeVisible();
    await expect(sheet.getByRole('term')).toHaveCount(1);
    await expect(sheet.getByText('Mikrofon wł./wył.')).toHaveCount(0);

    // Po klawiszach.
    await search.fill('alt+left');
    await expect(sheet.getByText('Wstecz (poprzednie miejsce)')).toBeVisible();
    await expect(sheet.getByRole('term')).toHaveCount(1);

    // Nic nie pasuje: komunikat zamiast pustego okna.
    await search.fill('zzzz');
    await expect(sheet.getByText('Żaden skrót nie pasuje do „zzzz”.')).toBeVisible();
    await expect(sheet.getByRole('term')).toHaveCount(0);

    // Ponowne otwarcie zaczyna od pełnej listy.
    await page.keyboard.press('Escape');
    await expect(sheet).toHaveCount(0);
    await page.keyboard.press('Control+/');
    await expect(search).toHaveValue('');
    await expect(sheet.getByText('Mikrofon wł./wył.')).toBeVisible();
  });
}
