import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, waitForChat } from './helpers';

// Style interfejsu („Szkło", „Next"): wybór w Ustawienia → Wygląd ustawia `data-skin` na <html>,
// każdy styl ma wariant jasny i ciemny, a kontrast i role przechodzą axe w rozmowie i w ustawieniach.

async function openAppearance(page: Page) {
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Wygląd' })
    .click();
  await expect(page.getByRole('heading', { name: 'Wygląd', level: 2 })).toBeVisible();
}

test('wybór stylu w ustawieniach przełącza interfejs i wraca do klasycznego', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto('/');
  await waitForChat(page);
  const html = page.locator('html');
  await expect(html).not.toHaveAttribute('data-skin', /.+/);
  await openAppearance(page);
  const style = page.getByLabel('Styl', { exact: true });
  await style.selectOption('glass');
  await expect(html).toHaveAttribute('data-skin', 'glass');
  await style.selectOption('next');
  await expect(html).toHaveAttribute('data-skin', 'next');
  await style.selectOption('classic');
  await expect(html).not.toHaveAttribute('data-skin', /.+/);
});

for (const skin of ['glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`styl ${skin}, motyw ${theme}: rozmowa i ustawienia dostępne`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
      await page.goto(`/?style=${skin}&theme=${theme}`);
      await waitForChat(page);
      const html = page.locator('html');
      await expect(html).toHaveAttribute('data-skin', skin);
      await expect(html).toHaveAttribute('data-theme', theme);
      await expectAccessible(page, `rozmowa ${skin} ${theme}`);
      await openAppearance(page);
      await expectAccessible(page, `ustawienia ${skin} ${theme}`);
    });
  }
}

test('Szybkie pytanie w stylu Szkło ma kryjące tło panelu', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.goto('/quick.html?style=glass&theme=dark');
  await expect(page.locator('html')).toHaveAttribute('data-skin', 'glass');
  // Okno jest przezroczyste: tło strony zostaje puste, a tafla panelu nie może prześwitywać.
  const bodyBg = await page.evaluate(() => getComputedStyle(document.body).backgroundImage);
  expect(bodyBg).toBe('none');
  await expectAccessible(page, 'quick glass dark');
});
