// Noc 27/28.10 (0.0.75) w PRAWDZIWYM oknie (WebView2, CSP): menu „Tryb pracy” z belki, panele
// chowane ze znacznikami (przełącznik z palety, wysunięcie po najechaniu) i słownik w dwóch
// częściach — okna leniwe (każda sekcja Ustawień, panele) nie pokazują surowych kluczy tekstów.
import type { Page } from '@playwright/test';
import { plRest } from '../src/lib/i18n/pl-rest';
import { expect, inEverySkin, shot, test } from './alfa';

const REST_KEYS = Object.keys(plRest);

/** Teksty strony równe kluczowi z drugiej części słownika (tekst nie doszedł). */
async function rawKeys(page: Page): Promise<string[]> {
  const texts = await page.evaluate(() => {
    const out: string[] = [];
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const text = node.textContent?.trim();
      if (text) out.push(text);
    }
    for (const el of document.querySelectorAll('[aria-label], [title], [placeholder]')) {
      for (const name of ['aria-label', 'title', 'placeholder']) {
        const value = el.getAttribute(name)?.trim();
        if (value) out.push(value);
      }
    }
    return out;
  });
  const keys = new Set(REST_KEYS);
  return [...new Set(texts.filter((t) => keys.has(t)))];
}

test('belka, panele chowane i teksty okien leniwych w WebView2', async ({ alfa }) => {
  const { main } = alfa;
  const back = main.getByRole('button', { name: 'Wróć do rozmowy' });
  if (await back.isVisible()) await back.click();
  await expect(main.locator('#alfa-composer')).toBeVisible({ timeout: 30_000 });

  // Menu „Tryb pracy” z belki w każdym stylu i motywie: cztery tryby, Esc zamyka bez zmiany.
  const menu = main.getByRole('menu', { name: 'Tryb pracy' });
  await inEverySkin(main, async (skin, theme) => {
    await main.getByRole('button', { name: /^Tryb pracy: / }).click();
    await expect(menu.getByRole('menuitemradio')).toHaveCount(4);
    await shot(alfa, `42a-menu-trybu-${skin}-${theme}`);
    await main.keyboard.press('Escape');
    await expect(menu).toHaveCount(0);
  });

  // Panele chowane: przełącznik z palety, znacznik przy krawędzi, wysunięcie po najechaniu.
  const toggle = async () => {
    await main.keyboard.press('Control+k');
    await expect(main.locator('dialog[open] input')).toBeFocused();
    await main.keyboard.type('chowaj panele');
    await expect(main.locator('dialog [role=option][data-selected]')).toContainText(
      'Chowaj panele boczne automatycznie',
    );
    await main.keyboard.press('Enter');
  };
  const edge = main.getByRole('button', { name: /^Panel Sesje — najedź/ });
  await toggle();
  try {
    await expect(edge).toBeVisible();
    await shot(alfa, '42b-panele-schowane');
    const peek = main.getByRole('complementary', { name: 'Sesje' });
    const [w, h] = await main.evaluate(() => [window.innerWidth, window.innerHeight]);
    // Wysunięty panel nad rozmową w każdym stylu i motywie (kryjące tło warstwy, cień, znacznik).
    await inEverySkin(main, async (skin, theme) => {
      await edge.hover();
      await expect(peek).toBeVisible();
      await shot(alfa, `42c-panel-wysuniety-${skin}-${theme}`);
      await main.mouse.move(w! / 2, h! / 2);
      await expect(peek).toHaveCount(0);
    });
  } finally {
    await toggle();
  }
  await expect(edge).toHaveCount(0);

  // Panele leniwe z tekstami drugiej części słownika (Pamięć, Głos).
  for (const key of ['Alt+4', 'Alt+6']) {
    await main.keyboard.press(key);
    await main.waitForTimeout(500);
    expect(await rawKeys(main), `panel ${key}`).toEqual([]);
  }

  // Każda sekcja Ustawień: bez surowych kluczy.
  await main.keyboard.press('Control+,');
  const nav = main.getByRole('navigation', { name: 'Sekcje ustawień' });
  await expect(nav).toBeVisible();
  // Sekcje dochodzą po wczytaniu stron ustawień z rdzenia; zwinięte grupy — rozwinięte.
  const pages = nav.locator('button.node[data-page]');
  await expect.poll(() => pages.count(), { timeout: 15_000 }).toBeGreaterThan(10);
  for (const group of await nav.locator('button.group[aria-expanded="false"]').all()) {
    await group.click();
  }
  const ids = await pages.evaluateAll((els) => els.map((el) => el.getAttribute('data-page') ?? ''));
  const problems: string[] = [];
  for (const id of ids) {
    await nav.locator(`button.node[data-page="${id}"]`).click();
    await main.waitForTimeout(300);
    for (const key of await rawKeys(main)) problems.push(`${id}: ${key}`);
  }
  expect(problems).toEqual([]);
  await main.keyboard.press('Escape');
});
