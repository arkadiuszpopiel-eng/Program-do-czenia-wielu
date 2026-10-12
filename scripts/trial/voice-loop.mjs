#!/usr/bin/env node
// Tor akustyczny (docs/plan/24 §2.4): komputer rozmawia sam ze sobą. Działa na INSTANCJI TESTOWEJ
// (`scripts/trial/profile.sh start --voice`) i ODTWARZA DŹWIĘK z głośnika — tylko w dzień.
// Nie zmienia głośności ani urządzeń systemu; gdy mikrofon nie słyszy głośnika, mówi to wprost.
//
//  1. „głośnik → mikrofon”: nagranie syntetycznego głosu z głośnika ma trafić do rozmowy jako
//     wypowiedź użytkownika (wykryta, zakończona, niepusty tekst; zgodność słów — informacyjnie);
//  2. „echo”: gdy agentka mówi z głośnika, jej głos NIE może wrócić jako wypowiedź użytkownika.
//
//   node scripts/trial/voice-loop.mjs [--wav plik.wav] [--expect "tekst"]
// Wynik: target/verify/trial/glos.md + glos.json; kod wyjścia 1, gdy próba czerwona.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const require = createRequire(join(root, 'apps', 'desktop', 'ui', 'package.json'));
const { chromium } = require('@playwright/test');

const args = process.argv.slice(2);
const option = (name, fallback) => (args.includes(name) ? args[args.indexOf(name) + 1] : fallback);
const port = process.env.ALFA_TRIAL_PORT ?? '9333';
const wav = option(
  '--wav',
  join(root, '..', 'alfa-glos-proba', 'wyniki', 'supertonic3', 'F1-01.wav'),
);
const expected = option('--expect', 'Dzień dobry, tu Alfa. W czym mogę dziś pomóc?');
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** Słowa bez znaków przestankowych i wielkości liter; „alpha” = „alfa” (pisownia rozpoznawania). */
const words = (text) =>
  text
    .toLowerCase()
    .replace(/alpha/g, 'alfa')
    .replace(/[^\p{L}\p{N}\s]/gu, ' ')
    .split(/\s+/)
    .filter(Boolean);

/** Odsetek błędnych słów (odległość edycyjna na słowach / długość wzorca). */
export function wordErrorRate(reference, hypothesis) {
  const a = words(reference);
  const b = words(hypothesis);
  const row = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    let previous = row[0];
    row[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const current = row[j];
      row[j] = Math.min(row[j] + 1, row[j - 1] + 1, previous + (a[i - 1] === b[j - 1] ? 0 : 1));
      previous = current;
    }
  }
  return a.length === 0 ? 1 : row[b.length] / a.length;
}

function play(path) {
  const command = `(New-Object System.Media.SoundPlayer '${path.replace(/'/g, "''")}').PlaySync()`;
  return (
    spawnSync('powershell', ['-NoProfile', '-Command', command], { encoding: 'utf8' }).status === 0
  );
}

