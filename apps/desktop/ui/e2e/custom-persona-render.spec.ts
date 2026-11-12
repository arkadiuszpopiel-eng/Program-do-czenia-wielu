import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

const pageErrors = new WeakMap<Page, string[]>();
test.beforeEach(({ page }) => {
  const errors: string[] = [];
  pageErrors.set(page, errors);
  page.on('pageerror', (error) => errors.push(error.stack ?? error.message));
});
test.afterEach(async ({ page }, info) => {
  await info.attach('page-errors', {
    body: JSON.stringify(pageErrors.get(page)),
    contentType: 'application/json',
  });
  if (info.status !== info.expectedStatus)
    await page.screenshot({ path: info.outputPath('failure.png') }).catch(() => undefined);
});

for (const theme of ['light', 'dark'] as const) {
  test(`własna persona: rozmowa, kokpit i Atlas bez awarii — ${theme}`, async ({ page }) => {
    if (theme === 'dark') await page.setViewportSize({ width: 390, height: 844 });
    const errors = pageErrors.get(page)!;
    await openApp(page, { theme, scenario: 'custom-persona' });
    await waitForChat(page);
    await expect(page.getByRole('article', { name: /luna/ }).first()).toBeVisible();
    expect(errors).toEqual([]);
    await page.getByRole('button', { name: 'Włącz kokpit Jarvis', exact: true }).click();
    const crew = page.getByRole('group', { name: 'Obsada rozmowy' });
    await expect(crew.getByRole('button', { name: /luna/ })).toBeVisible();
    await page.getByRole('button', { name: 'Mapa i systemy', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Mapa i systemy', exact: true });
    await expect(dialog.getByRole('button', { name: /luna/ })).toBeVisible();
    await expectAccessible(page, `własna persona ${theme}`);
    await dialog.getByRole('button', { name: /luna/ }).click();
    await expect(page.getByRole('heading', { name: 'Kreator agentek', level: 2 })).toBeVisible();
    expect(errors).toEqual([]);
  });
}

test('persona z identyfikatorem 64 znaki mieści się na telefonie 320px', async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 844 });
  await openApp(page, { theme: 'dark', scenario: 'custom-persona-long' });
  await waitForChat(page);
  const id = 'w'.repeat(64);
  const message = page.getByRole('article', { name: new RegExp(id) }).first();
  await expect(message).toBeVisible();
  expect(await message.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBeTruthy();
  await page.getByRole('button', { name: 'Włącz kokpit Jarvis', exact: true }).click();
  const person = page
    .getByRole('group', { name: 'Obsada rozmowy' })
    .getByRole('button', { name: new RegExp(id) });
  await expect(person).toBeVisible();
  expect(await person.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBeTruthy();
  await page.getByRole('button', { name: 'Mapa i systemy', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Mapa i systemy', exact: true });
  const entry = dialog.getByRole('button', { name: new RegExp(id) });
  await expect(entry).toBeVisible();
  expect(await entry.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBeTruthy();
  await page.screenshot({ path: test.info().outputPath('custom-320.png') });
  await dialog.getByRole('button', { name: 'Wróć do rozmowy', exact: true }).first().click();
  await page.getByRole('button', { name: 'Adresatka: Auto', exact: true }).click();
  await page.getByRole('menuitemcheckbox', { name: id, exact: true }).click();
  const composer = page.locator('form.composer');
  expect(await composer.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBeTruthy();
  expect(pageErrors.get(page)).toEqual([]);
});
