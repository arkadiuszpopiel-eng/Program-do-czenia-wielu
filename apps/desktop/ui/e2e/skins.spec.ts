import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, pickOption, waitForChat } from './helpers';

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
  const style = page.getByRole('combobox', { name: 'Styl', exact: true });
  await pickOption(style, 'glass');
  await expect(html).toHaveAttribute('data-skin', 'glass');
  await pickOption(style, 'next');
  await expect(html).toHaveAttribute('data-skin', 'next');
  await pickOption(style, 'classic');
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

// Błąd z wersji 0.0.3: rozwinięta lista wyboru była systemowa, a w stylu „Szkło" miała jasny tekst
// na białym tle. Lista jest teraz elementem strony — sprawdzamy ją otwartą w każdym stylu i motywie.
for (const skin of ['classic', 'glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`lista wyboru otwarta: ${skin}, ${theme} — czytelna, kryjąca, obsługa klawiaturą`, async ({
      page,
    }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
      await page.goto(`/?style=${skin}&theme=${theme}`);
      await waitForChat(page);
      await openAppearance(page);
      await expect(page.locator('select')).toHaveCount(0);
      const select = page.getByRole('combobox', { name: 'Motyw', exact: true });
      await select.click();
      const list = page.getByRole('listbox', { name: 'Motyw' });
      await expect(list.getByRole('option')).toHaveCount(3);
      const look = await list.evaluate((el) => {
        // Kolory mogą być w oklch() — rozwiązuje je płótno, nie parsowanie tekstu.
        const probe = document.createElement('canvas').getContext('2d');
        const rgba = (color: string) => {
          if (!probe) return [0, 0, 0, 255];
          probe.clearRect(0, 0, 1, 1);
          probe.fillStyle = color;
          probe.fillRect(0, 0, 1, 1);
          return [...probe.getImageData(0, 0, 1, 1).data];
        };
        const lum = ([r = 0, g = 0, b = 0]: number[]) => {
          const ch = (v: number) => {
            const c = v / 255;
            return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
          };
          return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b);
        };
        const bg = rgba(getComputedStyle(el).backgroundColor);
        const option = el.querySelector('[role="option"]:not(.active)') ?? el;
        const fg = rgba(getComputedStyle(option).color);
        const [hi, lo] = lum(bg) > lum(fg) ? [lum(bg), lum(fg)] : [lum(fg), lum(bg)];
        return { alpha: (bg[3] ?? 0) / 255, contrast: (hi + 0.05) / (lo + 0.05) };
      });
      expect(look.alpha).toBe(1);
      expect(look.contrast).toBeGreaterThanOrEqual(4.5);
      await expectAccessible(page, `lista wyboru ${skin} ${theme}`);
      // Klawiatura: strzałka i Enter wybierają, Esc zamyka bez zmiany.
      await page.keyboard.press('Escape');
      await expect(list).toHaveCount(0);
      await expect(page.getByRole('heading', { name: 'Wygląd', level: 2 })).toBeVisible();
      const density = page.getByRole('combobox', { name: 'Gęstość', exact: true });
      await density.focus();
      await page.keyboard.press('ArrowDown');
      await page.keyboard.press('ArrowDown');
      await page.keyboard.press('Enter');
      await expect(density).toHaveAttribute('data-value', 'compact');
      await expect(page.locator('html')).toHaveAttribute('data-density', 'compact');
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
