import AxeBuilder from '@axe-core/playwright';
import { expect, type Page } from '@playwright/test';

/** Otwiera okno z atrapą w danym scenariuszu i motywie; czeka na gotowy widok. */
export async function openApp(
  page: Page,
  opts: { scenario?: string; theme?: 'light' | 'dark'; path?: string } = {},
) {
  await page.emulateMedia({ colorScheme: opts.theme ?? 'light', reducedMotion: 'reduce' });
  const query = opts.scenario ? `?scenario=${opts.scenario}` : '';
  await page.goto(`${opts.path ?? '/'}${query}`);
}

export async function waitForChat(page: Page) {
  await expect(page.locator('#alfa-composer')).toBeVisible();
}

/** 0 naruszeń critical/serious (kryterium F1, ACC-F1-ui-shell-02). */
export async function expectAccessible(page: Page, label: string) {
  const results = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
    .analyze();
  const blocking = results.violations.filter(
    (v) => v.impact === 'critical' || v.impact === 'serious',
  );
  const summary = blocking.map(
    (v) =>
      `${v.id} (${v.impact}): ${v.nodes
        .map((n) => n.target.join(' '))
        .slice(0, 3)
        .join(' | ')}`,
  );
  expect(summary, `axe: ${label}`).toEqual([]);
}
