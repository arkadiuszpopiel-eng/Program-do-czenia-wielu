import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, waitForChat } from './helpers';

// Wyróżnianie czytanego zdania (plan 43, decyzja właściciela 29.10): „Przeczytaj na głos” przy
// wiadomości agentki wyróżnia zdanie, które właśnie słychać (CSS Custom Highlight API), przesuwa się
// zdanie po zdaniu, a Esc albo koniec czytania je zdejmuje. Atrapa wysyła `SpeechSegment` jak rdzeń.

const SKINS = [
  ['classic', 'light'],
  ['classic', 'dark'],
  ['glass', 'light'],
  ['glass', 'dark'],
  ['next', 'light'],
  ['next', 'dark'],
] as const;

/** Tekst wyróżnienia `alfa-reading` (pusty — brak wyróżnienia). */
const marked = (page: Page) =>
  page.evaluate(() => {
    const h = CSS.highlights.get('alfa-reading');
    return h ? [...h].map((r) => r.toString()).join(' ') : '';
  });

async function startReading(page: Page, style: string, theme: 'light' | 'dark') {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?style=${style}&theme=${theme}`);
  await waitForChat(page);
  const message = page.locator('article.msg:not(.user)').last();
  await expect(message).toBeVisible();
  await message.hover();
  await message.getByRole('button', { name: 'Przeczytaj na głos' }).click();
  return message;
}

test('czytane zdanie: wyróżnione zdanie po zdaniu, zdjęte na końcu czytania', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await startReading(page, 'classic', 'light');
  // Całe zdania z ekranu (kropka w „raport-Q3.docx” nie kończy zdania), jedno wyróżnienie naraz.
  await expect.poll(() => marked(page)).toBe('Utworzyłam szkic raport-Q3.docx');
  await expect
    .poll(() => marked(page), { timeout: 15_000 })
    .toBe(
      'Chcę jeszcze nadpisać szablon zarząd.dotx, żeby stopka miała właściwy kwartał — to wymaga Twojej zgody',
    );
  // Koniec czytania: wyróżnienie znika samo.
  await expect.poll(() => marked(page), { timeout: 30_000 }).toBe('');
  expect(errors).toEqual([]);
});

test('czytane zdanie: Esc zatrzymuje czytanie i zdejmuje wyróżnienie', async ({ page }) => {
  await startReading(page, 'classic', 'light');
  await expect.poll(() => marked(page)).not.toBe('');
  await page.keyboard.press('Escape');
  await expect.poll(() => marked(page)).toBe('');
  // Po „Stop” nie przychodzą już zdania przerwanego czytania.
  await page.waitForTimeout(600);
  expect(await marked(page)).toBe('');
});

test('czytane zdanie w każdym stylu i motywie: wyróżnienie widoczne, axe bez naruszeń', async ({
  page,
}) => {
  for (const [style, theme] of SKINS) {
    const message = await startReading(page, style, theme);
    await expect.poll(() => marked(page)).not.toBe('');
    await expectAccessible(page, `czytane zdanie ${style}/${theme}`);
    await message.screenshot({ path: `test-results/czytane-zdanie-${style}-${theme}.png` });
    await page.keyboard.press('Escape');
    await expect.poll(() => marked(page)).toBe('');
  }
});
