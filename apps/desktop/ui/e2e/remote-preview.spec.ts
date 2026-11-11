import { expect, test } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

test('panel telefonu nie udaje połączenia ani nie odblokowuje poleceń', async ({ page }) => {
  const sockets: string[] = [];
  page.on('websocket', (ws) => sockets.push(ws.url()));
  await openApp(page);
  await waitForChat(page);
  await page.getByRole('button', { name: 'Włącz kokpit Jarvis', exact: true }).click();
  const launch = page.getByRole('button', { name: 'Podgląd panelu telefonu', exact: true });
  await launch.click({ timeout: 5000 });
  const dialog = page.getByRole('dialog', { name: 'Panel telefonu — podgląd', exact: true });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('status')).toContainText('Brak połączenia');
  await expect(
    dialog.getByRole('button', { name: 'Sparuj urządzenie', exact: true }),
  ).toBeDisabled();
  const actions = dialog.locator('[data-remote-action]');
  await expect(actions).toHaveCount(8);
  for (const action of await actions.all()) await expect(action).toBeDisabled();
  for (const button of await dialog.getByRole('button').all()) {
    const box = await button.boundingBox();
    expect(box?.height, 'cel dotykowy podglądu telefonu').toBeGreaterThanOrEqual(44);
    expect(box?.width, 'cel dotykowy podglądu telefonu').toBeGreaterThanOrEqual(44);
  }
  expect(sockets).toEqual([]);
  await expectAccessible(page, 'panel telefonu bez połączenia');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  await expect(launch).toBeFocused();
});

for (const width of [320, 390, 1280]) {
  for (const theme of ['light', 'dark'] as const) {
    test(`panel telefonu ${width}px ${theme} jest czytelny i zablokowany`, async ({ page }) => {
      await page.setViewportSize({ width, height: width < 400 ? 740 : 900 });
      await openApp(page, { theme, path: `/?theme=${theme}` });
      await waitForChat(page);
      await page.getByRole('button', { name: 'Włącz kokpit Jarvis', exact: true }).click();
      await page.getByRole('button', { name: 'Podgląd panelu telefonu', exact: true }).click();
      const dialog = page.getByRole('dialog', { name: 'Panel telefonu — podgląd', exact: true });
      await expect(dialog).toHaveAttribute('data-connection', 'unconfigured');
      const box = await dialog.boundingBox();
      expect(box && box.x >= 0 && box.x + box.width <= width + 1).toBeTruthy();
      await page.screenshot({ path: test.info().outputPath('remote-preview.png') });
      await expectAccessible(page, `podgląd telefonu ${width} ${theme}`);
      await dialog.getByRole('button', { name: 'Wróć do kokpitu', exact: true }).click();
      await expect(dialog).toHaveCount(0);
    });
  }
}

test('podgląd telefonu ma angielskie etykiety i nie twierdzi, że jest połączony', async ({
  page,
}) => {
  await openApp(page, { path: '/?locale=en' });
  await waitForChat(page);
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  await page.getByRole('button', { name: 'Enable Jarvis cockpit', exact: true }).click();
  await page.getByRole('button', { name: 'Preview phone panel', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Phone panel — preview', exact: true });
  await expect(dialog.getByRole('status')).toContainText('Not connected');
  await expect(dialog.getByRole('button', { name: 'Pair device', exact: true })).toBeDisabled();
  await expectAccessible(page, 'phone preview English');
});
