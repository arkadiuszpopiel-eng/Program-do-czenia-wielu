import { expect, test } from '@playwright/test';
import { withDockingViewport } from '../e2e-app/docking-viewport';
import { openApp, waitForChat } from './helpers';

test.use({ viewport: { width: 1028, height: 779 } });

test('viewport dokowania: szuflada1028px, dokowanie1600px i przywrócenie', async ({ page }) => {
  await openApp(page, { path: '/?panels=docked' });
  await waitForChat(page);
  await page.getByRole('button', { name: 'Pokaż panel Sesje', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Sesje', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Schowaj panel Sesje (Ctrl+B)' })).toHaveCount(0);
  await withDockingViewport(page, async () => {
    const sessions = page.getByRole('navigation', { name: 'Sesje' });
    const collapse = page.getByRole('button', { name: 'Schowaj panel Sesje (Ctrl+B)' });
    await expect(collapse).toBeVisible();
    await collapse.click();
    await expect(sessions).toHaveCount(0);
    const edge = page.locator('.edge-tab.left');
    await expect(edge).toBeVisible();
    await edge.hover();
    const peek = page.getByRole('complementary', { name: 'Sesje' });
    await expect(peek).toBeVisible();
    await peek.getByRole('button', { name: 'Przypnij panel' }).click();
    await expect(sessions).toBeVisible();
    await expect(collapse).toBeVisible();
  });
  await expect.poll(() => page.evaluate(() => window.innerWidth)).toBe(1028);
});

test('viewport dokowania wraca także po błędzie scenariusza', async ({ page }) => {
  await page.goto('about:blank');
  await expect(
    withDockingViewport(page, async () => {
      throw new Error('kontrolowane przerwanie');
    }),
  ).rejects.toThrow('kontrolowane przerwanie');
  await expect
    .poll(() => page.evaluate(() => ({ width: window.innerWidth, height: window.innerHeight })))
    .toEqual({ width: 1028, height: 779 });
});

test('viewport dokowania usuwa emulację w kontekście bez zadanego viewportu', async ({
  browser,
}) => {
  // Preset Desktop Chrome podaje skalę1; kontekst bez viewportu nie może jej wymuszać.
  const context = await browser.newContext({ viewport: null, deviceScaleFactor: undefined });
  try {
    const page = await context.newPage();
    expect(page.viewportSize()).toBeNull();
    const before = await page.evaluate(() => ({
      width: window.innerWidth,
      height: window.innerHeight,
      dpr: window.devicePixelRatio,
    }));
    await withDockingViewport(page, async () => {
      await expect.poll(() => page.evaluate(() => window.innerWidth)).toBe(1600);
    });
    await expect
      .poll(() =>
        page.evaluate(() => ({
          width: window.innerWidth,
          height: window.innerHeight,
          dpr: window.devicePixelRatio,
        })),
      )
      .toEqual(before);
  } finally {
    await context.close();
  }
});
