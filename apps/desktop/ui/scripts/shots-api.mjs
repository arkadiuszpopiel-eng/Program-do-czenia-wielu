#!/usr/bin/env node
// Zrzuty strony „API i kolejki modeli” oraz testu mikrofonu w każdym stylu i motywie: kolejka,
// menu dobierania otwarte, klasa bez kolejki, przeglądarka z filtrem, wąskie okno.
//
// Użycie: node scripts/shots-api.mjs <katalog-wyjściowy> [--url http://127.0.0.1:4173]
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { chromium } from '@playwright/test';

const args = process.argv.slice(2);
const outDir = resolve(args[0] ?? 'shots-api');
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
    await page.keyboard.press('Control+,');
    const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
    await nav.getByRole('button', { name: 'API i kolejki modeli' }).click();
    await page.getByRole('tab', { name: /Praca — OpenAI/ }).click();
    const shot = async (name, full = false) => {
      await page.waitForTimeout(250);
      await page.screenshot({ path: join(outDir, `${skin}-${theme}-${name}.png`), fullPage: full });
    };
    await shot('1-kolejka');
    await page.getByRole('button', { name: 'Dobierz automatycznie' }).click();
    await shot('2-menu-otwarte');
    await page.keyboard.press('Escape');
    await page.getByRole('tab', { name: /^Embeddingi/ }).click();
    await shot('3-klasa-pusta');
    await page.getByRole('tab', { name: /^Rozmowa/ }).click();
    await page.getByLabel('Szukaj modelu').fill('mini');
    await page.getByLabel('Szukaj modelu').scrollIntoViewIfNeeded();
    await shot('4-przegladarka-szukanie');
    await page.getByLabel('Szukaj modelu').fill('');
    await page.getByRole('button', { name: /^Synteza mowy/ }).click();
    await shot('5-przegladarka-filtr');
    await page.setViewportSize({ width: 760, height: 900 });
    await page
      .getByRole('heading', { name: 'Kolejki modeli', exact: true })
      .scrollIntoViewIfNeeded();
    await shot('6-waskie-okno');
    await page.setViewportSize({ width: 1280, height: 900 });
    await nav.getByRole('button', { name: 'Głos', exact: true }).click();
    await page.getByRole('button', { name: /test/i }).first().click();
    await page.waitForTimeout(400);
    await shot('7-test-mikrofonu');
    await page.close();
  }
}
await browser.close();
console.log(`shots-api: zrzuty → ${outDir}`);
