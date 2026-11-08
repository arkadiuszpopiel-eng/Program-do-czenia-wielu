#!/usr/bin/env node
// Zrzuty górnej belki (pasek LED w ruchu, przyciski okna: najechany „zamknij”, okno
// zmaksymalizowane), strony „Urządzenie” i nawigacji Ustawień w każdym stylu i motywie.
//
// Użycie: node scripts/shots-chrome.mjs <katalog-wyjściowy> [--url http://127.0.0.1:4173]
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { chromium } from '@playwright/test';

const args = process.argv.slice(2);
const outDir = resolve(args[0] ?? 'shots-chrome');
const at = args.indexOf('--url');
const base = at >= 0 ? args[at + 1] : 'http://127.0.0.1:4173';
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch({ channel: process.env.PW_CHANNEL || undefined });
for (const skin of ['classic', 'glass', 'next']) {
  for (const theme of ['light', 'dark']) {
    const page = await browser.newPage({ viewport: { width: 1280, height: 860 }, locale: 'pl-PL' });
    await page.emulateMedia({ colorScheme: theme });
    await page.goto(`${base}/?style=${skin}&theme=${theme}`);
    await page.locator('#alfa-composer').waitFor();
    const file = (name) => join(outDir, `${skin}-${theme}-${name}.png`);
    const bar = { x: 0, y: 0, width: 1280, height: 60 };
    // Światło w trzech miejscach drogi.
    for (const [i, wait] of [600, 1100, 1100].entries()) {
      await page.waitForTimeout(wait);
      await page.screenshot({ path: file(`1-led-${i + 1}`), clip: bar });
    }
    await page.getByRole('button', { name: 'Zamknij', exact: true }).hover();
    await page.screenshot({ path: file('2-zamknij-najechany'), clip: bar });
    await page.getByRole('button', { name: 'Maksymalizuj' }).click();
    await page.getByRole('button', { name: 'Przywróć rozmiar' }).hover();
    await page.waitForTimeout(700);
    await page.screenshot({ path: file('3-zmaksymalizowane') });
    await page.keyboard.press('Control+,');
    const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
    await nav.waitFor();
    await page.waitForTimeout(300);
    await page.screenshot({ path: file('4-ustawienia') });
    await nav.getByRole('button', { name: 'Kolejki modeli API', exact: true }).hover();
    await nav.getByRole('button', { name: 'Wygląd', exact: true }).focus();
    await page.getByRole('button', { name: 'Grupa: Głos' }).click();
    await page.getByRole('button', { name: 'Grupa: Bezpieczeństwo' }).click();
    await page.screenshot({ path: file('4a-grupy-zwiniete-najechanie-fokus') });
    await nav.hover();
    await page.mouse.wheel(0, 500);
    await page.waitForTimeout(250);
    await page.screenshot({ path: file('4b-przewiniete') });
    await page.getByLabel('Szukaj ustawień').fill('mikrofon');
    await page.waitForTimeout(250);
    await page.screenshot({ path: file('4c-szukanie') });
    await page.getByLabel('Szukaj ustawień').fill('qqqq');
    await page.waitForTimeout(200);
    await page.screenshot({ path: file('4d-szukanie-puste') });
    await page.getByRole('button', { name: 'Wyczyść szukanie' }).click();
    await page.getByRole('button', { name: 'Zwiń wszystkie grupy' }).click();
    await page.waitForTimeout(200);
    await page.screenshot({ path: file('4e-wszystko-zwiniete') });
    await page.getByRole('button', { name: 'Rozwiń wszystkie grupy' }).click();
    await nav.getByRole('button', { name: 'Urządzenia', exact: true }).click();
    await page.waitForTimeout(400);
    await page.screenshot({ path: file('5-urzadzenia') });
    await page.setViewportSize({ width: 620, height: 860 });
    await page.waitForTimeout(300);
    await page.screenshot({ path: file('6-waskie') });
    await page.getByRole('button', { name: /^Sekcja: Urządzenia/ }).click();
    await page.waitForTimeout(250);
    await page.screenshot({ path: file('6a-waskie-lista-otwarta') });
    await page.close();
  }
}
await browser.close();
console.log(`zrzuty: ${outDir}`);
