import { expect, test } from '@playwright/test';
import { openApp, waitForChat } from './helpers';
import { STYLES, THEMES, layoutDefects, watchConsole } from './layout';

// Przegląd układu „każde okno, każdy panel, każdy skrót” (zgłoszenie właściciela 2026-10-11):
// w każdym stylu i motywie przechodzi wszystkie strony Ustawień (w dwóch szerokościach okna),
// wszystkie panele boczne i wszystkie nakładki otwierane skrótem. Sprawdza rzeczy mierzalne:
// nic nie wystaje poza okno, strona nie przewija się w poziomie, żaden przycisk ani pole nie ma
// uciętego tekstu, nakładka zamyka się klawiszem Escape, konsola jest bez błędów.
// Dostępność (kontrast, role) sprawdzają `a11y.spec.ts` i `a11y-all.spec.ts`.

test.describe.configure({ timeout: 240_000 });

for (const style of STYLES) {
  for (const theme of THEMES) {
    test(`strony Ustawień bez wad układu — ${style} ${theme}`, async ({ page }) => {
      const errors = watchConsole(page);
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      await page.keyboard.press('Control+,');
      const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
      await expect(nav.locator('button.node').first()).toBeVisible();
      const count = await nav.locator('button.node').count();
      expect(count).toBeGreaterThan(20);
      const found: string[] = [];
      for (let i = 0; i < count; i += 1) {
        const node = nav.locator('button.node').nth(i);
        const name = ((await node.innerText()).split('\n')[0] ?? '').trim();
        await node.click();
        await expect(page.getByRole('heading', { level: 2, name })).toBeVisible();
        await page.waitForTimeout(150);
        for (const defect of await layoutDefects(page)) found.push(`${name} (1280): ${defect}`);
      }
      expect(found, 'wady układu stron Ustawień').toEqual([]);
      expect(errors, 'błędy w konsoli').toEqual([]);
    });

    test(`panele i nakładki ze skrótów — ${style} ${theme}`, async ({ page }) => {
      const errors = watchConsole(page);
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      const found: string[] = [];
      for (const key of ['Alt+1', 'Alt+2', 'Alt+3', 'Alt+4', 'Alt+5', 'Alt+6', 'Alt+7']) {
        await page.keyboard.press(key);
        await page.waitForTimeout(300);
        for (const defect of await layoutDefects(page)) found.push(`panel ${key}: ${defect}`);
      }
      // Nakładki otwierane skrótem: każda ma się pokazać jako okno dialogowe i zamknąć Escape.
      for (const key of ['Control+K', 'Control+/', 'Control+P']) {
        await page.keyboard.press(key);
        const dialog = page.getByRole('dialog').last();
        await expect(dialog, `skrót ${key} otwiera nakładkę`).toBeVisible();
        await page.waitForTimeout(200);
        for (const defect of await layoutDefects(page)) found.push(`nakładka ${key}: ${defect}`);
        await page.keyboard.press('Escape');
        await expect(page.getByRole('dialog'), `Escape zamyka nakładkę ${key}`).toHaveCount(0);
      }
      // Skróty, które nie otwierają nakładki, tylko przenoszą do pola: szukanie we wszystkich
      // sesjach (lista sesji) i szukanie w rozmowie (pasek nad rozmową, zamykany Escape).
      await page.keyboard.press('Control+Shift+F');
      await expect(page.locator('#alfa-sessions-search')).toBeFocused();
      await page.keyboard.press('Control+F');
      await expect(page.locator('#alfa-find-input')).toBeFocused();
      for (const defect of await layoutDefects(page)) found.push(`szukanie w rozmowie: ${defect}`);
      await page.keyboard.press('Escape');
      await expect(page.locator('#alfa-find-input')).toHaveCount(0);
      expect(found, 'wady układu paneli i nakładek').toEqual([]);
      expect(errors, 'błędy w konsoli').toEqual([]);
    });
  }
}

for (const style of STYLES) {
  for (const theme of THEMES) {
    test(`wyskakujące elementy belki i pola pisania — ${style} ${theme}`, async ({ page }) => {
      const errors = watchConsole(page);
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      const found: string[] = [];
      const check = async (label: string) => {
        await page.waitForTimeout(250);
        for (const defect of await layoutDefects(page)) found.push(`${label}: ${defect}`);
      };
      // Belka: objaśnienie stanu i szczegóły kosztów — otwierane myszką, zamykane Escape.
      for (const name of [/^Stan: /, /^Koszt sesji /]) {
        const button = page.getByRole('button', { name });
        await button.click();
        await expect(button, `${name} otwiera panel`).toHaveAttribute('aria-expanded', 'true');
        await check(`belka ${name}`);
        await page.keyboard.press('Escape');
        await expect(button, `Escape zamyka ${name}`).toHaveAttribute('aria-expanded', 'false');
      }
      // Pole pisania: podpowiedzi poleceń („/”) i agentek („@”).
      const composer = page.locator('#alfa-composer');
      for (const trigger of ['/', '@']) {
        await composer.click();
        await composer.fill('');
        await page.keyboard.type(trigger);
        await expect(
          page.getByRole('listbox').first(),
          `„${trigger}” otwiera podpowiedzi`,
        ).toBeVisible();
        await check(`podpowiedzi „${trigger}”`);
        await page.keyboard.press('Escape');
        await expect(page.getByRole('listbox')).toHaveCount(0);
        await composer.fill('');
      }
      // Prawy przycisk w polu pisania: menu Alfy (nie systemowe).
      await composer.click({ button: 'right' });
      const menu = page.getByRole('menu');
      await expect(menu).toBeVisible();
      await expect(menu.getByRole('menuitem', { name: 'Wklej' })).toBeVisible();
      await check('menu pola pisania');
      await page.keyboard.press('Escape');
      await expect(page.getByRole('menu')).toHaveCount(0);
      expect(found, 'wady układu elementów wyskakujących').toEqual([]);
      expect(errors, 'błędy w konsoli').toEqual([]);
    });
  }
}

test('strony Ustawień w wąskim oknie (900 px) — bez wad układu', async ({ page }) => {
  await page.setViewportSize({ width: 900, height: 800 });
  await openApp(page, { theme: 'light' });
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  await expect(nav.locator('button.node').first()).toBeVisible();
  const count = await nav.locator('button.node').count();
  const found: string[] = [];
  for (let i = 0; i < count; i += 1) {
    const node = nav.locator('button.node').nth(i);
    const name = ((await node.innerText()).split('\n')[0] ?? '').trim();
    await node.click();
    await expect(page.getByRole('heading', { level: 2, name })).toBeVisible();
    await page.waitForTimeout(150);
    for (const defect of await layoutDefects(page)) found.push(`${name} (900): ${defect}`);
  }
  expect(found, 'wady układu w wąskim oknie').toEqual([]);
});
