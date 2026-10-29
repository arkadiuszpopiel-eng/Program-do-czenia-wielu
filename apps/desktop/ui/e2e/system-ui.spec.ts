import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { waitForChat } from './helpers';

// Zgłoszenie właściciela (2026-10-09): prawy przycisk otwierał menu systemowe WebView2 („Emoji”,
// „Kierunek pisania”, „Wykonaj inspekcję”). Menu kontekstowe i podpowiedzi rysuje Alfa; systemowe
// dymki `title`, strzałki pól liczbowych i znaczniki rozwijania nie pojawiają się nigdzie.

const menu = (page: Page) => page.getByRole('menu', { name: 'Menu edycji' });
const item = (page: Page, name: string) =>
  menu(page).getByRole('menuitem', { name: new RegExp(`^${name}`) });

async function open(page: Page, query = '') {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto(`/${query}`);
  await waitForChat(page);
}

/** Czy zdarzenie `contextmenu` zostało zablokowane (menu systemowe się nie pokaże). */
async function nativeMenuBlocked(page: Page, selector: string): Promise<boolean> {
  return page
    .locator(selector)
    .first()
    .evaluate((el) => {
      const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
      el.dispatchEvent(event);
      return event.defaultPrevented;
    });
}

test('pole pisania: własne menu z wycinaniem, kopiowaniem i wklejaniem', async ({
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page);
  const field = page.locator('#alfa-composer');
  await field.fill('Ala ma kota');
  await field.click({ button: 'right' });
  await expect(menu(page)).toBeVisible();
  await expect(menu(page).getByRole('menuitem')).toHaveCount(5);
  for (const [name, keys] of [
    ['Cofnij', 'Ctrl+Z'],
    ['Wytnij', 'Ctrl+X'],
    ['Kopiuj', 'Ctrl+C'],
    ['Wklej', 'Ctrl+V'],
    ['Zaznacz wszystko', 'Ctrl+A'],
  ] as const)
    await expect(item(page, name)).toContainText(keys);
  // Bez zaznaczenia nie ma czego wycinać ani kopiować.
  await expect(item(page, 'Wytnij')).toBeDisabled();
  await expect(item(page, 'Kopiuj')).toBeDisabled();
  await expectReadable(page, 'menu kontekstowe pola');

  await item(page, 'Zaznacz wszystko').click();
  await expect(menu(page)).toHaveCount(0);
  await field.click({ button: 'right' });
  await item(page, 'Wytnij').click();
  await expect(field).toHaveValue('');
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('Ala ma kota');

  await field.click({ button: 'right' });
  await item(page, 'Wklej').click();
  await expect(field).toHaveValue('Ala ma kota');
  // Wklejenie da się cofnąć z menu (historia edycji pola zostaje).
  await field.click({ button: 'right' });
  await item(page, 'Cofnij').click();
  await expect(field).toHaveValue('');
});

test('menu obsługuje się klawiaturą i zamyka Escape albo kliknięciem obok', async ({ page }) => {
  await open(page);
  const field = page.locator('#alfa-composer');
  await field.fill('tekst');
  await field.click({ button: 'right' });
  await page.keyboard.press('ArrowDown');
  await expect(menu(page).locator('button.active')).toContainText('Cofnij');
  await page.keyboard.press('End');
  await expect(menu(page).locator('button.active')).toContainText('Zaznacz wszystko');
  await page.keyboard.press('Enter');
  await expect(menu(page)).toHaveCount(0);
  const selected = await field.evaluate(
    (el: HTMLTextAreaElement) => el.selectionEnd - el.selectionStart,
  );
  expect(selected).toBe(5);

  await field.click({ button: 'right' });
  await page.keyboard.press('Escape');
  await expect(menu(page)).toHaveCount(0);
  // Escape zamknął tylko menu — rozmowa i pole zostają.
  await expect(field).toBeVisible();
  await field.click({ button: 'right' });
  await page.mouse.click(700, 80);
  await expect(menu(page)).toHaveCount(0);
});

