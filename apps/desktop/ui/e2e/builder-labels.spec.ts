import { expect, test } from '@playwright/test';
import { openApp, waitForChat } from './helpers';

for (const en of [false, true]) {
  test(`Kreator: istniejące grupy mają etykiety ${en ? 'EN' : 'PL'}`, async ({ page }) => {
    await openApp(page);
    await waitForChat(page);
    await page.keyboard.press('Control+,');
    await page
      .getByRole('navigation', { name: 'Sekcje ustawień' })
      .getByRole('button', { name: 'Kreator agentek', exact: true })
      .click();
    if (en) {
      await page.keyboard.press('Control+k');
      await page.locator('dialog[open] input').fill('angielski');
      await expect(page.locator('dialog [role=option][data-selected]')).toContainText('angielski');
      await page.keyboard.press('Enter');
      await expect(page.locator('html')).toHaveAttribute('lang', 'en');
    }
    await expect(
      page.getByRole('checkbox', {
        name: en ? 'system tools' : 'narzędzia systemowe',
        exact: true,
      }),
    ).toBeVisible();
    await expect(
      page.getByRole('checkbox', { name: en ? 'image analysis' : 'analiza obrazu', exact: true }),
    ).toBeVisible();
    await expect(page.getByText('builder.group.system', { exact: true })).toHaveCount(0);
    await expect(page.getByText('builder.group.vision', { exact: true })).toHaveCount(0);
  });
}
