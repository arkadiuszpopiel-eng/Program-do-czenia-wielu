import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, waitForChat } from './helpers';

// Wyróżnianie czytanego zdania w rozmowie głosowej (plan 43, faza 2; etap K.3). Rdzeń zgłasza zdania
// odpowiedzi z nieznaną z góry liczbą (`of` = u32::MAX), koniec — `index == of`. Atrapa w fazie
// „agentka mówi” mówi ostatnią odpowiedź agentki tak jak rozmowa głosowa.

/** Tekst wyróżnienia `alfa-reading` (pusty — brak wyróżnienia). */
const marked = (page: Page) =>
  page.evaluate(() => {
    const h = CSS.highlights.get('alfa-reading');
    return h ? [...h].map((r) => r.toString()).join(' ') : '';
  });

async function talk(page: Page, style: string, theme: 'light' | 'dark') {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?style=${style}&theme=${theme}`);
  await waitForChat(page);
  await page.keyboard.press('Control+Shift+M');
}

test('rozmowa głosowa: zdanie odpowiedzi wyróżnione, koniec mowy je zdejmuje', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await talk(page, 'junior', 'light');
  await expect
    .poll(() => marked(page), { timeout: 10_000, intervals: [50] })
    .toBe('Utworzyłam szkic raport-Q3.docx');
  await expect
    .poll(() => marked(page), { timeout: 15_000, intervals: [50] })
    .toBe(
      'Chcę jeszcze nadpisać szablon zarząd.dotx, żeby stopka miała właściwy kwartał — to wymaga Twojej zgody',
    );
  await expectAccessible(page, 'rozmowa głosowa junior/light');
  // Liczba zdań nieznana z góry: koniec mowy (`index == of`) zdejmuje wyróżnienie.
  await expect.poll(() => marked(page), { timeout: 30_000 }).toBe('');
  expect(errors).toEqual([]);
});

test('rozmowa głosowa: wyłączenie mikrofonu ucina mowę i wyróżnienie', async ({ page }) => {
  await talk(page, 'classic', 'dark');
  await expect.poll(() => marked(page), { timeout: 10_000, intervals: [50] }).not.toBe('');
  await page.keyboard.press('Control+Shift+M');
  await expect.poll(() => marked(page)).toBe('');
  // Po wyłączeniu nie przychodzą już zdania przerwanej mowy.
  await page.waitForTimeout(800);
  expect(await marked(page)).toBe('');
});