const textOf = (turn) =>
  (turn.blocks ?? [])
    .map((block) => String(block.html_sanitized ?? block.text ?? ''))
    .join(' ')
    .replace(/<[^>]+>/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();

if (!existsSync(wav)) {
  console.error(`Brak nagrania ${wav}`);
  process.exit(2);
}
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
const userTurns = async (sessionId) =>
  (await invoke('turns_list', { sessionId })).turns.filter((t) => t.author === 'user');

const results = [];
const session = await invoke('sessions_create', { template: 'empty' });
await invoke('app_set_active_session', { sessionId: session.id });
let refused = '';
try {
  await invoke('voice_set_mic_enabled', { enabled: true });
} catch (error) {
  refused = String(error.message ?? error)
    .split(/\r?\n/)[0]
    .slice(0, 300);
}
let status = await invoke('voice_status');
for (let i = 0; i < 30 && status.state !== 'active' && !refused; i++) {
  await sleep(1_000);
  status = await invoke('voice_status');
}
if (status.state !== 'active') {
  results.push({
    id: 'glosnik-mikrofon',
    state: 'nieuruchomiony',
    why: `rozmowa głosowa niedostępna: ${status.reason?.pl ?? status.state}; brakuje: ${(status.missing ?? []).join(', ') || '—'}${refused ? `; odmowa: ${refused}` : ''}`,
  });
} else {
  // Silniki rozgrzewają się po włączeniu mikrofonu — pierwsze rozpoznanie bywa wolniejsze.
  await sleep(12_000);
  const before = (await userTurns(session.id)).length;
  const played = play(wav);
  let heard = null;
  for (let i = 0; i < 40 && !heard; i++) {
    await sleep(1_000);
    const turns = await userTurns(session.id);
    if (turns.length > before) heard = textOf(turns[turns.length - 1]);
  }
  const rate = heard ? wordErrorRate(expected, heard) : 1;
  results.push({
    id: 'glosnik-mikrofon',
    // Kryterium: wypowiedź z głośnika została wykryta, zakończona i zamieniona na niepusty tekst.
    // Zgodność słów jest tylko informacją — mikrofon laptopa tłumi własny głośnik, więc surowe
    // nagranie tego układu też jest rozpoznawane źle (pomiar 2026-10-12, docs/plan/24).
    state: !played ? 'CZERWONY' : heard && heard.trim().length > 0 ? 'zielony' : 'CZERWONY',
    why: !played
      ? 'nie udało się odtworzyć nagrania'
      : heard
        ? `wypowiedź wykryta i zakończona; rozpoznano: „${heard.trim().slice(0, 160)}” (błędne słowa ${(rate * 100).toFixed(0)} % — informacyjnie)`
        : 'mikrofon nie usłyszał głośnika w 40 s (głośność systemu? wyciszony głośnik? słuchawki?)',
  });

  // Echo: agentka mówi próbkę głosu z głośnika; przez 15 s nie może przybyć wypowiedź użytkownika.
  await sleep(4_000);
  const quiet = (await userTurns(session.id)).length;
  let spoke = true;
  try {
    await invoke('voice_preview', { agent: 'alfa' });
  } catch (error) {
    spoke = false;
    results.push({
      id: 'echo',
      state: 'nieuruchomiony',
      why: `próbka głosu niedostępna: ${String(error).slice(0, 200)}`,
    });
  }
  if (spoke) {
    await sleep(15_000);
    const after = await userTurns(session.id);
    const leaked = after.slice(quiet).map(textOf);
    results.push({
      id: 'echo',
      state: leaked.length === 0 ? 'zielony' : 'CZERWONY',
      why:
        leaked.length === 0
          ? 'głos agentki nie wrócił jako wypowiedź użytkownika'
          : `echo w rozmowie: ${leaked.join(' | ').slice(0, 200)}`,
    });
  }
}
await invoke('voice_set_mic_enabled', { enabled: false }).catch(() => undefined);
await browser.close();

const out = join(root, 'target', 'verify', 'trial');
mkdirSync(out, { recursive: true });
const lines = [
  '# Tor akustyczny: głośnik → mikrofon → rozpoznawanie',
  '',
  `Bieg: ${new Date().toISOString()} · nagranie: ${wav}`,
  '',
  '| Próba | Wynik | Szczegóły |',
  '| --- | --- | --- |',
  ...results.map((r) => `| ${r.id} | ${r.state} | ${r.why} |`),
  '',
];
writeFileSync(join(out, 'glos.md'), `${lines.join('\n')}\n`);
writeFileSync(join(out, 'glos.json'), `${JSON.stringify({ results }, null, 2)}\n`);
for (const r of results) console.log(`— ${r.id}: ${r.state} — ${r.why}`);
process.exit(results.some((r) => r.state === 'CZERWONY') ? 1 : 0);
