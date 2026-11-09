import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

async function proposal(page: Page, english = false) {
  await waitForChat(page);
  if (english) {
    await page.keyboard.press('Control+k');
    await page.locator('dialog[open] input').fill('angielski');
    await page.keyboard.press('Enter');
    await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  }
  await page.locator('#alfa-composer').fill('Delta, uporządkuj pliki w folderze Pobrane.');
  await page.keyboard.press('Enter');
  const tools = page.locator('.tools').last();
  await expect(tools.getByRole('button').first()).toContainText(
    english ? 'Awaiting confirmation: 1' : 'Do potwierdzenia: 1',
  );
  await tools.getByRole('button').first().click();
  const pending = tools.locator('[data-tool-status="needs_confirmation"]');
  await expect(pending).toHaveCount(1);
  await expect(pending).toContainText(english ? 'Requires confirmation' : 'Wymaga potwierdzenia');
  await expect(pending).toContainText('Uruchom w terminalu');
  await expect(pending.getByRole('button')).toHaveCount(0);
  await expect(tools.locator('[data-tool-status="done"]')).toHaveCount(2);
  await expect(tools.locator('[data-tool-status="running"]')).toHaveCount(0);
  return tools;
}

for (const style of ['classic', 'glass', 'next', 'junior']) {
  for (const theme of ['light', 'dark'] as const) {
    test(`propozycja narzędzia wymaga potwierdzenia — ${style} ${theme}`, async ({ page }) => {
      await openApp(page, { theme, path: `/?style=${style}` });
      await proposal(page);
      await expectAccessible(page, `stan potwierdzenia ${style} ${theme}`);
      if (style === 'classic' && theme === 'light') {
        await page.screenshot({
          path: test.info().outputPath('confirmation-classic.png'),
          fullPage: true,
        });
      }
    });
  }
}

test('stan potwierdzenia jest czytelny po angielsku', async ({ page }) => {
  await openApp(page);
  const tools = await proposal(page, true);
  await page
    .locator('.intent')
    .last()
    .getByRole('button', { name: 'Open terminal', exact: true })
    .click();
  await expect(tools.locator('[data-tool-status="needs_confirmation"]')).toHaveCount(1);
  await expectAccessible(page, 'confirmation status EN');
});

test('stan potwierdzenia mieści się w wąskiej rozmowie', async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 800 });
  await openApp(page);
  const tools = await proposal(page);
  const size = await tools.evaluate((el) => ({ width: el.clientWidth, scroll: el.scrollWidth }));
  expect(size.scroll).toBeLessThanOrEqual(size.width + 1);
  await expectAccessible(page, 'stan potwierdzenia 360px');
  await page.screenshot({
    path: test.info().outputPath('confirmation-mobile.png'),
    fullPage: true,
  });
});
