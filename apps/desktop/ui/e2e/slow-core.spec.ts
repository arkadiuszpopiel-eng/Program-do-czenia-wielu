import { expect, test, type Page } from '@playwright/test';
import { openApp, waitForChat } from './helpers';

// Audyt stanów ładowania: w scenariuszu „slow-core” rdzeń odpowiada po 1,5 s, a atrapa zapisuje
// każde wywołanie komendy (`window.__alfaCalls`). Strona albo panel, który po otwarciu czeka na
// rdzeń, ma to pokazać (role=status: szkielet / „Ładowanie…”), a nie pustą kartę — PLAN §14.8:
// szkielet zamiast pustki, gdy ładowanie trwa dłużej niż 300 ms.

/** Komendy w tle (stan systemu, aktualizacje, Broker) — nie należą do otwieranego widoku. */
const BACKGROUND =
  /^(system|updates|broker|voice\.status|gui\.status|app\.saveLayout|triggers\.previewCron)\b/;

async function pendingSince(page: Page, since: number): Promise<string[]> {
  return page.evaluate(
    ([from, bg]) =>
      (window.__alfaCalls ?? [])
        .filter((c) => c.started >= from && c.ended === null && !new RegExp(bg).test(c.name))
        .map((c) => c.name),
    [since, BACKGROUND.source] as const,
  );
}

/** `scope` — kontener widoku (strona ustawień; panel zadokowany albo szuflada „Panel boczny”). */
async function check(page: Page, scope: string, label: string, act: () => Promise<void>) {
  const since = await page.evaluate(() => performance.now());
  await act();
  // Szkielet pojawia się po 300 ms; rdzeń odpowiada po 1500 ms.
  await page.waitForTimeout(700);
  const pending = await pendingSince(page, since);
  if (pending.length === 0) return null;
  const busy = await page.locator(`${scope} :is([role="status"], [aria-busy="true"])`).count();
  return busy > 0
    ? null
    : `${label}: czeka na ${[...new Set(pending)].join(', ')} bez stanu ładowania`;
}

test.describe.configure({ timeout: 180_000 });

test('strony ustawień pokazują stan ładowania, gdy rdzeń odpowiada wolno', async ({ page }) => {
  await openApp(page, { scenario: 'slow-core' });
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  await expect(nav.getByRole('button').first()).toBeVisible({ timeout: 10_000 });
  const names = await nav.getByRole('button').allInnerTexts();
  const report: string[] = [];
  for (const [i, raw] of names.entries()) {
    const name = raw.split('\n')[0]?.trim() ?? raw;
    const problem = await check(page, 'section[aria-labelledby="settings-page-title"]', name, () =>
      nav.getByRole('button').nth(i).click(),
    );
    if (problem) report.push(problem);
    await page.waitForTimeout(1_000);
  }
  expect(report, 'widoki bez stanu ładowania').toEqual([]);
});

test('panele pokazują stan ładowania, gdy rdzeń odpowiada wolno', async ({ page }) => {
  await openApp(page, { scenario: 'slow-core' });
  await waitForChat(page);
  await page.waitForTimeout(2_000);
  const report: string[] = [];
  for (const key of ['Alt+1', 'Alt+2', 'Alt+3', 'Alt+4', 'Alt+5', 'Alt+6', 'Alt+7']) {
    const problem = await check(page, ':is(aside.right, [role="dialog"])', key, () =>
      page.keyboard.press(key),
    );
    if (problem) report.push(problem);
    await page.waitForTimeout(1_000);
  }
  expect(report, 'panele bez stanu ładowania').toEqual([]);
});
