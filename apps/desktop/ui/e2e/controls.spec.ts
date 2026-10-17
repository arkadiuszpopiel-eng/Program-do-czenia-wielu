import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { openApp, waitForChat } from './helpers';

// Regresje z przeglądu zmian 0.0.1–0.0.5: lista wyboru (klawiatura z AltGr, etykieta, utrata fokusu,
// skala interfejsu), fokus z klawiatury w rozmowie oraz sam miernik kontrastu.

async function openSettingsPage(page: Page, name: string) {
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name })
    .click();
  await expect(page.getByRole('heading', { name, level: 2 })).toBeVisible();
}

test('miernik kontrastu odrzuca nieczytelny tekst, także nad półprzezroczystym tłem', async ({
  page,
}) => {
  await openApp(page);
  await waitForChat(page);
  await expectReadable(page, 'stan wyjściowy');
  await page.evaluate(() => {
    const box = document.createElement('div');
    box.id = 'probe-box';
    box.style.cssText =
      'position:fixed;top:8px;left:8px;z-index:999;padding:4px;background:rgba(255,255,255,0.05)';
    const text = document.createElement('span');
    text.style.color = 'rgba(128,128,128,0.6)';
    text.textContent = 'słabo widoczny napis';
    box.append(text);
    document.body.append(box);
  });
  await expect(expectReadable(page, 'celowo nieczytelny')).rejects.toThrow(/słabo widoczny/);
});

test('rozmowa: fokus przeniesiony klawiaturą do wiadomości zostaje w widoku', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 620 });
  await openApp(page);
  await waitForChat(page);
  await page.locator('#alfa-composer').focus();
  let checked = 0;
  for (let i = 0; i < 40; i++) {
    await page.keyboard.press('Shift+Tab');
    // Dwie klatki: przewinięcie do fokusu i ewentualny (błędny) powrót listy na dół.
    await page.evaluate(
      () => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))),
    );
    const state = await page.evaluate(() => {
      const feed = document.querySelector('[role="feed"]');
      const active = document.activeElement;
      if (!feed || !active || !feed.contains(active) || active === feed) return null;
      const a = active.getBoundingClientRect();
      const f = feed.getBoundingClientRect();
      return { visible: a.bottom > f.top + 1 && a.top < f.bottom - 1, top: a.top - f.top };
    });
    if (!state) continue;
    checked += 1;
    expect(state.visible, `element z fokusem (krok ${i}) w widoku listy`).toBe(true);
  }
  expect(checked, 'fokus przeszedł przez elementy rozmowy').toBeGreaterThan(3);
});

test('lista wyboru: litera z AltGr przeskakuje do opcji', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await openSettingsPage(page, 'Wygląd');
  const theme = page.getByRole('combobox', { name: 'Motyw', exact: true });
  await theme.focus();
  // Polska klawiatura: AltGr zgłasza Ctrl+Alt — taka litera była odrzucana jako skrót.
  await theme.evaluate((button) =>
    button.dispatchEvent(
      new KeyboardEvent('keydown', {
        key: 'c',
        ctrlKey: true,
        altKey: true,
        modifierAltGraph: true,
        bubbles: true,
        cancelable: true,
      }),
    ),
  );
  const list = page.getByRole('listbox', { name: 'Motyw' });
  await expect(list).toBeVisible();
  const activeId = await theme.getAttribute('aria-activedescendant');
  await expect(page.locator(`[id="${activeId}"]`)).toHaveText('Ciemny');
  // Sam Ctrl+litera to nadal skrót, nie wyszukiwanie.
  await page.keyboard.press('Escape');
  await theme.evaluate((button) =>
    button.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'j', ctrlKey: true, bubbles: true, cancelable: true }),
    ),
  );
  await expect(list).toHaveCount(0);
});

test('lista wyboru: klik etykiety i utrata fokusu zamykają ją na stałe', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await openSettingsPage(page, 'Modele lokalne i silniki');
  const kind = page.getByRole('combobox', { name: 'Rodzaj', exact: true });
  await kind.scrollIntoViewIfNeeded();
  const list = page.getByRole('listbox', { name: 'Rodzaj' });
  await kind.click();
  await expect(list).toBeVisible();
  // Tekst etykiety obejmującej listę: klik zamyka i nie otwiera jej zaraz ponownie.
  await page.getByText('Rodzaj', { exact: true }).first().click();
  await page.waitForTimeout(400);
  await expect(list).toHaveCount(0);
  // Fokus zabrany programowo (np. otwarte okno): lista nie zostaje otwarta bez obsługi klawiszy.
  await kind.click();
  await expect(list).toBeVisible();
  await page.getByRole('searchbox').first().focus();
  await expect(list).toHaveCount(0);
});

test('lista wyboru: pierwszy klik zaraz po wczytaniu strony otwiera listę', async ({ page }) => {
  // Zegar strony cofnięty do zera tuż przed kliknięciem — jak na szybkim komputerze, gdzie klik pada
  // w pierwszych 300 ms po wczytaniu (2026-10-18: lista okresu rozmów nie otwierała się wcale).
  await page.addInitScript(() => {
    const real = performance.now.bind(performance);
    let base = 0;
    performance.now = () => real() - base;
    (window as unknown as { clockZero: () => void }).clockZero = () => {
      base = real();
    };
  });
  await openApp(page);
  await waitForChat(page);
  const period = page
    .getByRole('navigation', { name: 'Sesje' })
    .getByRole('combobox', { name: 'Pokaż rozmowy' });
  await page.evaluate(() => (window as unknown as { clockZero: () => void }).clockZero());
  await period.click();
  await expect(page.getByRole('listbox', { name: 'Pokaż rozmowy' })).toBeVisible();
});

test('lista wyboru przy skali 150 %: w dolnej części okna otwiera się w górę', async ({ page }) => {
  await page.setViewportSize({ width: 1500, height: 760 });
  await openApp(page);
  await waitForChat(page);
  for (let i = 0; i < 5; i++) await page.keyboard.press('Control+=');
  await openSettingsPage(page, 'Wygląd');
  const zoom = await page.evaluate(() =>
    Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--alfa-zoom')),
  );
  expect(zoom).toBeGreaterThanOrEqual(1.4);
  // Lista wyboru, której dolna krawędź leży między 65 % a 95 % wysokości okna (w jednostkach strony).
  const boxes = page.getByRole('combobox');
  const count = await boxes.count();
  let target = -1;
  for (let i = 0; i < count; i++) {
    const place = await boxes.nth(i).evaluate((el, z) => {
      const r = el.getBoundingClientRect();
      return r.bottom / (window.innerHeight / z);
    }, zoom);
    if (place > 0.65 && place < 0.95) target = i;
  }
  expect(target, 'lista wyboru w dolnej części okna').toBeGreaterThanOrEqual(0);
  const select = boxes.nth(target);
  await select.click();
  const listId = await select.getAttribute('aria-controls');
  const list = page.locator(`[id="${listId}"]`);
  await expect(list).toBeVisible();
  const [button, popup] = await Promise.all([select.boundingBox(), list.boundingBox()]);
  expect(popup && button && popup.y + popup.height <= button.y + 1, 'lista nad przyciskiem').toBe(
    true,
  );
});
