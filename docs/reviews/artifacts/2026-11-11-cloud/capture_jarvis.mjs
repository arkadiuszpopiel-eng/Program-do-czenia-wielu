import { createRequire } from 'node:module';
import { mkdir, writeFile } from 'node:fs/promises';
const require = createRequire('/workspace/wt-jarvis/apps/desktop/ui/package.json');
const { chromium } = require('@playwright/test');
const phase = process.argv[2];
const out = `/workspace/scratch/repair-20261111/ui-${phase}`;
await mkdir(out, { recursive: true });
const browser = await chromium.launch({ headless: true });
const results = [];
try {
  for (const [width, height] of [[1280, 900], [390, 844], [320, 568]]) {
    const context = await browser.newContext({ viewport: { width, height }, locale: 'pl-PL', colorScheme: 'dark', reducedMotion: 'reduce' });
    const page = await context.newPage();
    const errors = [];
    const external = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('request', r => { if (!r.url().startsWith('http://127.0.0.1:4173/')) external.push(r.url()); });
    await page.goto('http://127.0.0.1:4173/?style=next&theme=dark');
    await page.locator('#alfa-composer').waitFor();
    await page.locator('#alfa-boot').waitFor({ state: 'detached' });
    await page.getByRole('button', { name: 'Włącz kokpit Jarvis', exact: true }).click();
    await page.getByRole('region', { name: 'Kokpit Jarvis', exact: true }).waitFor();
    await page.screenshot({ path: `${out}/${width}x${height}.png` });
    if (phase.startsWith('after')) {
      await page.getByRole('button', { name: 'Mapa i systemy', exact: true }).click();
      await page.getByRole('dialog', { name: 'Mapa i systemy', exact: true }).waitFor();
      await page.screenshot({ path: `${out}/atlas-${width}x${height}.png` });
    }
    results.push({ width, height, errors, external, layout: await page.evaluate(() => ({ scrollWidth: document.documentElement.scrollWidth, viewport: innerWidth })) });
    await context.close();
  }
} finally { await browser.close(); }
await writeFile(`${out}/capture.json`, JSON.stringify({ scope: 'headless Linux Chromium + fake IPC; not physical phone/WebView2', results }, null, 2));
console.log(JSON.stringify(results));
