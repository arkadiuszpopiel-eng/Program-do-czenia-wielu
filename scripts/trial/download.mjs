#!/usr/bin/env node
// Pobieranie samym programem (docs/plan/24): na INSTANCJI TESTOWEJ z czystym profilem
// (`scripts/trial/profile.sh start`) zleca pobranie pozycji katalogu poleceniem `models_download`
// i czeka, aż program sam je pobierze, sprawdzi sumy i oznaczy jako zainstalowane.
// Domyślnie: rozpoznawanie mowy Qwen3-ASR (ok. 2,5 GB) i silnik `llama-server` na kartę NVIDIA —
// razem poniżej stałej zgody właściciela (3 GB). Wymaga sieci.
//
//   node scripts/trial/download.mjs [--ids a,b] [--minutes 40]
// Wynik: target/verify/trial/pobieranie.md + .json; kod wyjścia 1, gdy coś się nie pobrało.
import { mkdirSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const require = createRequire(join(root, 'apps', 'desktop', 'ui', 'package.json'));
const { chromium } = require('@playwright/test');

const args = process.argv.slice(2);
const option = (name, fallback) => (args.includes(name) ? args[args.indexOf(name) + 1] : fallback);
const port = process.env.ALFA_TRIAL_PORT ?? '9333';
const ids = option('--ids', 'qwen3-asr-1.7b-q8,sidecar-llama-cuda').split(',');
const minutes = Number(option('--minutes', '40'));
/** Górna granica łącznego rozmiaru bez pytania właściciela (zgoda stała z 2026-10-11). */
const LIMIT_BYTES = 3 * 1024 ** 3;

const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 20_000 });
const main = browser
  .contexts()
  .flatMap((c) => c.pages())
  .find((p) => new URL(p.url()).pathname === '/');
if (!main) throw new Error('Brak okna głównego instancji testowej.');
const invoke = (command, payload) =>
  main.evaluate(
    ([c, a]) =>
      window.__TAURI_INTERNALS__.invoke(c, a).catch((e) => {
        throw new Error(JSON.stringify(e));
      }),
    [command, payload ?? {}],
  );
const gb = (bytes) => `${(bytes / 1024 ** 3).toFixed(2)} GB`;

const catalog = (await invoke('models_list')).items;
const wanted = ids.map((id) => catalog.find((item) => item.id === id) ?? { id, missing: true });
const unknown = wanted.filter((item) => item.missing).map((item) => item.id);
const total = wanted.reduce((sum, item) => sum + (item.size_bytes ?? 0), 0);
const results = [];
if (unknown.length > 0) {
  results.push({
    id: unknown.join(', '),
    state: 'CZERWONY',
    why: 'nie ma takiej pozycji w katalogu',
  });
} else if (total > LIMIT_BYTES) {
  results.push({
    id: ids.join(', '),
    state: 'nieuruchomiony',
    why: `łączny rozmiar ${gb(total)} przekracza 3 GB — wymaga zgody właściciela`,
  });
} else {
  const started = Date.now();
  for (const item of wanted) {
    if (item.state === 'installed') continue;
    await invoke('models_download', { itemId: item.id });
  }
  const deadline = started + minutes * 60_000;
  let states = new Map();
  for (;;) {
    const now = (await invoke('models_list')).items;
    states = new Map(now.filter((i) => ids.includes(i.id)).map((i) => [i.id, i]));
    const done = ids.every((id) => states.get(id)?.state === 'installed');
    const failed = ids.some((id) =>
      ['failed', 'corrupt', 'needs_trust'].includes(states.get(id)?.state),
    );
    if (done || failed || Date.now() > deadline) break;
    await new Promise((resolve) => setTimeout(resolve, 3_000));
  }
  const seconds = Math.round((Date.now() - started) / 1000);
  for (const id of ids) {
    const item = states.get(id);
    const ok = item?.state === 'installed';
    results.push({
      id,
      state: ok ? 'zielony' : 'CZERWONY',
      why: ok
        ? `pobrane i sprawdzone (${gb(item.size_bytes)}, sumy przypięte: ${item.pinned ? 'tak' : 'nie'}), ${seconds} s łącznie`
        : `stan: ${item?.state ?? 'brak'}${item?.error ? ` — ${item.error}` : ''} po ${seconds} s`,
    });
  }
}
await browser.close();

const out = join(root, 'target', 'verify', 'trial');
mkdirSync(out, { recursive: true });
const lines = [
  '# Pobieranie samym programem (instancja testowa, czysty profil)',
  '',
  `Bieg: ${new Date().toISOString()} · pozycje: ${ids.join(', ')} · łącznie ${gb(total)}`,
  '',
  '| Pozycja | Wynik | Szczegóły |',
  '| --- | --- | --- |',
  ...results.map((r) => `| ${r.id} | ${r.state} | ${r.why} |`),
  '',
];
writeFileSync(join(out, 'pobieranie.md'), `${lines.join('\n')}\n`);
writeFileSync(join(out, 'pobieranie.json'), `${JSON.stringify({ results }, null, 2)}\n`);
for (const r of results) console.log(`— ${r.id}: ${r.state} — ${r.why}`);
process.exit(results.some((r) => r.state === 'CZERWONY') ? 1 : 0);
