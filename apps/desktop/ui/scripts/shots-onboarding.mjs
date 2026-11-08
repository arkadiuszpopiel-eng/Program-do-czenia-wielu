#!/usr/bin/env node
// Zrzuty wprowadzenia („Witaj w Alfie”): każdy krok w wybranych stylach i motywach, na atrapie
// (scenariusz `first-run`). Krok „Konta i klucze” także z otwartym kreatorem dostawcy.
//
// Użycie: node scripts/shots-onboarding.mjs <katalog-wyjściowy> [--url http://127.0.0.1:5173]
//         [--only classic-dark] [--size 1280x860]
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { chromium } from '@playwright/test';

const args = process.argv.slice(2);
const option = (name, fallback) => (args.includes(name) ? args[args.indexOf(name) + 1] : fallback);
const outDir = resolve(args[0] ?? 'shots-onboarding');
const base = option('--url', 'http://127.0.0.1:5173');
const only = option('--only', null);
const [width, height] = option('--size', '1280x860').split('x').map(Number);
mkdirSync(outDir, { recursive: true });

const STEPS = ['mikrofon', 'glos', 'sprzet', 'konta', 'autonomia', 'korpus', 'mosty', 'import'];
const browser = await chromium.launch({ channel: process.env.PW_CHANNEL || undefined });
for (const [skin, theme] of [
  ['classic', 'dark'],
  ['classic', 'light'],
  ['glass', 'dark'],
  ['glass', 'light'],
  ['next', 'dark'],
  ['next', 'light'],
]) {
  if (only && only !== `${skin}-${theme}`) continue;
  const page = await browser.newPage({ viewport: { width, height }, locale: 'pl-PL' });
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`${base}/?scenario=first-run&style=${skin}&theme=${theme}`);
  await page.getByRole('heading', { name: 'Witaj w Alfie' }).waitFor();
  for (const [index, name] of STEPS.entries()) {
    await page.waitForTimeout(500);
    const file = `${skin}-${theme}-${index + 1}-${name}`;
    await page.screenshot({ path: join(outDir, `${file}.png`) });
    if (name === 'konta') {
      await page.getByRole('button', { name: 'Dodaj teraz' }).click();
      await page.waitForTimeout(400);
      await page.screenshot({ path: join(outDir, `${file}-kreator.png`), fullPage: true });
      await page.getByRole('button', { name: 'Pomiń — dodam później' }).click();
      await page.getByRole('button', { name: 'Dodaj teraz' }).waitFor();
      await page.getByRole('button', { name: 'Pomiń — dodam później' }).click();
    } else if (index < STEPS.length - 1) {
      await page.getByRole('button', { name: 'Dalej', exact: true }).click();
    }
  }
  await page.close();
}
await browser.close();
console.log(`zrzuty: ${outDir}`);
