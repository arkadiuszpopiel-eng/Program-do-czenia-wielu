#!/usr/bin/env node
// Zrzuty „Historii zmian” (Ustawienia → Aktualizacje) w każdym stylu i motywie: lista z najnowszą
// wersją rozwiniętą, wszystkie wersje rozwinięte, filtr „Krytyczne”, wąskie okno.
//
// Użycie: node scripts/shots-history.mjs <katalog-wyjściowy> [--url http://localhost:4173]
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { chromium } from '@playwright/test';

const args = process.argv.slice(2);
const outDir = resolve(args[0] ?? 'shots-history');
const at = args.indexOf('--url');
const base = at >= 0 ? args[at + 1] : 'http://localhost:4173';
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch({ channel: process.env.PW_CHANNEL || undefined });
for (const skin of ['classic', 'glass', 'next']) {
  for (const theme of ['light', 'dark']) {
    const page = await browser.newPage({
      viewport: { width: 1280, height: 1000 },
      deviceScaleFactor: 1,
      locale: 'pl-PL',
    });
    await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
    const file = (name) => join(outDir, `${skin}-${theme}-${name}.png`);
    await page.goto(`${base}/?style=${skin}&theme=${theme}`);
    await page.locator('#alfa-composer').waitFor();
    await page.keyboard.press('Control+,');
    await page
      .getByRole('navigation', { name: 'Sekcje ustawień' })
      .getByRole('button', { name: 'Aktualizacje', exact: true })
      .click();
    const history = page.getByRole('region', { name: 'Historia zmian' });
    await history.waitFor();
    await history.scrollIntoViewIfNeeded();
    await page.waitForTimeout(300);
    await history.screenshot({ path: file('1-najnowsza') });
    await history.getByRole('button', { name: 'Rozwiń wszystkie' }).click();
    await history.getByRole('button', { name: /^Wersja 0\.0\.9/ }).hover();
    await page.waitForTimeout(200);
    await history.screenshot({ path: file('2-wszystkie') });
    await history.getByRole('button', { name: /^Krytyczne/ }).click();
    await page.waitForTimeout(200);
    await history.screenshot({ path: file('3-krytyczne') });
    await history.getByRole('button', { name: /^Wszystkie/ }).focus();
    await page.setViewportSize({ width: 520, height: 900 });
    await page.waitForTimeout(300);
    await history.screenshot({ path: file('4-waskie') });
    await page.close();
  }
}
await browser.close();
console.log(`zrzuty: ${outDir}`);
