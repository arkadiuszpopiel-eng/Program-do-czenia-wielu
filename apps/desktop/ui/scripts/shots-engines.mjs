#!/usr/bin/env node
// Zrzuty karty „Silniki rozmowy” (Ustawienia → Głos) w każdym stylu i motywie: bezpiecznik po
// zadziałaniu, po zamknięciu, wąskie okno oraz scenariusz bez mikrofonu (brakujące pozycje).
//
// Użycie: node scripts/shots-engines.mjs <katalog-wyjściowy> [--url http://localhost:4173]
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { chromium } from '@playwright/test';

const args = process.argv.slice(2);
const outDir = resolve(args[0] ?? 'shots-engines');
const at = args.indexOf('--url');
const base = at >= 0 ? args[at + 1] : 'http://localhost:4173';
mkdirSync(outDir, { recursive: true });

async function openCard(page, query) {
  await page.goto(`${base}/?${query}`);
  await page.locator('#alfa-composer').waitFor();
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Głos', exact: true })
    .click();
  const card = page.getByTestId('voice-engines');
  await card.waitFor();
  await card.scrollIntoViewIfNeeded();
  await page.waitForTimeout(300);
  return card;
}

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
    const card = await openCard(page, `style=${skin}&theme=${theme}`);
    await card.screenshot({ path: file('1-bezpiecznik-otwarty') });
    await card.getByRole('button', { name: 'Zamknij bezpiecznik' }).hover();
    await card.screenshot({ path: file('2-najechanie') });
    await card.getByRole('button', { name: 'Zamknij bezpiecznik' }).click();
    await card.getByText('Silnik główny pracuje').waitFor();
    await card.screenshot({ path: file('3-zamkniety') });
    await page.setViewportSize({ width: 520, height: 900 });
    await page.waitForTimeout(300);
    await card.screenshot({ path: file('4-waskie') });
    await page.setViewportSize({ width: 1280, height: 1000 });
    const missing = await openCard(page, `style=${skin}&theme=${theme}&scenario=no-mic`);
    await missing.screenshot({ path: file('5-braki') });
    await page.close();
  }
}
await browser.close();
console.log(`zrzuty: ${outDir}`);