test('zaznaczony tekst odpowiedzi: tylko „Kopiuj”; puste miejsce: żadnego menu', async ({
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page);
  // Ostatni akapit — lista wiadomości jest wirtualna, pierwszy (poza oknem) bywa odmontowany.
  const prose = page.locator('.alfa-prose p').last();
  await prose.selectText();
  await prose.click({ button: 'right' });
  await expect(menu(page).getByRole('menuitem')).toHaveCount(1);
  const selected = await page.evaluate(() => document.getSelection()?.toString() ?? '');
  await item(page, 'Kopiuj').click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(selected);

  await page.evaluate(() => document.getSelection()?.removeAllRanges());
  await page.locator('header').first().click({ button: 'right' });
  await expect(menu(page)).toHaveCount(0);
  // Menu systemowe jest zablokowane wszędzie: w polu, w treści i na pustym tle.
  for (const selector of ['#alfa-composer', '.alfa-prose', 'header', 'body'])
    expect(await nativeMenuBlocked(page, selector), selector).toBe(true);
});

test('pole hasła: kopiowanie i wycinanie wyłączone', async ({ page }) => {
  await open(page);
  await page.keyboard.press('Control+,');
  await page.getByRole('button', { name: 'Konta i klucze API', exact: true }).click();
  await page.getByRole('button', { name: 'Dodaj dostawcę' }).click();
  await page
    .getByRole('button', { name: /^OpenAI/ })
    .first()
    .click();
  const key = page.getByLabel('Klucz API');
  await key.fill('test-klucz-XXXXXXXXXXXXXXXX');
  await key.selectText();
  await key.click({ button: 'right' });
  await expect(item(page, 'Kopiuj')).toBeDisabled();
  await expect(item(page, 'Wytnij')).toBeDisabled();
  await expect(item(page, 'Wklej')).toBeEnabled();
  // Pola nie podpowiadają wcześniejszych wpisów listą systemu.
  await expect(key).toHaveAttribute('autocomplete', 'off');
});

test('podpowiedzi rysuje Alfa: po najechaniu i po fokusie, bez atrybutu title', async ({
  page,
}) => {
  await open(page);
  const button = page.getByRole('button', { name: 'Ustawienia' }).first();
  await button.hover();
  const tip = page.getByRole('tooltip');
  await expect(tip).toBeVisible();
  await expect(tip).toContainText('Ustawienia');
  await expect(button).not.toHaveAttribute('title');
  await expectReadable(page, 'podpowiedź');
  await page.mouse.move(400, 400);
  await expect(tip).toHaveCount(0);
  // Fokus z klawiatury pokazuje podpowiedź od razu; kolejny klawisz ją chowa.
  await button.focus();
  await page.keyboard.press('Shift+Tab');
  await page.keyboard.press('Tab');
  await expect(tip).toBeVisible();
  await page.keyboard.press('Shift');
  await expect(tip).toHaveCount(0);
});

test('pola liczbowe i sekcje rozwijane bez elementów rysowanych przez system', async ({ page }) => {
  await open(page);
  await page.keyboard.press('Control+,');
  await page.getByRole('button', { name: 'Mosty CLI', exact: true }).click();
  const marker = await page
    .locator('summary')
    .first()
    .evaluate((el) => getComputedStyle(el).listStyleType);
  expect(marker).toBe('none');
  // Nigdzie w dokumencie nie zostaje systemowa lista wyboru ani atrybut `title` po najechaniu.
  expect(await page.locator('select').count()).toBe(0);
  const spin = await page.evaluate(() => {
    const input = document.createElement('input');
    input.type = 'number';
    document.body.append(input);
    const look = getComputedStyle(input).appearance;
    input.remove();
    return look;
  });
  expect(spin).toBe('textfield');
});

for (const skin of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`menu kontekstowe i podpowiedź czytelne: ${skin} / ${theme}`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
      await page.goto(`/?style=${skin}&theme=${theme}`);
      await waitForChat(page);
      const field = page.locator('#alfa-composer');
      await field.fill('Ala ma kota');
      await field.selectText();
      await field.click({ button: 'right' });
      await expect(menu(page)).toBeVisible();
      await expectReadable(page, `${skin}/${theme} — menu kontekstowe`);
      const look = await menu(page).evaluate((el) => {
        const box = el.getBoundingClientRect();
        return {
          background: getComputedStyle(el).backgroundColor,
          right: box.right,
          bottom: box.bottom,
        };
      });
      expect(look.background).not.toContain(', 0)');
      const viewport = page.viewportSize() ?? { width: 0, height: 0 };
      expect(look.right).toBeLessThanOrEqual(viewport.width);
      expect(look.bottom).toBeLessThanOrEqual(viewport.height);
      await page.keyboard.press('Escape');
      await page.getByRole('button', { name: 'Ustawienia' }).first().hover();
      await expect(page.getByRole('tooltip')).toBeVisible();
      await expectReadable(page, `${skin}/${theme} — podpowiedź`);
    });
  }
}
