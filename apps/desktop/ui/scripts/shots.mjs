#!/usr/bin/env node
// Zrzuty ekranu wszystkich widoków (rozmowa, panele Alt+1…7, każda strona ustawień) w kilku rozmiarach
// okna — do przeglądu układu i skalowania. Wymaga działającego `pnpm dev` (atrapa backendu).
// Użycie: node scripts/shots.mjs <katalog-wyjściowy> [--sizes 1280x720,1920x1080] [--only chat,panels,settings,onboarding,windows,overlays]
//         [--scenario default] [--theme dark] [--style glass|next] [--url http://localhost:5173]
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { chromium } from '@playwright/test';

const args = process.argv.slice(2);
const flag = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : fallback;
};
const outDir = args[0] && !args[0].startsWith('--') ? args[0] : 'shots';
const sizes = flag('sizes', '1280x720,1920x1080,2560x1440')
  .split(',')
  .map((s) => s.split('x').map(Number));
const only = new Set(flag('only', 'chat,panels,settings').split(','));
const scenario = flag('scenario', '');
const theme = flag('theme', 'dark');
const base = flag('url', 'http://localhost:5173');
// Styl interfejsu (`classic`, `glass`, `next`) — przekazywany atrapie razem z motywem.
const style = flag('style', '');
const query = new URLSearchParams({
  ...(scenario ? { scenario } : {}),
  ...(style ? { style, theme } : {}),
}).toString();
// Parametry stylu bez scenariusza — dla widoków, które mają własny scenariusz albo własną stronę.
const skinQuery = style ? `style=${style}&theme=${theme}` : '';
const slug = (s) =>
  s
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/ł/g, 'l')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');

mkdirSync(outDir, { recursive: true });
// Bez pobranej przeglądarki Playwrighta używamy systemowego Edge — ten sam silnik co WebView2 Alfy.
const browser = await chromium.launch().catch(() => chromium.launch({ channel: 'msedge' }));
for (const [width, height] of sizes) {
  const page = await browser.newPage({ viewport: { width, height }, locale: 'pl-PL' });
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`${base}/${query ? `?${query}` : ''}`);
  await page.locator('#alfa-composer').waitFor();
  await page.waitForTimeout(400);
  const shot = (name) => page.screenshot({ path: join(outDir, `${width}-${name}.png`) });

  if (only.has('chat')) await shot('00-rozmowa');
  if (only.has('panels')) {
    for (let i = 1; i <= 7; i++) {
      await page.keyboard.press(`Alt+${i}`);
      await page.waitForTimeout(500);
      await shot(`0${i}-panel`);
    }
  }
  if (only.has('settings')) {
    await page.keyboard.press('Control+,');
    const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
    await nav.getByRole('button').first().waitFor();
    const names = await nav.getByRole('button').allInnerTexts();
    for (const [i, raw] of names.entries()) {
      await nav.getByRole('button').nth(i).click();
      await page.waitForTimeout(450);
      await shot(`s${String(i + 1).padStart(2, '0')}-${slug(raw.split('\n')[0].trim())}`);
    }
  }
  // Nakładki: paleta poleceń (Ctrl+K) i ściągawka skrótów (Ctrl+/) nad rozmową.
  if (only.has('overlays')) {
    await page.goto(`${base}/${query ? `?${query}` : ''}`);
    await page.locator('#alfa-composer').waitFor();
    await page.locator('#alfa-composer').focus();
    for (const [key, name] of [
      ['Control+k', 'paleta'],
      ['Control+/', 'sciagawka'],
    ]) {
      await page.keyboard.press(key);
      await page.locator('dialog[open], [role=dialog]').first().waitFor();
      await page.waitForTimeout(400);
      await shot(`n-${name}`);
      await page.keyboard.press('Escape');
      await page.waitForTimeout(200);
    }
  }
  await page.close();

  // Wprowadzenie (scenariusz `first-run`): każdy krok po kolei przyciskiem „Dalej”.
  if (only.has('onboarding')) {
    const ob = await browser.newPage({ viewport: { width, height }, locale: 'pl-PL' });
    await ob.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
    await ob.goto(`${base}/?scenario=first-run${skinQuery ? `&${skinQuery}` : ''}`);
    await ob.locator('#ob-title').waitFor();
    for (let i = 1; i <= 12; i++) {
      await ob.waitForTimeout(500);
      await ob.screenshot({ path: join(outDir, `${width}-ob${String(i).padStart(2, '0')}.png`) });
      const next = ob.getByRole('button', { name: 'Dalej', exact: true });
      if (!(await next.count()) || !(await next.isEnabled())) break;
      await next.click();
    }
    await ob.close();
  }
  // Okna poboczne: Szybkie pytanie i pigułka głosowa (małe okna o stałym rozmiarze).
  if (only.has('windows')) {
    for (const [name, w, h] of [
      ['quick', 640, 420],
      ['pill', 360, 96],
    ]) {
      const win = await browser.newPage({ viewport: { width: w, height: h }, locale: 'pl-PL' });
      await win.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
      await win.goto(`${base}/${name}.html${skinQuery ? `?${skinQuery}` : ''}`);
      await win.waitForTimeout(700);
      await win.screenshot({ path: join(outDir, `okno-${name}.png`) });
      await win.close();
    }
  }
  console.log(`shots: ${width}x${height} gotowe`);
}
await browser.close();
