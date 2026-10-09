#!/usr/bin/env node
// Zrzuty prawej części górnej belki (obsada, mikrofon, stan) w każdym stylu i motywie: spoczynek,
// mikrofon włączony, otwarte objaśnienie stanu, otwarte koszty, brak sieci, wąskie okno.
//
// Użycie: node scripts/shots-bar.mjs <katalog-wyjściowy> [--url http://127.0.0.1:4173]
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { chromium } from '@playwright/test';

const args = process.argv.slice(2);
const outDir = resolve(args[0] ?? 'shots-bar');
const at = args.indexOf('--url');
const base = at >= 0 ? args[at + 1] : 'http://127.0.0.1:4173';
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch({ channel: process.env.PW_CHANNEL || undefined });
for (const skin of ['classic', 'glass', 'next']) {
  for (const theme of ['light', 'dark']) {
    const page = await browser.newPage({
      viewport: { width: 1280, height: 520 },
      deviceScaleFactor: 2,
      locale: 'pl-PL',
    });
    await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
    const file = (name) => join(outDir, `${skin}-${theme}-${name}.png`);
    const bar = { x: 480, y: 0, width: 800, height: 44 };
    const open = async (extra = '') => {
      await page.goto(`${base}/?style=${skin}&theme=${theme}${extra}`);
      await page.locator('#alfa-composer').waitFor();
      await page.waitForTimeout(500);
    };
    await open();
    await page.screenshot({ path: file('1-spoczynek'), clip: bar });
    await page.getByRole('group', { name: 'Obsada agentek' }).getByRole('button').nth(1).hover();
    await page.waitForTimeout(700);
    await page.screenshot({ path: file('2-agentka-podpowiedz'), clip: { ...bar, height: 110 } });
    await page.keyboard.press('Control+Shift+M');
    await page.waitForTimeout(600);
    await page.screenshot({ path: file('3-mikrofon-wlaczony'), clip: bar });
    await page.keyboard.press('Control+Shift+M');
    await page.getByRole('button', { name: /^Stan: / }).click();
    await page.waitForTimeout(300);
    await page.screenshot({ path: file('4-stan-otwarty'), clip: { ...bar, height: 420 } });
    await page.keyboard.press('Escape');
    await page.getByRole('button', { name: /^Koszt sesji/ }).click();
    await page.waitForTimeout(300);
    await page.screenshot({ path: file('5-koszty-otwarte'), clip: { ...bar, height: 420 } });
    await page.keyboard.press('Escape');
    await open('&scenario=offline');
    await page.screenshot({ path: file('6-brak-sieci'), clip: bar });
    await open();
    await page.setViewportSize({ width: 620, height: 520 });
    await page.waitForTimeout(300);
    await page.screenshot({ path: file('7-waskie'), clip: { x: 0, y: 0, width: 620, height: 44 } });
    await page.setViewportSize({ width: 400, height: 520 });
    await page.waitForTimeout(300);
    await page.screenshot({ path: file('8-400'), clip: { x: 0, y: 0, width: 400, height: 44 } });
    await page.close();
  }
}
await browser.close();
console.log(`zrzuty: ${outDir}`);
