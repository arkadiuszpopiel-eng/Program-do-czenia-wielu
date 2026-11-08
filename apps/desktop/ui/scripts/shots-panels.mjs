#!/usr/bin/env node
// Zrzuty widoku głównego, paneli bocznych (Alt+1…Alt+7) i nakładek ze skrótów — do przeglądu „na oko” w skali 1:1.
//
// Użycie: node scripts/shots-panels.mjs <katalog> [--url http://localhost:4173] [--only classic-light,glass-dark]
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { chromium } from '@playwright/test';

const args = process.argv.slice(2);
const outDir = resolve(args[0] ?? 'shots-panels');
const urlAt = args.indexOf('--url');
const base = urlAt >= 0 ? args[urlAt + 1] : 'http://localhost:4173';
const onlyAt = args.indexOf('--only');
const only = onlyAt >= 0 ? args[onlyAt + 1].split(',') : null;
mkdirSync(outDir, { recursive: true });

const OVERLAYS = [
  ['paleta', 'Control+k'],
  ['skroty', 'Control+/'],
  ['sesje', 'Control+p'],
];

const browser = await chromium.launch({ channel: process.env.PW_CHANNEL || undefined });
for (const skin of ['classic', 'glass', 'next']) {
  for (const theme of ['light', 'dark']) {
    if (only && !only.includes(`${skin}-${theme}`)) continue;
    const page = await browser.newPage({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 1,
      locale: 'pl-PL',
    });
    await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
    await page.goto(`${base}/?style=${skin}&theme=${theme}`);
    await page.locator('#alfa-composer').waitFor();
    await page.waitForTimeout(400);
    const shot = (name) => page.screenshot({ path: join(outDir, `${skin}-${theme}-${name}.png`) });
    await shot('00-glowny');
    for (let i = 1; i <= 7; i += 1) {
      await page.keyboard.press(`Alt+${i}`);
      await page.waitForTimeout(350);
      await shot(`panel-${i}`);
    }
    for (const [name, keys] of OVERLAYS) {
      await page.keyboard.press(keys);
      await page.waitForTimeout(350);
      await shot(`nakladka-${name}`);
      await page.keyboard.press('Escape');
      await page.waitForTimeout(150);
    }
    await page.close();
  }
}
await browser.close();
console.log(`zrzuty: ${outDir}`);
