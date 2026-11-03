import { expect, test } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

// Menu „Tryb pracy” w pasku pisania (decyzja właściciela 27.10; od 3.11 pięć trybów — plan 50),
// klawisze 1–5, obniżenie od razu, podniesienie przez okno Brokera (raz). Każdy styl i motyw.
for (const style of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`menu trybu pracy — ${style} ${theme}`, async ({ page }) => {
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      const trigger = page.getByRole('button', { name: /^Tryb pracy: Automatycznie/ });
      await trigger.click();
      const menu = page.getByRole('menu', { name: 'Tryb pracy' });
      const items = menu.getByRole('menuitemradio');
      await expect(items).toHaveCount(5);
      await expect(items.nth(0)).toContainText('Plan');
      await expect(items.nth(1)).toContainText('Ręcznie');
      await expect(items.nth(2)).toContainText('Akceptuj edycje');
      await expect(items.nth(2)).toContainText('Zmiany w folderze pracy bez pytania');
      await expect(items.nth(3)).toContainText('Automatycznie');
      await expect(items.nth(4)).toContainText('Pełny dostęp');
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
      await expect(page.getByText('Tryb pracy: Ręcznie.')).toBeVisible();
      const lowered = page.getByRole('button', { name: /^Tryb pracy: Ręcznie/ });
      await expect(lowered).toBeVisible();

      // Podniesienie do „Pełnego dostępu” czeka na okno Brokera — pasek bez zmiany.
      await lowered.click();
      await expect(menu.getByRole('menuitemradio', { checked: true })).toContainText('Ręcznie');
      await menu.getByRole('menuitemradio', { name: /Pełny dostęp/ }).click();
      await expect(page.getByText('Potwierdź „Pełny dostęp” w oknie Sędziego Cieni')).toBeVisible();
      await expect(lowered).toBeVisible();

      // Klawisz 3: „Akceptuj edycje” — z „Ręcznie” to podniesienie, czeka na okno Sędziego.
      // Powiadomienia leżą nad paskiem pisania — żeton trybu da się kliknąć (3.11).
      await lowered.click();
      await expect(menu.getByRole('menuitemradio', { checked: true })).toBeFocused();
      await page.keyboard.press('3');
      await expect(
        page.getByText('Potwierdź „Akceptuj edycje” w oknie Sędziego Cieni'),
      ).toBeVisible();
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
