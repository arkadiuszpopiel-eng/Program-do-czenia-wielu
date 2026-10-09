#!/usr/bin/env node
// Zrzuty kreatora „Dodaj dostawcę": krok klucza (pusty, z podpowiedziami) i każdy wynik testu,
// w wybranych stylach i motywach. Wynik testu w atrapie zależy od treści klucza — słowa w środku
// („bad", „restricted", „nocredit"); to nie są prawdziwe klucze.
//
// Użycie: node scripts/shots-wizard.mjs <katalog-wyjściowy> [--url http://127.0.0.1:4173]
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { chromium } from '@playwright/test';

const args = process.argv.slice(2);
const outDir = resolve(args[0] ?? 'shots-wizard');
const at = args.indexOf('--url');
const base = at >= 0 ? args[at + 1] : 'http://127.0.0.1:4173';
mkdirSync(outDir, { recursive: true });

/** Zmyślony klucz o długości prawdziwego: przedrostek + znacznik wyniku + wypełnienie. */
const fakeKey = (prefix, mark) => `${prefix}${mark}-${'XXXX'.repeat(8)}`;
const KEYS = [
  ['blad', fakeKey('sk-', 'bad')],
  ['ograniczony', fakeKey('sk-', 'restricted')],
  ['brak-srodkow', fakeKey('sk-', 'nocredit')],
  ['region', fakeKey('sk-', 'region')],
  ['odmowa', fakeKey('sk-', 'refused')],
  ['bez-testu', fakeKey('sk-', 'unsupported')],
  ['brak-odpowiedzi', fakeKey('sk-', 'timeout')],
  ['dziala', fakeKey('sk-', 'proj')],
];

const browser = await chromium.launch({ channel: process.env.PW_CHANNEL || undefined });
for (const [skin, theme] of [
  ['classic', 'dark'],
  ['classic', 'light'],
  ['glass', 'dark'],
  ['glass', 'light'],
  ['next', 'light'],
  ['next', 'dark'],
]) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 860 }, locale: 'pl-PL' });
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  const open = async () => {
    await page.goto(`${base}/?style=${skin}&theme=${theme}`);
    await page.locator('#alfa-composer').waitFor();
    await page.keyboard.press('Control+,');
    await page
      .getByRole('navigation', { name: 'Sekcje ustawień' })
      .getByRole('button', { name: 'Modele i dostawcy' })
      .click();
    await page.getByRole('button', { name: 'Dodaj dostawcę' }).click();
    await page
      .getByRole('button', { name: /^OpenAI/ })
      .first()
      .click();
    await page.getByLabel('Klucz API').waitFor();
  };
  const shot = async (name) => {
    await page.getByRole('heading', { name: 'Dodaj dostawcę' }).scrollIntoViewIfNeeded();
    await page.waitForTimeout(250);
    await page.screenshot({ path: join(outDir, `${skin}-${theme}-${name}.png`) });
  };
  await open();
  await shot('1-klucz-pusty');
  await page.getByLabel('Klucz API').fill(`"${fakeKey('sk-ant-', 'api03')}"\n`);
  await shot('2-klucz-innego-dostawcy');
  await page.getByLabel('Klucz API').fill('to jest zdanie, nie klucz');
  await shot('2-to-nie-klucz');
  for (const [name, key] of KEYS) {
    await open();
    await page.getByLabel('Klucz API').fill(key);
    await page.getByRole('button', { name: 'Sprawdź klucz' }).click();
    await page.locator('[data-verdict]').first().waitFor();
    if (name !== 'dziala')
      await page
        .locator('details summary')
        .first()
        .click()
        .catch(() => undefined);
    await shot(`3-wynik-${name}`);
  }
  // Lista kont po kreatorze: powód przy koncie, wyjaśnienie i wymiana klucza.
  await open();
  await page.getByLabel('Klucz API').fill(fakeKey('sk-', 'nocredit'));
  await page.getByRole('button', { name: 'Sprawdź klucz' }).click();
  await page.locator('[data-verdict]').first().waitFor();
  await page.getByRole('button', { name: /Zamknij — resztę/ }).click();
  const row2 = () =>
    page
      .getByRole('list', { name: 'Twoje konta' })
      .getByRole('listitem')
      .filter({ hasText: /^OpenAI/ });
  const row = row2();
  await row.getByRole('button', { name: 'Co zrobić' }).click();
  await row.scrollIntoViewIfNeeded();
  await page.waitForTimeout(250);
  await page.screenshot({ path: join(outDir, `${skin}-${theme}-4-lista-powod.png`) });
  await row.getByRole('button', { name: 'Zmień klucz' }).first().click();
  await row.getByLabel('Klucz API').fill(fakeKey('sk-', 'proj'));
  await page.waitForTimeout(250);
  await page.screenshot({ path: join(outDir, `${skin}-${theme}-5-lista-zmiana-klucza.png`) });
  await row.getByRole('button', { name: 'Sprawdź i zamień klucz' }).click();
  await page.locator('[data-verdict]').first().waitFor();
  await page.waitForTimeout(250);
  await page.screenshot({ path: join(outDir, `${skin}-${theme}-6-lista-po-zmianie.png`) });
  await row2().getByRole('button', { name: 'Zamknij' }).click();
  await row2().getByRole('button', { name: 'Zmień klucz' }).click();
  await row2().getByLabel('Klucz API').fill(fakeKey('sk-', 'bad'));
  await row2().getByRole('button', { name: 'Sprawdź i zamień klucz' }).click();
  await page.locator('[data-verdict]').first().waitFor();
  await page.waitForTimeout(250);
  await page.screenshot({ path: join(outDir, `${skin}-${theme}-7-lista-nowy-odrzucony.png`) });
  await page.close();
}
await browser.close();
console.log(`shots-wizard: zrzuty → ${outDir}`);
