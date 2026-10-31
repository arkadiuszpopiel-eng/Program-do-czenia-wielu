import { expect, test } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

// Menu „Tryb pracy” z paska tytułu (decyzja właściciela 27.10): cztery tryby po polsku, klawisze
// 1–4, obniżenie od razu, podniesienie przez okno Brokera (raz). Każdy styl i motyw.
for (const style of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`menu trybu pracy — ${style} ${theme}`, async ({ page }) => {
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      const trigger = page.getByRole('button', { name: /^Tryb pracy: Automatycznie/ });
      await trigger.click();
      const menu = page.getByRole('menu', { name: 'Tryb pracy' });
      const items = menu.getByRole('menuitemradio');
      await expect(items).toHaveCount(4);
      await expect(items.nth(0)).toContainText('Tylko podgląd');
      await expect(items.nth(1)).toContainText('Pytaj o wszystko');
      await expect(items.nth(2)).toContainText('Automatycznie');
      await expect(items.nth(3)).toContainText('Pełna automatyka');
      await expect(menu.getByRole('menuitemradio', { checked: true })).toContainText(
        'Automatycznie',
      );
      await expect(menu.getByRole('menuitemradio', { checked: true })).toBeFocused();
      await expect(page.getByText('Ctrl+Shift+F12')).toBeVisible();
      // Lista leży w okienku: bez własnego tła (Szkło daje je samodzielnym `[role=menu]`).
      await expect(menu).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
      await expect(menu).toHaveCSS('box-shadow', 'none');
      await expectAccessible(page, `menu trybu ${style} ${theme}`);

      // Klawisz 2: obniżenie działa od razu, pasek pokazuje nowy tryb.
      await page.keyboard.press('2');
      await expect(menu).toHaveCount(0);
      await expect(page.getByText('Tryb pracy: Pytaj o wszystko.')).toBeVisible();
      const lowered = page.getByRole('button', { name: /^Tryb pracy: Pytaj o wszystko/ });
      await expect(lowered).toBeVisible();

      // Podniesienie do „Pełnej automatyki” czeka na okno Brokera — pasek bez zmiany.
      await lowered.click();
      await expect(menu.getByRole('menuitemradio', { checked: true })).toContainText(
        'Pytaj o wszystko',
      );
      await menu.getByRole('menuitemradio', { name: /Pełna automatyka/ }).click();
      await expect(page.getByText('Potwierdź „Pełna automatyka” w oknie Sędziego Cieni')).toBeVisible();
      await expect(lowered).toBeVisible();

      // Escape zamyka menu bez zmiany.
      await lowered.click();
      await expect(menu).toBeVisible();
      await page.keyboard.press('Escape');
      await expect(menu).toHaveCount(0);
      await expect(lowered).toBeFocused();
    });
  }
}

test('paleta (Ctrl+K): „Tryb pracy” otwiera menu w belce, a w Ustawieniach — stronę Uprawnienia', async ({
  page,
}) => {
  await openApp(page);
  await waitForChat(page);
  const run = async () => {
    await page.keyboard.press('Control+k');
    await expect(page.locator('dialog[open] input')).toBeFocused();
    await page.keyboard.type('tryb pracy');
    await expect(page.locator('dialog [role=option][data-selected]')).toContainText('Tryb pracy');
    await page.keyboard.press('Enter');
  };
  await run();
  const menu = page.getByRole('menu', { name: 'Tryb pracy' });
  await expect(menu).toBeVisible();
  await expect(menu.getByRole('menuitemradio', { checked: true })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(menu).toHaveCount(0);

  await page.keyboard.press('Control+,');
  await run();
  await expect(menu).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'Poziom autonomii' })).toBeVisible();
});
