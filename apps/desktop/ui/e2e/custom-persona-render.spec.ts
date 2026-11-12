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
  if (info.status !== info.expectedStatus) {
    const geometry = await page
      .locator('article.msg')
      .evaluateAll((messages) =>
        messages.map((message) => ({
          width: message.clientWidth,
          scroll: message.scrollWidth,
          overflow: [...message.querySelectorAll('*')]
            .filter(
              (el) => el.getBoundingClientRect().right > message.getBoundingClientRect().right,
            )
            .map((el) => ({
              tag: el.tagName,
              classes: el.getAttribute('class'),
              width: el.getBoundingClientRect().width,
              font: getComputedStyle(el).fontFamily,
              whiteSpace: getComputedStyle(el).whiteSpace,
            })),
        })),
      )
      .catch(() => []);
    await info.attach('overflow-geometry', {
      body: JSON.stringify(geometry),
      contentType: 'application/json',
    });
    await page.screenshot({ path: info.outputPath('failure.png') }).catch(() => undefined);
  }
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
  // Karta ładuje się leniwie; wymiar sprawdzamy także po jej pojawieniu się.
  await expect(
    message.getByRole('button', { name: 'Otwórz w oknie Sędziego Cieni', exact: true }),
  ).toBeVisible();
  expect(await message.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBeTruthy();
  await page.getByRole('button', { name: 'Włącz kokpit Jarvis', exact: true }).click();
  const person = page
    .getByRole('group', { name: 'Obsada rozmowy' })
    .getByRole('button', { name: new RegExp(id) });
  await expect(person).toBeVisible();
  expect(await person.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBeTruthy();
  expect(
    await person.evaluate(
      (el) => el.getBoundingClientRect().right <= el.parentElement!.getBoundingClientRect().right,
    ),
  ).toBeTruthy();
  await page.getByRole('button', { name: 'Mapa i systemy', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Mapa i systemy', exact: true });
  const entry = dialog.getByRole('button', { name: new RegExp(id) });
  await entry.scrollIntoViewIfNeeded();
  await expect(entry).toBeVisible();
  expect(await entry.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBeTruthy();
  await page.screenshot({ path: test.info().outputPath('custom-320.png') });
  await dialog.getByRole('button', { name: 'Wróć do rozmowy', exact: true }).first().click();
  await page.getByRole('button', { name: 'Adresatka: Auto', exact: true }).click();
  await page.getByRole('menuitemcheckbox', { name: id, exact: true }).click();
  const composer = page.locator('form.composer');
  expect(await composer.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBeTruthy();
  await page.screenshot({ path: test.info().outputPath('recipient-320.png') });
  expect(pageErrors.get(page)).toEqual([]);
});

for (const locale of ['pl', 'en'] as const) {
  test(`karta zgody na320px mieści pełny przycisk przy szerszym foncie — ${locale}`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 320, height: 844 });
    await openApp(page, {
      theme: 'dark',
      path: `/?scenario=custom-persona-long&locale=${locale}`,
    });
    await waitForChat(page);
    // W Cloud system-ui to OpenAI Sans; domyślne fonty Linuksa mają inne metryki.
    await page.addStyleTag({
      content: ':root { --alfa-font-sans: "DejaVu Sans", monospace; }',
    });
    const message = page.getByRole('article', { name: new RegExp('w'.repeat(64)) }).first();
    const label =
      locale === 'pl' ? 'Otwórz w oknie Sędziego Cieni' : 'Open in the Shadow Judge window';
    const button = message.getByRole('button', { name: label, exact: true });
    await expect(button).toBeVisible();
    expect((await button.boundingBox())!.height).toBeGreaterThanOrEqual(44);
    expect(await message.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBeTruthy();
    expect(await button.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBeTruthy();
    const card = button.locator('xpath=ancestor::article[1]');
    const title = card.getByText((await card.getAttribute('aria-label'))!, { exact: true });
    // Tytuł nie może być ściskany przez znacznik ryzyka do pionowej kolumny liter.
    expect((await title.boundingBox())!.width).toBeGreaterThanOrEqual(
      (await card.boundingBox())!.width / 2,
    );
    expect(
      await button.evaluate(
        (el) =>
          el.getBoundingClientRect().right <= el.closest('article')!.getBoundingClientRect().right,
      ),
    ).toBeTruthy();
    await page.screenshot({ path: test.info().outputPath(`approval-font-${locale}.png`) });
    expect(pageErrors.get(page)).toEqual([]);
  });
}
