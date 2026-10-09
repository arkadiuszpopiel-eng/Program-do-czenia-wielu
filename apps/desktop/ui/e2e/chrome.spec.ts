import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { waitForChat } from './helpers';

// Zgłoszenia właściciela (2026-10-09): górna belka z jadącym paskiem LED i własnymi przyciskami
// okna (belka systemowa zniknęła), dopracowana nawigacja Ustawień, role kart graficznych.

const SKINS = ['classic', 'glass', 'next'] as const;
const THEMES = ['light', 'dark'] as const;

async function open(page: Page, query = '', motion: 'reduce' | 'no-preference' = 'reduce') {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: motion });
  await page.goto(`/${query}`);
  await waitForChat(page);
}

const nav = (page: Page) => page.getByRole('navigation', { name: 'Sekcje ustawień' });
const node = (page: Page, name: string) => nav(page).getByRole('button', { name, exact: true });

test('pasek LED jedzie od lewej do prawej, a bez animacji stoi jako równa listwa', async ({
  page,
}) => {
  await open(page, '', 'no-preference');
  const led = page.locator('header.titlebar .led');
  await expect(led).toBeVisible();
  const light = () =>
    led.evaluate((el) => {
      const style = getComputedStyle(el, '::after');
      return { name: style.animationName, x: new DOMMatrix(style.transform).m41 };
    });
  const first = await light();
  expect(first.name).not.toBe('none');
  await page.waitForTimeout(400);
  const second = await light();
  expect(second.x, 'światło przesuwa się w prawo').toBeGreaterThan(first.x);
  // Pasek leży na całej szerokości belki i nie przechwytuje kliknięć.
  const box = await led.boundingBox();
  expect(box?.width).toBe(page.viewportSize()?.width);
  expect(await led.evaluate((el) => getComputedStyle(el).pointerEvents)).toBe('none');

  // Animacje wyłączone w Ustawieniach → światło znika, listwa zostaje.
  await page.evaluate(() => (document.documentElement.dataset['motion'] = 'off'));
  expect(await led.evaluate((el) => getComputedStyle(el, '::after').display)).toBe('none');
  expect(
    Number(await led.evaluate((el) => getComputedStyle(el, '::before').opacity)),
  ).toBeGreaterThan(0.5);
});

for (const skin of SKINS) {
  for (const theme of THEMES) {
    test(`przyciski okna i nawigacja ustawień są czytelne — ${skin}/${theme}`, async ({ page }) => {
      await open(page, `?style=${skin}&theme=${theme}`);
      const controls = page.getByRole('group', { name: 'Okno' });
      for (const name of ['Minimalizuj', 'Maksymalizuj', 'Zamknij']) {
        await expect(controls.getByRole('button', { name, exact: true })).toBeVisible();
      }
      // Przyciski stoją przy prawej krawędzi, zamknięcie jest ostatnie.
      const close = await controls.getByRole('button', { name: 'Zamknij' }).boundingBox();
      expect(Math.round((close?.x ?? 0) + (close?.width ?? 0))).toBe(page.viewportSize()?.width);
      await controls.getByRole('button', { name: 'Maksymalizuj' }).click();
      await expect(controls.getByRole('button', { name: 'Przywróć rozmiar' })).toBeVisible();

      await page.keyboard.press('Control+,');
      await expect(node(page, 'Ogólne')).toHaveAttribute('aria-current', 'page');
      await node(page, 'Wygląd').hover();
      await page.getByLabel('Szukaj ustawień').fill('mikrofon');
      await expect(nav(page).locator('button.node')).not.toHaveCount(0);
      await expectReadable(page, 'aside.nav');
      await page.getByRole('button', { name: 'Wyczyść szukanie' }).click();
      await page.getByRole('button', { name: 'Grupa: Głos' }).click();
      await expectReadable(page, 'aside.nav');
    });
  }
}

test('nawigacja ustawień: grupy, szukanie, klawiatura, pamięć zwinięcia', async ({ page }) => {
  await open(page);
  await page.keyboard.press('Control+,');
  // Każda strona ma ikonę; nagłówki grup nie udają stron.
  const pages = nav(page).locator('button.node');
  await expect(pages.first()).toBeVisible();
  const count = await pages.count();
  expect(count).toBeGreaterThan(25);
  await expect(nav(page).locator('button.node svg')).toHaveCount(count);

  // Zwinięcie grupy chowa jej strony i pokazuje ich liczbę; stan wraca po ponownym otwarciu.
  const voice = page.getByRole('button', { name: 'Grupa: Głos' });
  await voice.click();
  await expect(voice).toHaveAttribute('aria-expanded', 'false');
  await expect(node(page, 'Urządzenia')).toBeHidden();
  await expect(voice.locator('.count')).toHaveText('2');
  await page.reload();
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await expect(page.getByRole('button', { name: 'Grupa: Głos' })).toHaveAttribute(
    'aria-expanded',
    'false',
  );

  // Ctrl+F przenosi do wyszukiwarki; lista pokazuje tylko strony z trafieniami i ich liczbę.
  await page.keyboard.press('Control+f');
  const search = page.getByLabel('Szukaj ustawień');
  await expect(search).toBeFocused();
  await search.fill('mikrofon');
  await expect(node(page, 'Głos')).toBeVisible();
  await expect(node(page, 'Głos').locator('.hits')).toHaveText(/^\d+$/);
  await expect(node(page, 'Wygląd')).toBeHidden();
  // Strzałka w dół wchodzi na listę, dalej strzałki i End; Enter otwiera stronę i czyści szukanie.
  await search.press('ArrowDown');
  await expect(pages.first()).toBeFocused();
  await page.keyboard.press('End');
  await expect(pages.last()).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('Home');
  await expect(pages.first()).toBeFocused();
  const opened = (await pages.first().locator('.label').innerText()).trim();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { level: 2, name: opened })).toBeVisible();
  await expect(search).toHaveValue('');

  // Pierwszy Escape czyści szukanie i zostawia Ustawienia otwarte.
  await search.fill('qqqq');
  await expect(nav(page).getByText('Żadna strona nie pasuje.')).toBeVisible();
  await search.press('Escape');
  await expect(search).toHaveValue('');
  await expect(nav(page)).toBeVisible();

  // Wejście na stronę ze zwiniętej grupy (z wyników) rozwija tę grupę.
  await search.fill('redukcja szumów');
  await node(page, 'Głos').click();
  await expect(page.getByRole('heading', { level: 2, name: 'Głos' })).toBeVisible();
  await expect(node(page, 'Głos')).toHaveAttribute('aria-current', 'page');
  await expect(page.getByRole('button', { name: 'Grupa: Głos' })).toHaveAttribute(
    'aria-expanded',
    'true',
  );

  // Zwiń / rozwiń wszystko.
  await page.getByRole('button', { name: 'Zwiń wszystkie grupy' }).click();
  await expect(pages).toHaveCount(count);
  await expect(node(page, 'Ogólne')).toBeHidden();
  await page.getByRole('button', { name: 'Rozwiń wszystkie grupy' }).click();
  await expect(node(page, 'Ogólne')).toBeVisible();
});

