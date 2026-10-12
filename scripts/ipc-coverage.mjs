#!/usr/bin/env node
// Pokrycie poleceń rdzenia w teście prawdziwej aplikacji (docs/plan/24 §2.3).
// Wejście: `ipc-seen.txt` ze śladu poleceń (apps/desktop/ui/e2e-app/ipc.ts) i spis poleceń ze
// schematu łącza. Wyjście: liczby, lista nieprzećwiczonych poleceń pogrupowana według przedrostka,
// opcjonalnie plik JSON. `--min <procent>` kończy błędem, gdy pokrycie jest niższe (bramka).
//
//   node scripts/ipc-coverage.mjs <katalog-z-ipc-seen.txt> [--json <plik>] [--min <procent>]
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const dir = args[0];
const option = (name) => {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : null;
};
if (!dir) {
  console.error(
    'Użycie: node scripts/ipc-coverage.mjs <katalog-z-ipc-seen.txt> [--json plik] [--min procent]',
  );
  process.exit(2);
}
const seenFile = join(dir, 'ipc-seen.txt');
if (!existsSync(seenFile)) {
  console.error(`Brak ${seenFile} — najpierw test prawdziwej aplikacji (e2e-app/run.ps1).`);
  process.exit(2);
}
const schema = JSON.parse(readFileSync(join(root, 'packages/schemas/ipc.v1.json'), 'utf8'));
const all = Object.keys(schema.commands).sort();
const counts = new Map();
for (const line of readFileSync(seenFile, 'utf8').split(/\r?\n/)) {
  const name = line.trim();
  if (name) counts.set(name, (counts.get(name) ?? 0) + 1);
}
const seen = all.filter((name) => counts.has(name));
const missing = all.filter((name) => !counts.has(name));
const unknown = [...counts.keys()].filter((name) => !all.includes(name)).sort();
const percent = Math.round((seen.length / all.length) * 1000) / 10;

const groups = new Map();
for (const name of missing) {
  const prefix = name.split('_')[0];
  groups.set(prefix, [...(groups.get(prefix) ?? []), name]);
}
console.log(
  `Polecenia rdzenia przećwiczone w prawdziwej aplikacji: ${seen.length} z ${all.length} (${percent} %)`,
);
console.log(`Nieprzećwiczone: ${missing.length}`);
for (const [prefix, names] of [...groups].sort((a, b) => b[1].length - a[1].length)) {
  console.log(`  ${prefix} (${names.length}): ${names.join(', ')}`);
}
if (unknown.length > 0) console.log(`Wywołania spoza schematu: ${unknown.join(', ')}`);

const json = option('--json');
if (json) {
  writeFileSync(
    json,
    `${JSON.stringify({ total: all.length, seen: seen.length, percent, missing, unknown }, null, 2)}\n`,
  );
}
const min = option('--min');
if (min !== null && percent < Number(min)) {
  console.error(`Pokrycie ${percent} % jest niższe niż próg ${min} %.`);
  process.exit(1);
}
