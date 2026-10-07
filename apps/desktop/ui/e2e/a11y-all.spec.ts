import { expect, test } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

// Dostępność każdej strony ustawień i każdego panelu w obu motywach (WCAG 2.2 AA: 0 naruszeń
// critical/serious) — uzupełnienie `a11y.spec.ts`, które sprawdza wybrane ekrany szczegółowo.

test.describe.configure({ timeout: 180_000 });

for (const theme of ['light', 'dark'] as const) {
  test(`wszystkie strony ustawień — motyw ${theme}`, async ({ page }) => {
    await openApp(page, { theme });
    await waitForChat(page);
    await page.keyboard.press('Control+,');
    const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
    await expect(nav.getByRole('button').first()).toBeVisible();
    await expect(nav.getByRole('button').first()).toBeVisible();
    const names = await nav.getByRole('button').allInnerTexts();
    expect(names.length).toBeGreaterThan(20);
    for (const [i, raw] of names.entries()) {
      const name = raw.split('\n')[0]?.trim() ?? raw;
      await nav.getByRole('button').nth(i).click();
      await expect(page.getByRole('heading', { level: 2, name })).toBeVisible();
      await page.waitForTimeout(250);
      await expectAccessible(page, `ustawienia: ${name} (${theme})`);
    }
  });

  test(`wszystkie panele — motyw ${theme}`, async ({ page }) => {
    await openApp(page, { theme });
    await waitForChat(page);
    for (const key of ['Alt+1', 'Alt+2', 'Alt+3', 'Alt+4', 'Alt+5', 'Alt+6', 'Alt+7']) {
      await page.keyboard.press(key);
      await page.waitForTimeout(400);
      await expectAccessible(page, `panel ${key} (${theme})`);
    }
  });
}
