import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';
import { STYLES, THEMES, layoutDefects, watchConsole } from './layout';
import { overlapDefects } from './overlap';

// Panele chowane (właściciel 27.10: „lewy i prawy panel mają się chować automatycznie, wysuwać
// i mieć znaczniki, że tam są”). Każdy styl i motyw: znacznik przy krawędzi, wysunięcie po
// najechaniu, schowanie po zjechaniu, przypięcie i ustawienie „Panele chowają się same”.

const tab = (page: Page, name: RegExp) => page.getByRole('button', { name });
const SESSIONS = /^Panel Sesje — najedź/;
const RIGHT = /^Panel prawy — najedź/;

async function clean(page: Page, label: string) {
  await page.waitForTimeout(250);
  expect(await layoutDefects(page), `wystawanie: ${label}`).toEqual([]);
  expect(await overlapDefects(page), `nachodzenie: ${label}`).toEqual([]);
}

for (const style of STYLES) {
  for (const theme of THEMES) {
    test(`schowany panel ma znacznik i wysuwa się — ${style} ${theme}`, async ({ page }) => {
      const errors = watchConsole(page);
      await page.setViewportSize({ width: 1600, height: 900 });
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      // Panele zadokowane: bez znaczników.
      await expect(tab(page, SESSIONS)).toHaveCount(0);
      await expect(tab(page, RIGHT)).toHaveCount(0);

      await page.getByRole('button', { name: 'Ukryj panel Sesje' }).click();
      await expect(tab(page, SESSIONS)).toBeVisible();
      await tab(page, SESSIONS).hover();
      const peek = page.getByRole('complementary', { name: 'Sesje' });
      await expect(peek).toBeVisible();
      await expect(peek.getByRole('button', { name: 'Przypnij panel' })).toBeVisible();
      await clean(page, `wysunięty lewy ${style} ${theme}`);
      await expectAccessible(page, `wysunięty lewy ${style} ${theme}`);

      // Zjechanie myszą na środek rozmowy chowa panel.
      await page.mouse.move(800, 450);
      await expect(peek).toHaveCount(0);

      // Przypięcie z wysuniętego panelu przywraca stały układ.
      await tab(page, SESSIONS).hover();
      await peek.getByRole('button', { name: 'Przypnij panel' }).click();
      await expect(peek).toHaveCount(0);
      await expect(tab(page, SESSIONS)).toHaveCount(0);
      await expect(page.getByRole('button', { name: 'Ukryj panel Sesje' })).toBeVisible();
      expect(errors, 'błędy w konsoli').toEqual([]);
    });
  }
}

test('„Panele chowają się same”: oba schowane, przełącznik w pasku i Esc', async ({ page }) => {
  const errors = watchConsole(page);
  await page.setViewportSize({ width: 1600, height: 900 });
  await openApp(page, { path: '/?panels=autohide' });
  await waitForChat(page);
  await expect(tab(page, SESSIONS)).toBeVisible();
  await expect(tab(page, RIGHT)).toBeVisible();
  await clean(page, 'oba schowane');

  // Przycisk panelu prawego w pasku wysuwa go; Esc chowa.
  await page.getByRole('button', { name: 'Pokaż panel prawy' }).click();
  const peek = page.getByRole('complementary', { name: 'Panel boczny' });
  await expect(peek).toBeVisible();
  await clean(page, 'wysunięty prawy');
  await page.keyboard.press('Escape');
  await expect(peek).toHaveCount(0);

  // Kliknięcie obok chowa wysunięty panel.
  await tab(page, RIGHT).hover();
  await expect(peek).toBeVisible();
  await page.mouse.click(700, 300);
  await expect(peek).toHaveCount(0);

  // Przypnij: ustawienie się wyłącza, panel zostaje zadokowany.
  await tab(page, RIGHT).click();
  await expect(peek).toBeVisible();
  await peek.getByRole('button', { name: 'Przypnij panel' }).click();
  await expect(tab(page, RIGHT)).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Ukryj panel prawy' })).toBeVisible();
  expect(errors, 'błędy w konsoli').toEqual([]);
});

test('paleta (Ctrl+K): „Chowaj panele boczne automatycznie” włącza i wyłącza', async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await openApp(page);
  await waitForChat(page);
  const toggle = async () => {
    await page.keyboard.press('Control+k');
    await expect(page.locator('dialog[open] input')).toBeFocused();
    await page.keyboard.type('chowaj panele');
    await expect(page.locator('dialog [role=option][data-selected]')).toContainText(
      'Chowaj panele boczne automatycznie',
    );
    await page.keyboard.press('Enter');
  };
  await toggle();
  await expect(tab(page, SESSIONS)).toBeVisible();
  await expect(tab(page, RIGHT)).toBeVisible();
  await toggle();
  await expect(tab(page, SESSIONS)).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Ukryj panel Sesje' })).toBeVisible();
});

test('wąskie okno: znacznik otwiera szufladę kliknięciem', async ({ page }) => {
  await page.setViewportSize({ width: 900, height: 800 });
  await openApp(page);
  await waitForChat(page);
  await expect(tab(page, SESSIONS)).toBeVisible();
  await clean(page, 'wąskie okno');
  await tab(page, SESSIONS).click();
  await expect(page.getByRole('dialog', { name: 'Sesje' })).toBeVisible();
});
