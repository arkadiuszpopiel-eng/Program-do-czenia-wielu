#!/usr/bin/env node
// Zrzuty wszystkich stron Ustawień w wybranych stylach i motywach — do przeglądu „na oko”.
//
// Użycie: node scripts/shots-settings-all.mjs <katalog> [--url http://localhost:4173] [--only classic-light,glass-dark]
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { chromium } from '@playwright/test';

const args = process.argv.slice(2);
const outDir = resolve(args[0] ?? 'shots-settings');
const urlAt = args.indexOf('--url');
const base = urlAt >= 0 ? args[urlAt + 1] : 'http://localhost:4173';
const onlyAt = args.indexOf('--only');
const only = onlyAt >= 0 ? args[onlyAt + 1].split(',') : null;
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch({ channel: process.env.PW_CHANNEL || undefined });
for (const skin of ['classic', 'glass', 'next']) {
  for (const theme of ['light', 'dark']) {
    if (only && !only.includes(`${skin}-${theme}`)) continue;
    const page = await browser.newPage({
      viewport: { width: 1280, height: 800 },
      deviceScaleFactor: 1,
      locale: 'pl-PL',
    });
    await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
    await page.goto(`${base}/?style=${skin}&theme=${theme}`);
    await page.locator('#alfa-composer').waitFor();
    await page.keyboard.press('Control+,');
    const nodes = page.getByRole('navigation', { name: 'Sekcje ustawień' }).locator('button.node');
    await nodes.first().waitFor();
    const count = await nodes.count();
    for (let i = 0; i < count; i += 1) {
      await nodes.nth(i).click();
      await page.waitForTimeout(350);
      const index = String(i + 1).padStart(2, '0');
      await page.screenshot({ path: join(outDir, `${skin}-${theme}-${index}.png`) });
    }
    await page.close();
  }
}
await browser.close();
console.log(`zrzuty: ${outDir}`);
