import { expect, test } from '@playwright/test';
import { openApp, waitForChat } from './helpers';

test('F5 i Ctrl+R nie przeładowują okna', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await page.evaluate(() => ((window as unknown as { marker: number }).marker = 42));
  await page.keyboard.press('F5');
  await page.keyboard.press('Control+r');
  await page.waitForTimeout(300);
  expect(await page.evaluate(() => (window as unknown as { marker?: number }).marker)).toBe(42);
});

test('paleta: otwarcie ≤ 50 ms do pierwszej klatki (mediana z 7 prób po rozgrzaniu) i wybór z klawiatury', async ({
  page,
}) => {
  await openApp(page);
  await waitForChat(page);
  await page.waitForTimeout(800);
  const open = () =>
    page.evaluate(async () => {
      const t0 = performance.now();
      window.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'k', code: 'KeyK', ctrlKey: true, bubbles: true }),
      );
      for (;;) {
        if (document.querySelector('dialog[open] input')) break;
        await new Promise((r) => setTimeout(r, 0));
      }
      // Czas do pierwszej klatki z otwartą paletą.
      await new Promise((r) => requestAnimationFrame(r));
      return performance.now() - t0;
    });
  await open();
  await page.keyboard.press('Escape');
  const samples: number[] = [];
  for (let i = 0; i < 7; i++) {
    await page.waitForTimeout(150);
    samples.push(await open());
    await page.keyboard.press('Escape');
  }
  await expect(page.locator('dialog[open]')).toHaveCount(0);
  samples.sort((a, b) => a - b);
  // Mediana jako bramka (odporna na szum współdzielonego runnera); maksimum — z zapasem na szum.
  const label = `próby: ${samples.map((s) => s.toFixed(1)).join(', ')} ms`;
  expect(samples[3], label).toBeLessThan(50);
  expect(samples[6], label).toBeLessThan(150);
  await page.keyboard.press('Control+k');
  await expect(page.locator('dialog[open] input')).toBeFocused();
  await page.keyboard.type('ustawienia');
  // Lista przefiltrowana (bits-ui sortuje po zmianie zapytania) — wybrana pozycja to „Ustawienia".
  await expect(page.locator('dialog [role=option][data-selected]')).toContainText('Ustawienia');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Ustawienia', level: 1 })).toBeVisible();
});

test('composer: Enter wysyła, strumień widoczny, Esc zatrzymuje', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await page.locator('#alfa-composer').fill('Napisz szczegółowy esej');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('feed')).toHaveAttribute('aria-busy', 'true');
  await page.waitForTimeout(1800);
  await page.keyboard.press('Escape');
  await expect(page.getByText('Przerwano.')).toBeVisible();
});

test('@agentka: podpowiedź z listy i adresowanie', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  const composer = page.locator('#alfa-composer');
  await composer.click();
  await page.keyboard.type('@de');
  await expect(page.getByRole('option', { name: /@Delta/ })).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(composer).toHaveValue('@Delta ');
});

test('↑ w pustym composerze edytuje ostatnią wiadomość (nowa gałąź)', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await page.locator('#alfa-composer').click();
  await page.keyboard.press('ArrowUp');
  const editor = page.getByLabel('Edycja wiadomości — wysłanie utworzy nową gałąź');
  await expect(editor).toBeFocused();
  await editor.fill('Delta, zrób wersję PDF');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('group', { name: 'Gałąź 2 z 2' })).toBeVisible();
});

test('strumień 100 tok/s: aktualizacje DOM najwyżej raz na klatkę', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await page.evaluate(() => {
    const w = window as unknown as {
      frames_: number;
      mut: number;
      lastFrame: number;
      multi: number;
    };
    w.frames_ = 0;
    w.mut = 0;
    w.multi = 0;
    let seenThisFrame = 0;
    const loop = () => {
      w.frames_++;
      seenThisFrame = 0;
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
    new MutationObserver(() => {
      w.mut++;
      seenThisFrame++;
      if (seenThisFrame > 1) w.multi++;
    }).observe(document.querySelector('[role=feed]') as Node, {
      subtree: true,
      childList: true,
      characterData: true,
    });
  });
  await page.locator('#alfa-composer').fill('Zaplanuj tydzień');
  await page.keyboard.press('Enter');
  await page.waitForTimeout(1500);
  const stats = await page.evaluate(() => {
    const w = window as unknown as { frames_: number; mut: number; multi: number };
    return { frames: w.frames_, mut: w.mut, multi: w.multi };
  });
  expect(stats.mut).toBeGreaterThan(5);
  expect(stats.multi).toBeLessThanOrEqual(2);
});

test('responsywność: < 720 px — panele jako arkusze, Ctrl+B otwiera arkusz Sesji', async ({
  page,
}) => {
  await page.setViewportSize({ width: 600, height: 800 });
  await openApp(page);
  await waitForChat(page);
  await expect(page.getByRole('navigation', { name: 'Sesje' })).toHaveCount(0);
  await page.keyboard.press('Control+b');
  await expect(page.getByRole('dialog', { name: 'Sesje' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'Sesje' })).toHaveCount(0);
});

test('język EN przełącza interfejs', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await page.keyboard.press('Control+k');
  await expect(page.locator('dialog[open] input')).toBeFocused();
  await page.keyboard.type('angielski');
  await expect(page.locator('dialog [role=option][data-selected]')).toContainText('angielski');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('navigation', { name: 'Sessions' })).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
});
