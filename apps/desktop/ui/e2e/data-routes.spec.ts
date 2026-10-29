import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// „Gdzie trafiają dane” (Ustawienia → Dobór modelu, C4.1): kraj, ryzyko trenowania, decyzja
// dla sesji prywatnej z powodem; przełącznik „Tylko włączone trasy”; każdy styl i motyw.

async function open(page: Page, skin = 'classic', theme: 'light' | 'dark' = 'light') {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?style=${skin}&theme=${theme}`);
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Dobór modelu' })
    .click();
  await expect(page.getByRole('heading', { name: 'Gdzie trafiają dane' })).toBeVisible();
}

test('kraj, trenowanie i sesja prywatna dla każdej trasy', async ({ page }) => {
  await open(page);
  const list = page.getByRole('list', { name: 'Trasy do dostawców' });
  await expect(list.getByRole('listitem')).toHaveCount(5);
  const deepseek = list.getByRole('listitem').filter({ hasText: 'deepseek' });
  await expect(deepseek.getByText('dane: Chiny')).toBeVisible();
  await expect(deepseek.getByText('Dostawca może uczyć modele na twoich danych.')).toBeVisible();
  await expect(deepseek.getByText(/Sesja prywatna: zablokowana — .*CN/)).toBeVisible();
  const mistral = list.getByRole('listitem').filter({ hasText: 'mistral' });
  await expect(mistral.getByText('dane: Unia Europejska')).toBeVisible();
  await expect(mistral.getByText('Sesja prywatna: dozwolona.')).toBeVisible();
  await page.getByRole('switch', { name: 'Tylko włączone trasy' }).click();
  await expect(list.getByRole('listitem')).toHaveCount(6);
  await expect(list.getByText('wyłączona', { exact: true })).toBeVisible();
});

for (const skin of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`gdzie trafiają dane: ${skin}, ${theme}`, async ({ page }) => {
      await open(page, skin, theme);
      await page.getByRole('heading', { name: 'Gdzie trafiają dane' }).scrollIntoViewIfNeeded();
      await expectAccessible(page, `trasy ${skin} ${theme}`);
      await expectReadable(page, `trasy ${skin} ${theme}`);
    });
  }
}