test('wąskie okno: lista stron chowa się pod przyciskiem bieżącej strony', async ({ page }) => {
  await page.setViewportSize({ width: 560, height: 800 });
  await open(page);
  await page.keyboard.press('Control+,');
  const current = page.getByRole('button', { name: /^Sekcja: Ogólne/ });
  await expect(current).toBeVisible();
  await expect(nav(page)).toBeHidden();
  await current.click();
  await node(page, 'Wygląd').click();
  await expect(page.getByRole('heading', { level: 2, name: 'Wygląd' })).toBeVisible();
  await expect(nav(page)).toBeHidden();
  await expect(page.getByRole('button', { name: /^Sekcja: Wygląd/ })).toBeVisible();
});

test('karty graficzne: każda z rolą — główna i do lekkich zadań', async ({ page }) => {
  await open(page);
  await page.keyboard.press('Control+,');
  await node(page, 'Urządzenia').click();
  const cards = page.locator('ul.gpus > li');
  await expect(cards).toHaveCount(2);
  await expect(cards.nth(0)).toContainText('NVIDIA GeForce RTX 4050 Laptop GPU');
  await expect(cards.nth(0).locator('.gpu-role')).toHaveText('karta główna');
  await expect(cards.nth(1)).toContainText('Intel Iris Xe Graphics');
  await expect(cards.nth(1).locator('.gpu-role')).toHaveText('lekkie zadania');
  await expect(cards.nth(1)).toContainText('pamięć współdzielona');
  await expect(cards.nth(1)).toContainText('llama.cpp Vulkan');
});

// Zgłoszenie właściciela (2026-10-09): obsada, mikrofon i stan w belce „też popraw solidnie”.
for (const skin of SKINS) {
  for (const theme of THEMES) {
    test(`belka: obsada, mikrofon i stan z objaśnieniem — ${skin}/${theme}`, async ({ page }) => {
      await open(page, `?style=${skin}&theme=${theme}`);
      const cast = page.getByRole('group', { name: 'Obsada agentek' });
      await expect(cast.getByRole('button')).toHaveCount(4);
      // Każda agentka podaje imię, rolę i stan (podpowiedź rysuje Alfa z tego samego tekstu).
      for (const name of ['Alfa', 'Beta', 'Gama', 'Delta']) {
        await expect(cast.getByRole('button', { name: new RegExp(`^${name} · `) })).toBeVisible();
      }

      // Stan: połączenie + profil + autonomia; klik otwiera objaśnienie z przejściem do ustawień.
      const state = page.getByRole('button', {
        name: /^Stan: połączono, profil Hybryda, autonomia L\d/,
      });
      await state.click();
      const details = page.getByRole('dialog', { name: 'Stan Alfy' });
      await expect(details).toContainText('Proste rzeczy liczy ten komputer');
      await expect(details).toContainText(/L\d — /);
      await expectReadable(page, 'header.titlebar');
      await details.getByRole('button', { name: 'Uprawnienia' }).click();
      await expect(details).toHaveCount(0);
      await expect(page.getByRole('heading', { level: 2, name: /Uprawnienia/ })).toBeVisible();
      await page.keyboard.press('Escape');

      // Koszt ma własną część i własne okienko.
      await page.getByRole('button', { name: /^Koszt sesji / }).click();
      await expect(page.getByRole('dialog', { name: 'Koszty i kontekst' })).toBeVisible();
      await expectReadable(page, 'header.titlebar');
      await page.keyboard.press('Escape');

      // Mikrofon: włączony ma wypełniony stan i słupki poziomu.
      const mic = page.getByRole('button', { name: /^Mikrofon: / });
      await expect(mic).toHaveAttribute('aria-pressed', 'false');
      await mic.click();
      await expect(mic).toHaveAttribute('aria-pressed', 'true');
      await expect(mic.locator('.bars span')).toHaveCount(3);
      await expectReadable(page, 'header.titlebar');
    });
  }
}

test('belka bez sieci: znak braku połączenia i kolejka w objaśnieniu', async ({ page }) => {
  await open(page, '?scenario=offline');
  const state = page.getByRole('button', { name: /^Stan: brak sieci/ });
  await expect(state).toBeVisible();
  await state.click();
  await expect(page.getByRole('dialog', { name: 'Stan Alfy' })).toContainText(
    'Działa tylko model lokalny',
  );
});
