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
  // Przejścia CSS (także 0,01 ms przy reduced motion) kończą się dopiero w kolejnej klatce, a axe liczy
  // kontrast ze stylu bieżącego — np. przycisk wychodzący z `disabled` (przezroczystość 0,5) dawał w CI
  // fałszywe `color-contrast`. Czekamy na koniec animacji (z limitem, żeby nic nie zawisło).
  await page
    .waitForFunction(() => document.getAnimations().every((a) => a.playState !== 'running'), null, {
      timeout: 2_000,
    })
    .catch(() => undefined);
  await page.evaluate(
    () => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))),
  );
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
