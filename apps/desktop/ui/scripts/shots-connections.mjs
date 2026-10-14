#!/usr/bin/env node
// Zrzuty strony Ustawienia → Stan połączeń (i karty Mostów CLI) w 3 stylach × 2 motywach oraz
// w wąskim oknie — do przeglądu „na oko”.
//
// Użycie: node scripts/shots-connections.mjs <katalog> [--url http://localhost:4173]
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { chromium } from '@playwright/test';

const args = process.argv.slice(2);
const outDir = resolve(args[0] ?? 'shots-connections');
const urlAt = args.indexOf('--url');
const base = urlAt >= 0 ? args[urlAt + 1] : 'http://localhost:4173';
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch({ channel: process.env.PW_CHANNEL || undefined });
const open = async (skin, theme, width) => {
  const page = await browser.newPage({ viewport: { width, height: 900 }, locale: 'pl-PL' });
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`${base}/?style=${skin}&theme=${theme}`);
  await page.locator('#alfa-composer').waitFor();
  await page.getByRole('button', { name: 'Ustawienia (Ctrl+,)' }).click();
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  await nav.locator('button.node', { hasText: 'Stan połączeń' }).click();
  await page.getByRole('button', { name: 'Sprawdź wszystko' }).waitFor();
  await page.waitForTimeout(600);
  return page;
};
for (const skin of ['classic', 'glass', 'next']) {
  for (const theme of ['light', 'dark']) {
    const page = await open(skin, theme, 1280);
    await page.screenshot({ path: join(outDir, `${skin}-${theme}.png`) });
    // Dół strony (głos, ochrona i system): przewinięcie obszaru strony do końca.
    await page.getByRole('heading', { name: 'Ochrona i system' }).scrollIntoViewIfNeeded();
    await page.screenshot({ path: join(outDir, `${skin}-${theme}-dol.png`) });
    await page.close();
  }
}
const narrow = await open('classic', 'light', 900);
await narrow.screenshot({ path: join(outDir, 'classic-light-waskie.png'), fullPage: true });
await narrow.close();
await browser.close();
console.log(`zrzuty: ${outDir}`);
