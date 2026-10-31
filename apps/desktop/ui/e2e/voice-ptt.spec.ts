import { expect, test, type Page } from '@playwright/test';
import { waitForChat } from './helpers';

// Spacja „przytrzymaj, aby mówić” (`voice.ptt`) w oknie: w trybie PTT przytrzymanie Spacji poza
// polem pisania zaczyna rozmowę głosową (atrapa mówi odpowiedź — wyróżnione zdanie), puszczenie ją
// kończy. W trybie „Przełącznik” Spacja nic nie włącza.

/** Tekst wyróżnienia `alfa-reading` (pusty — agentka nie mówi). */
const marked = (page: Page) =>
  page.evaluate(() => {
    const h = CSS.highlights.get('alfa-reading');
    return h ? [...h].map((r) => r.toString()).join(' ') : '';
  });

async function outsideField(page: Page, url: string) {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto(url);
  await waitForChat(page);
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
}

test('voice.ptt: przytrzymanie Spacji mówi, puszczenie kończy', async ({ page }) => {
  await outsideField(page, '/?mic=ptt');
  await page.keyboard.down('Space');
  await expect.poll(() => marked(page), { timeout: 10_000, intervals: [50] }).not.toBe('');
  await page.keyboard.up('Space');
  await expect.poll(() => marked(page)).toBe('');
  // Spacja w polu pisania zostaje zwykłą spacją.
  const field = page.locator('#alfa-composer');
  await field.fill('a');
  await field.press('Space');
  await expect(field).toHaveValue('a ');
  await page.waitForTimeout(500);
  expect(await marked(page)).toBe('');
});

test('voice.ptt: w trybie „Przełącznik” Spacja nie włącza mikrofonu', async ({ page }) => {
  await outsideField(page, '/');
  await page.keyboard.down('Space');
  await page.waitForTimeout(1500);
  expect(await marked(page)).toBe('');
  await page.keyboard.up('Space');
});
