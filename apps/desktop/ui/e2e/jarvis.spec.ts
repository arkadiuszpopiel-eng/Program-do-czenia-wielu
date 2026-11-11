import { expect, test } from '@playwright/test';
import { expectAccessible, openApp, pickOption, waitForChat } from './helpers';

for (const style of ['classic', 'glass', 'next', 'junior']) {
  for (const theme of ['light', 'dark'] as const) {
    test(`Jarvis osobno dla każdej agentki — ${style} ${theme}`, async ({ page }) => {
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      await page.keyboard.press('Alt+1');
      for (const agent of ['alfa', 'beta', 'gama', 'delta']) {
        const card = page.locator(`.interaction[data-agent="${agent}"]`);
        await pickOption(card.getByRole('combobox'), 'jarvis');
        await expect(card).toHaveAttribute('data-interaction', 'jarvis');
        await expect(card.getByRole('status')).toContainText('Jarvis');
        await pickOption(card.getByRole('combobox'), 'standard');
        await expect(card).toHaveAttribute('data-interaction', 'standard');
      }
      const alfa = page.locator('.interaction[data-agent="alfa"]');
      await pickOption(alfa.getByRole('combobox'), 'jarvis');
      await expect(page.locator('.interaction[data-agent="beta"]')).toHaveAttribute(
        'data-interaction',
        'standard',
      );
      await expectAccessible(page, `Jarvis ${style} ${theme}`);
      await alfa.getByRole('button', { name: 'Sterowanie głosem' }).click();
      await expect(page.getByRole('tab', { name: 'Głos', exact: true })).toHaveAttribute(
        'aria-selected',
        'true',
      );
    });
  }
}

test('wybór zakresu trybu rozmowy działa w menu', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await page.getByRole('button', { name: /Tryb pracy:/ }).click();
  await page.getByRole('radio', { name: 'Ta rozmowa', exact: true }).check();
  await page.getByRole('menuitemradio', { name: /Ręcznie/ }).click();
  await expect(page.getByRole('button', { name: /Tryb pracy:.*Ręcznie/ })).toBeVisible();
});
