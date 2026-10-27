import { expect, test, type Page } from '@playwright/test';
import { openApp, waitForChat } from './helpers';
import { STYLES, THEMES, layoutDefects, watchConsole } from './layout';
import { overlapDefects } from './overlap';

// Przegląd całego interfejsu na nachodzenie elementów (zgłoszenie właściciela 2026-10-27: „czy nic
// nie wychodzi, nic się nie psuje, nie ma nachodzenia na siebie, teksty są równe i profile
// sprawne”). W każdym stylu i motywie: okno główne w szerokościach od 720 do 1920 px, wszystkie
// strony Ustawień, panele boczne i przełączanie profilu modelu. Wystawanie i ucięcie w poziomie
// sprawdza także `review-sweep.spec.ts`; tu dochodzi nachodzenie i ucięcie w pionie.

test.describe.configure({ timeout: 300_000 });

const WIDTHS = [1920, 1600, 1400, 1280, 1100, 1000, 900, 800, 720] as const;

async function defects(page: Page, label: string, found: string[]) {
  await page.waitForTimeout(200);
  for (const d of await layoutDefects(page)) found.push(`${label}: ${d}`);
  for (const d of await overlapDefects(page)) found.push(`${label}: ${d}`);
}

for (const style of STYLES) {
  for (const theme of THEMES) {
    test(`okno główne we wszystkich szerokościach — ${style} ${theme}`, async ({ page }) => {
      const errors = watchConsole(page);
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      const found: string[] = [];
      for (const width of WIDTHS) {
        await page.setViewportSize({ width, height: 860 });
        await defects(page, `okno ${width} px`, found);
      }
      expect(found, 'nachodzenie i wystawanie w oknie głównym').toEqual([]);
      expect(errors, 'błędy w konsoli').toEqual([]);
    });

    test(`strony Ustawień i panele bez nachodzenia — ${style} ${theme}`, async ({ page }) => {
      const errors = watchConsole(page);
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      const found: string[] = [];
      for (const [i, key] of [
        'Alt+1',
        'Alt+2',
        'Alt+3',
        'Alt+4',
        'Alt+5',
        'Alt+6',
        'Alt+7',
      ].entries())
        await page.keyboard.press(key).then(() => defects(page, `panel ${i + 1}`, found));
      await page.keyboard.press('Control+,');
      const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
      await expect(nav.locator('button.node').first()).toBeVisible();
      const count = await nav.locator('button.node').count();
      for (let i = 0; i < count; i += 1) {
        const node = nav.locator('button.node').nth(i);
        const name = ((await node.innerText()).split('\n')[0] ?? '').trim();
        await node.click();
        await expect(page.getByRole('heading', { level: 2, name })).toBeVisible();
        await defects(page, `Ustawienia → ${name}`, found);
      }
      expect(found, 'nachodzenie w Ustawieniach i panelach').toEqual([]);
      expect(errors, 'błędy w konsoli').toEqual([]);
    });

    test(`profile modelu: wybór działa, menu bez nachodzenia — ${style} ${theme}`, async ({
      page,
    }) => {
      const errors = watchConsole(page);
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      const found: string[] = [];
      for (const profile of ['Lokalny', 'Chmura', 'Hybryda']) {
        await page.getByRole('button', { name: /^Profil modelu: / }).click();
        const menu = page.getByRole('menu');
        await expect(menu).toBeVisible();
        await defects(page, `menu profilu (przed: ${profile})`, found);
        await menu.getByRole('menuitemcheckbox', { name: profile }).click();
        await expect(page.getByRole('menu')).toHaveCount(0);
        const button = page.getByRole('button', { name: `Profil modelu: ${profile}` });
        await expect(button, `wybrany profil ${profile}`).toBeVisible();
        await button.click();
        await expect(
          page.getByRole('menu').getByRole('menuitemcheckbox', { name: profile }),
          `${profile} zaznaczony w menu`,
        ).toHaveAttribute('aria-checked', 'true');
        await page.keyboard.press('Escape');
        await defects(page, `po wyborze ${profile}`, found);
      }
      expect(found, 'nachodzenie przy profilach').toEqual([]);
      expect(errors, 'błędy w konsoli').toEqual([]);
    });
  }
}
