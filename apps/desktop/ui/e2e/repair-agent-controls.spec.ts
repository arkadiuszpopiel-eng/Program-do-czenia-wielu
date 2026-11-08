import { expect, test } from '@playwright/test';
import { waitForChat } from './helpers';

test.beforeEach(async ({ page }) => {
  await page.goto('/?theme=dark');
  await waitForChat(page);
});

test('każda agentka otwiera własne menu, a obsada jest wyśrodkowana', async ({ page }) => {
  const cast = page.getByRole('group', { name: 'Obsada' });
  expect(await cast.evaluate((el) => el.closest('[data-tauri-drag-region]'))).toBeNull();
  const box = await cast.boundingBox();
  expect(box).not.toBeNull();
  expect(Math.abs(box!.x + box!.width / 2 - 640)).toBeLessThan(4);
  for (const name of ['Alfa', 'Beta', 'Gama', 'Delta']) {
    await cast.getByRole('button', { name: new RegExp(name) }).click();
    await expect(page.getByRole('menuitem', { name: `Ustawienia agentki ${name}` })).toBeVisible();
    await page.keyboard.press('Escape');
  }
});

test('ustawienia są po prawej i dopuszczają 500 kroków', async ({ page }) => {
  await page.keyboard.press('Control+,');
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  const box = await nav.boundingBox();
  expect(box!.x).toBeGreaterThan(900);
  await nav.getByRole('button', { name: 'Agentki', exact: true }).click();
  const steps = page.getByRole('spinbutton', { name: 'Limit kroków zadania' });
  await expect(steps).toHaveAttribute('max', '500');
  await steps.fill('500');
  await steps.blur();
  await expect(steps).toHaveValue('500');
  await expect(page.getByTestId('agent-permissions')).toBeVisible();
});

test('pipeline: etapy nie kurczą się i mieszczą się także na wąskim ekranie', async ({ page }) => {
  await page.getByTestId('open-pipelines-btn').click();
  const dialog = page.getByTestId('pipelines-dialog');
  await expect(dialog.getByTestId('pipeline-stage')).toHaveCount(1);
  await expect(dialog.getByRole('navigation', { name: 'Etapy pipeline’u' })).toBeVisible();
  await dialog.getByRole('button', { name: /2.*Architektura/ }).click();
  await expect(dialog.getByLabel('Nazwa etapu')).toHaveValue('Architektura komponentów');
  await page.screenshot({ path: 'test-results/pipeline-desktop.png', fullPage: true });
  await page.setViewportSize({ width: 640, height: 700 });
  const overflow = await dialog.evaluate((el) => el.scrollWidth > el.clientWidth + 1);
  expect(overflow).toBe(false);
  await page.screenshot({ path: 'test-results/pipeline-mobile.png', fullPage: true });
});

test('menu Bety prowadzi do jej profilu, a zmiana profilu pozostaje po powrocie', async ({
  page,
}) => {
  await page.getByRole('group', { name: 'Obsada' }).getByRole('button', { name: /Beta/ }).click();
  await page.getByRole('menuitem', { name: 'Ustawienia agentki Beta' }).click();
  const access = page.getByTestId('agent-permissions');
  await expect(access.getByRole('heading', { name: /Beta/ })).toBeVisible();
  await access.getByRole('combobox').click();
  await page.getByRole('option', { name: 'Bez narzędzi', exact: true }).click();
  await page.keyboard.press('Control+,');
  await page.keyboard.press('Control+,');
  await expect(access.getByRole('combobox')).toContainText('Bez narzędzi');
  await access.getByRole('button', { name: 'Role w sesji i katalog roboczy' }).click();
  await expect(page.locator('#alfa-composer')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Zmień role agentki Beta' })).toBeVisible();
});

test('obsada nie nachodzi na stan mikrofonu i pozostałe wskaźniki', async ({ page }) => {
  for (const width of [1280, 1000, 800]) {
    await page.setViewportSize({ width, height: 800 });
    const cast = await page.getByRole('group', { name: 'Obsada' }).boundingBox();
    for (const selector of ['.mic', '.badge', '.gui', '.cam']) {
      const indicator = page.locator(`header.titlebar ${selector}`);
      if (await indicator.count()) {
        const box = await indicator.boundingBox();
        if (box)
          expect(box.x, `${selector} przy ${width}px`).toBeGreaterThanOrEqual(
            cast!.x + cast!.width,
          );
      }
    }
  }
});

test('pipeline zachowuje szkic po zamknięciu, a powielanie tworzy osobną kopię', async ({
  page,
}) => {
  await page.getByTestId('open-pipelines-btn').click();
  const dialog = page.getByTestId('pipelines-dialog');
  await dialog.getByTestId('pipeline-name').fill('szkic-test');
  await dialog.getByRole('button', { name: 'Zamknij', exact: true }).click();
  await page.getByTestId('open-pipelines-btn').click();
  await expect(dialog.getByTestId('pipeline-name')).toHaveValue('szkic-test');
  await dialog.locator('.toolbar').getByRole('button', { name: 'Powiel', exact: true }).click();
  await expect(dialog.getByTestId('pipeline-name')).not.toHaveValue('szkic-test');
  await dialog.getByTestId('pipeline-new').click();
  await expect(dialog.getByTestId('pipeline-name')).toHaveValue('');
});
