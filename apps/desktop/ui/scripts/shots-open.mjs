#!/usr/bin/env node
// Zrzuty kontrolek w stanie OTWARTYM (lista wyboru w ustawieniach i w filtrach) w każdym stylu
// i motywie. Uzupełnia shots.mjs, który pokazuje widoki w stanie spoczynkowym — a błąd nieczytelnej
// listy w stylu „Szkło" był widoczny dopiero po jej rozwinięciu.
//
// Użycie: node scripts/shots-open.mjs <katalog-wyjściowy> [--url http://localhost:4173]
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { chromium } from '@playwright/test';

const args = process.argv.slice(2);
const outDir = resolve(args[0] ?? 'shots-open');
const at = args.indexOf('--url');
const base = at >= 0 ? args[at + 1] : 'http://localhost:4173';
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch({ channel: process.env.PW_CHANNEL || undefined });
for (const skin of ['classic', 'glass', 'next']) {
  for (const theme of ['light', 'dark']) {
    const page = await browser.newPage({ viewport: { width: 1280, height: 760 }, locale: 'pl-PL' });
    await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
    await page.goto(`${base}/?style=${skin}&theme=${theme}`);
    await page.locator('#alfa-composer').waitFor();
    await page.keyboard.press('Control+,');
    const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
    await nav.getByRole('button', { name: 'Wygląd' }).click();
    await page.getByRole('combobox', { name: 'Styl', exact: true }).click();
    await page.getByRole('listbox', { name: 'Styl' }).waitFor();
    await page.screenshot({ path: join(outDir, `${skin}-${theme}-ustawienia.png`) });
    await page.keyboard.press('Escape');
    await nav.getByRole('button', { name: 'Modele i silniki' }).click();
    const kind = page.getByRole('combobox', { name: 'Rodzaj', exact: true });
    await kind.scrollIntoViewIfNeeded();
    await kind.click();
    await page.getByRole('listbox', { name: 'Rodzaj' }).waitFor();
    await page.screenshot({ path: join(outDir, `${skin}-${theme}-filtr.png`) });
    await page.keyboard.press('Escape');
    // Kontrolki, które domyślnie rysuje system: suwaki, pola zaznaczenia, wybór pliku, paski postępu.
    for (const [name, file] of [
      ['Kreator agentek', 'kreator'],
      ['Wtyczki', 'wtyczki'],
      ['Uprawnienia i bezpieczeństwo', 'uprawnienia'],
    ]) {
      await nav.getByRole('button', { name }).first().click();
      await page.waitForTimeout(400);
      await page.screenshot({ path: join(outDir, `${skin}-${theme}-${file}.png`), fullPage: true });
    }
    await page.close();
  }
}
await browser.close();
console.log(`shots-open: zrzuty → ${outDir}`);
