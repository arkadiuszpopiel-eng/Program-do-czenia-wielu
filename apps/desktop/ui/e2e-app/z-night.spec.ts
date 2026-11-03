// Noc 27/28.10 (0.0.75) i 29.10 (0.0.76) w PRAWDZIWYM oknie (WebView2, CSP): menu „Tryb pracy”
// z paska pisania (zaznaczony tryb = tryb z rdzenia), panele chowane w całości ze znacznikami
// (przycisk w nagłówku, przełącznik z palety, wysunięcie po najechaniu, przypięcie) i słownik w dwóch
// częściach — okna leniwe (każda sekcja Ustawień, panele) nie pokazują surowych kluczy tekstów.
import type { Page } from '@playwright/test';
import { plRest } from '../src/lib/i18n/pl-rest';
import { expect, inEverySkin, shot, test } from './alfa';
import { call } from './ipc-call';

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

  // Menu „Tryb pracy” z paska pisania w każdym stylu i motywie: cztery tryby, zaznaczony ten sam co
  // na żetonie (tryb czytany z rdzenia — 29.10 menu pokazywało stary), Esc zamyka bez zmiany.
  const menu = main.getByRole('menu', { name: 'Tryb pracy' });
  const composer = main.locator('form.composer');
  await expect(main.locator('header.titlebar .cluster')).toHaveCount(0);
  await inEverySkin(main, async (skin, theme) => {
    const chip = composer.getByRole('button', { name: /^Tryb pracy: / });
    const label = (await chip.getAttribute('aria-label')) ?? '';
    const mode = /^Tryb pracy: ([^.]+)\./.exec(label)?.[1] ?? '?';
    await chip.click();
    await expect(menu.getByRole('menuitemradio')).toHaveCount(5);
    await expect(menu.getByRole('menuitemradio', { checked: true })).toContainText(mode);
    await shot(alfa, `42a-menu-trybu-${skin}-${theme}`);
    await main.keyboard.press('Escape');
    await expect(menu).toHaveCount(0);
    // Okienko stanu i menu adresatki z paska pisania (okienka nad rozmową).
    await composer.getByRole('button', { name: /^Stan: / }).click();
    const details = main.getByRole('dialog', { name: 'Stan Alfy' });
    await expect(details).toBeVisible();
    await shot(alfa, `42d-okienko-stanu-${skin}-${theme}`);
    await main.keyboard.press('Escape');
    await expect(details).toHaveCount(0);
    await main
      .locator('form.composer')
      .getByRole('button', { name: /^Adresatka: / })
      .click();
    const models = main.getByRole('menu', { name: /^Adresatka: / });
    await expect(models).toBeVisible();
    await shot(alfa, `42e-menu-adresatki-${skin}-${theme}`);
    await main.keyboard.press('Escape');
    await expect(models).toHaveCount(0);
  });

  const toggle = async () => {
    await main.keyboard.press('Control+k');
    await expect(main.locator('dialog[open] input')).toBeFocused();
    await main.keyboard.type('chowaj panele');
    await expect(main.locator('dialog [role=option][data-selected]')).toContainText(
      'Chowaj panele boczne automatycznie',
    );
    await main.keyboard.press('Enter');
  };
  // Od 29.10 panele chowają się domyślnie: ta część startuje z zadokowanymi, a na końcu wraca
  // ustawienie sprzed testu.
  const autohide = (await call<Record<string, unknown>>(main, 'settings_values'))[
    'ui.panels_autohide'
  ];
  if (autohide !== false) await toggle();

  // Sesje chowane w całości przyciskiem w nagłówku: znacznik, wysunięcie, „Przypnij panel”.
  const sessions = main.getByRole('navigation', { name: 'Sesje' });
  await main.getByRole('button', { name: 'Schowaj panel Sesje (Ctrl+B)' }).click();
  await expect(sessions).toHaveCount(0);
  const leftEdge = main.locator('.edge-tab.left');
  await expect(leftEdge).toBeVisible();
  await leftEdge.hover();
  const pinned = main.getByRole('complementary', { name: 'Sesje' });
  await expect(pinned).toBeVisible();
  await shot(alfa, '43a-sesje-wysuniete');
  await pinned.getByRole('button', { name: 'Przypnij panel' }).click();
  await expect(sessions).toBeVisible();
  await expect(leftEdge).toHaveCount(0);

  // Panele chowane: przełącznik z palety, znacznik przy krawędzi, wysunięcie po najechaniu.
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
  if (autohide !== false) await toggle();

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
