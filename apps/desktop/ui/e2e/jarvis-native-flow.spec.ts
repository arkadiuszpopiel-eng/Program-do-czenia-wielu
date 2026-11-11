import { expect, test } from '@playwright/test';
import { jarvisFlow } from '../e2e-app/jarvis-flow';
import { openApp, waitForChat } from './helpers';

for (const width of [1028, 1600]) {
  test(`przepływ Jarvisa WebView: szuflada lub dokowanie — ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 779 });
    await openApp(page);
    await waitForChat(page);
    await jarvisFlow(page, () => {});
    await page.keyboard.press('Alt+6');
    await expect(page.getByText('Mikrofon wyłączony.', { exact: true })).toBeVisible();
  });
}
