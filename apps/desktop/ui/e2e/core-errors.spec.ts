import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

// Audyt obsługi błędów w całym interfejsie: w scenariuszu „core-errors” rdzeń odrzuca każdą komendę
// poza startem. Każda strona ustawień i każdy panel ma pokazać błąd na miejscu — bez wyjątku strony
// (`pageerror`) i bez odrzucenia, które dotarło dopiero do siatki bezpieczeństwa w `main.ts`.

const UNHANDLED = '[alfa] nieobsłużone odrzucenie';

function watch(page: Page) {
  const unhandled: string[] = [];
  const crashes: string[] = [];
  page.on('console', (m) => {
    if (m.text().startsWith(UNHANDLED)) unhandled.push(m.text());
  });
  page.on('pageerror', (e) => crashes.push(e.message));
  return { unhandled, crashes };
}

/** Co widać po błędzie: komunikaty „Nie udało się…” (role=alert) w widoku i treści toastów. */
async function visible(page: Page, scope: string) {
  const alerts = await page.locator(`${scope} [role="alert"]`).count();
  const toasts = await page.locator('.toasts').allInnerTexts();
  return {
    alerts,
    toasts: toasts
      .join(' ')
      .split('\n')
      .filter((t) => t.trim()),
  };
}

async function settle(page: Page) {
  await page.evaluate(
    () => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))),
  );
  await page.waitForTimeout(150);
}

test('wszystkie strony ustawień: błąd rdzenia widoczny na miejscu, bez nieobsłużonych odrzuceń', async ({
  page,
}) => {
  const seen = watch(page);
  await openApp(page, { scenario: 'core-errors' });
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  await expect(nav.getByRole('button').first()).toBeVisible();
  const names = await nav.getByRole('button').allInnerTexts();
  expect(names.length).toBeGreaterThan(20);
  const report: string[] = [];
  const shown: string[] = [];
  for (const [i, raw] of names.entries()) {
    const name = raw.split('\n')[0]?.trim() ?? raw;
    const before = seen.unhandled.length + seen.crashes.length;
    const toastsBefore = (await visible(page, 'main')).toasts;
    await nav.getByRole('button').nth(i).click();
    await expect(page.getByRole('heading', { level: 2, name })).toBeVisible();
    await settle(page);
    const v = await visible(page, 'section[aria-labelledby="settings-page-title"]');
    const fresh = v.toasts.filter((t) => !toastsBefore.includes(t)).length;
    shown.push(`${name}\t${v.alerts}\t${fresh}`);
    const after = seen.unhandled.length + seen.crashes.length;
    if (after > before)
      report.push(`${name}: ${[...seen.unhandled, ...seen.crashes].slice(before).join(' | ')}`);
  }
  await test.info().attach('strony-ustawien.tsv', {
    body: `strona\tkomunikaty\ttoasty\n${shown.join('\n')}\n`,
    contentType: 'text/tab-separated-values',
  });
  if (process.env.ALFA_AUDIT_OUT) console.log(shown.join('\n'));
  expect(report, 'strony z nieobsłużonym błędem').toEqual([]);
  await expectAccessible(page, 'ustawienia — błędy rdzenia');
});

test('panele i rozmowa: błąd rdzenia widoczny na miejscu, bez nieobsłużonych odrzuceń', async ({
  page,
}) => {
  const seen = watch(page);
  await openApp(page, { scenario: 'core-errors' });
  await waitForChat(page);
  const report: string[] = [];
  for (const key of ['Alt+1', 'Alt+2', 'Alt+3', 'Alt+4', 'Alt+5', 'Alt+6', 'Alt+7']) {
    const before = seen.unhandled.length + seen.crashes.length;
    await page.keyboard.press(key);
    await settle(page);
    const after = seen.unhandled.length + seen.crashes.length;
    if (after > before)
      report.push(`${key}: ${[...seen.unhandled, ...seen.crashes].slice(before).join(' | ')}`);
  }
  const composer = page.locator('#alfa-composer');
  await composer.fill('Test błędu rdzenia');
  await composer.press('Enter');
  await settle(page);
  await expect(composer).toHaveValue('Test błędu rdzenia');
  expect(report, 'panele z nieobsłużonym błędem').toEqual([]);
  expect(seen.crashes, 'wyjątki strony').toEqual([]);
  expect(seen.unhandled, 'nieobsłużone odrzucenia').toEqual([]);
});
