import { expect, test } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

// Tryb „Plan” (plan 50 §4.1, Z.5): wybór w menu „Tryb pracy” obowiązuje otwartą rozmowę, pod
// planem trzy odpowiedzi jak w Claude Code; akceptacja podnosi poziom rozmowy przez okno Sędziego
// Cieni, a po zgodzie agentka dostaje polecenie wykonania planu. Każdy styl i motyw.
for (const style of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`tryb Plan — ${style} ${theme}`, async ({ page }) => {
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      await page.getByRole('button', { name: /^Tryb pracy: Automatycznie/ }).click();
      const menu = page.getByRole('menu', { name: 'Tryb pracy' });
      await expect(menu.getByRole('menuitemradio').nth(0)).toContainText('Plan');
      await expect(menu.getByRole('menuitemradio', { checked: true })).toBeFocused();
      await page.keyboard.press('1');
      await expect(page.getByText('Tryb pracy: Plan.')).toBeVisible();
      const chip = page.getByRole('button', { name: /^Tryb pracy: Plan/ });
      await expect(chip).toBeVisible();

      // Menu w rozmowie w trybie Plan: zaznaczony Plan.
      await chip.click();
      await expect(menu.getByRole('menuitemradio', { checked: true })).toContainText('Plan');
      await page.keyboard.press('Escape');

      const composer = page.locator('.composer textarea');
      await composer.fill('Zaplanuj porządki w raportach');
      await composer.press('Enter');
      const plan = page.getByRole('article', { name: 'Plan gotowy — co dalej?' });
      await expect(plan).toBeVisible();
      await expect(plan.getByRole('button')).toHaveText([
        /Tak, i działaj automatycznie/,
        /Tak, zatwierdzam ręcznie/,
        /Nie, planuj dalej/,
      ]);
      await expectAccessible(page, `tryb Plan ${style} ${theme}`);

      // „Nie, planuj dalej” (klawisz 3) — fokus w polu pisania, poziom bez zmian.
      await plan.getByRole('button', { name: /Nie, planuj dalej/ }).focus();
      await page.keyboard.press('3');
      await expect(composer).toBeFocused();
      await expect(page.getByText('Napisz, co zmienić w planie')).toBeVisible();

      // Akceptacja: karta Sędziego, po zgodzie — wykonanie w trybie „Automatycznie”.
      await plan.getByRole('button', { name: /Tak, i działaj automatycznie/ }).click();
      await expect(
        page.getByText('Potwierdź „Automatycznie” w oknie Sędziego Cieni — potem agentka'),
      ).toBeVisible();
      await expect(chip).toBeVisible();
      await page.evaluate(() => window.__alfaApproveRaise?.());
      await expect(page.getByText('Akceptuję plan — wykonaj go.')).toBeVisible({
        timeout: 10_000,
      });
      await expect(page.getByRole('button', { name: /^Tryb pracy: Automatycznie/ })).toBeVisible();
      await expect(plan).toHaveCount(0);
    });
  }
}
