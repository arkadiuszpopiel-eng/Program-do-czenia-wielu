import { expect, test, type Page } from '@playwright/test';
import { openApp, waitForChat } from './helpers';
import { overlapDefects } from './overlap';

async function settings(page: Page, name: string, compact = false) {
  await page.keyboard.press('Control+,');
  const sections = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  const collapsed = page.getByRole('button', { name: /^Sekcja: / });
  if (compact) await collapsed.click();
  await sections.getByRole('button', { name, exact: true }).click();
  await expect(page.getByRole('heading', { level: 2, name })).toBeVisible();
  await page.evaluate(async () => {
    await document.fonts.ready;
    await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));
  });
}

for (const style of ['classic', 'glass', 'next', 'junior']) {
  test(`przyciski kont mieszczą się w360px — ${style}`, async ({ page }) => {
    await page.setViewportSize({ width: 360, height: 800 });
    await openApp(page, { path: `/?style=${style}` });
    await waitForChat(page);
    await settings(page, 'Konta i klucze API', true);
    const buttons = page.locator('.head-actions').getByRole('button');
    await expect(buttons).toHaveCount(2);
    for (const larger of [false, true]) {
      if (larger) {
        for (let i = 0; i < 5; i++) await page.keyboard.press('Control+=');
      }
      await page.screenshot({ path: test.info().outputPath(`accounts-${larger}.png`) });
      for (const button of await buttons.all()) {
        await expect(button).toBeInViewport({ ratio: 1 });
        expect(
          await button.locator('.label').evaluate((el) => el.scrollWidth <= el.clientWidth + 1),
        ).toBe(true);
      }
    }
  });
}

for (const theme of ['light', 'dark'] as const) {
  test(`problemy wtyczek bez nakładania tekstu — junior ${theme}`, async ({ page }) => {
    await openApp(page, { theme, path: '/?style=junior' });
    await waitForChat(page);
    for (let i = 0; i < 3; i++) await page.keyboard.press('Control+=');
    await settings(page, 'Wtyczki');
    const section = page.locator('[aria-labelledby="pl-problems"]');
    await expect(section.locator('li').first()).toBeVisible();
    await section.scrollIntoViewIfNeeded();
    await page.screenshot({ path: test.info().outputPath('plugin-problems.png') });
    expect(await overlapDefects(page, '[aria-labelledby="pl-problems"]')).toEqual([]);
  });
}
