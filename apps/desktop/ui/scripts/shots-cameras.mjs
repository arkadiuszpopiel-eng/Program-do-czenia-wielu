#!/usr/bin/env node
// Zrzuty strony „Kamery” w każdym stylu i motywie: kamery wyłączone, włączone, dwa podglądy naraz
// (wskaźnik w belce), zmiana nazwy, wąskie okno.
//
// Użycie: node scripts/shots-cameras.mjs <katalog-wyjściowy> [--url http://127.0.0.1:4173]
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { chromium } from '@playwright/test';

const args = process.argv.slice(2);
const outDir = resolve(args[0] ?? 'shots-cameras');
const at = args.indexOf('--url');
const base = at >= 0 ? args[at + 1] : 'http://127.0.0.1:4173';
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch({ channel: process.env.PW_CHANNEL || undefined });
for (const skin of ['classic', 'glass', 'next']) {
  for (const theme of ['light', 'dark']) {
    const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, locale: 'pl-PL' });
    await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
    await page.goto(`${base}/?style=${skin}&theme=${theme}`);
    await page.locator('#alfa-composer').waitFor();
    const file = (name) => join(outDir, `${skin}-${theme}-${name}.png`);
    await page.keyboard.press('Control+,');
    await page
      .getByRole('navigation', { name: 'Sekcje ustawień' })
      .getByRole('button', { name: 'Kamery', exact: true })
      .click();
    await page.getByRole('heading', { name: 'Twoje kamery' }).waitFor();
    await page.waitForTimeout(300);
    await page.screenshot({ path: file('1-wylaczone') });
    await page.getByRole('switch', { name: 'Kamery włączone' }).click();
    await page.waitForTimeout(300);
    await page.screenshot({ path: file('2-wlaczone') });
    const previews = page.getByRole('button', { name: 'Podgląd', exact: true });
    await previews.first().click();
    await previews.first().click();
    await page.locator('li.camera img').nth(1).waitFor();
    await page.waitForTimeout(700);
    await page.screenshot({ path: file('3-dwa-podglady') });
    await page.getByRole('button', { name: /^Zmień nazwę kamery Integrated/ }).click();
    await page.getByLabel('Nazwa kamery').fill('Biurko');
    await page.screenshot({ path: file('4-zmiana-nazwy') });
    await page.keyboard.press('Enter');
    await page.setViewportSize({ width: 620, height: 900 });
    await page.waitForTimeout(500);
    await page.screenshot({ path: file('5-waskie') });
    await page.close();
  }
}
await browser.close();
console.log(`zrzuty: ${outDir}`);
