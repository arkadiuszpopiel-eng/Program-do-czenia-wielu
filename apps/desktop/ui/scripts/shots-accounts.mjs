#!/usr/bin/env node
// Zrzuty strony „Konta i klucze API” w każdym stylu i motywie: lista kont z wiekiem klucza
// i przypomnieniem o wymianie oraz ustawienia sprawdzania kluczy w tle.
//
// Użycie: node scripts/shots-accounts.mjs <katalog-wyjściowy> [--url http://127.0.0.1:4173]
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { chromium } from '@playwright/test';

const args = process.argv.slice(2);
const outDir = resolve(args[0] ?? 'shots-accounts');
const at = args.indexOf('--url');
const base = at >= 0 ? args[at + 1] : 'http://127.0.0.1:4173';
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch({ channel: process.env.PW_CHANNEL || undefined });
for (const skin of ['classic', 'glass', 'next']) {
  for (const theme of ['light', 'dark']) {
    const page = await browser.newPage({
      viewport: { width: 1280, height: 900 },
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
      .getByRole('button', { name: 'Konta i klucze API', exact: true })
      .click();
    await page.getByText('Wiek klucza (dni): 12').waitFor();
    await page.waitForTimeout(300);
    await page.screenshot({ path: file('1-konta') });
    await page
      .getByRole('spinbutton', { name: 'Sprawdzanie kluczy w tle' })
      .scrollIntoViewIfNeeded();
    await page.getByRole('spinbutton', { name: 'Sprawdzanie kluczy w tle' }).focus();
    await page.waitForTimeout(200);
    await page.screenshot({ path: file('2-ustawienia') });
    await page.close();
  }
}
await browser.close();
console.log(`zrzuty: ${outDir}`);
