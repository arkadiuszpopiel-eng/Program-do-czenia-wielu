#!/usr/bin/env node
// Generuje listę licencji zależności dla strony „O programie" (crates/app-updates/data/licenses.json):
//  - Rust: `cargo metadata` powłoki Tauri (apps/desktop/src-tauri — pełne drzewo binarium Alfy,
//    platforma x86_64-pc-windows-msvc), tylko crate'y z rejestru (bez własnych crate'ów workspace);
//  - npm: `pnpm licenses list --prod --json` dla UI (@alfa/desktop-ui, zależności produkcyjne).
// Użycie: node apps/desktop/scripts/gen-licenses.mjs [--check]
//   --check — kończy się kodem 1, gdy plik jest nieaktualny (bez zapisu; data generowania pomijana).
// Uruchamia go workflow wydania przed budową (lista zawsze zgodna z wydaniem); w repo — migawka.
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const out = join(root, 'crates/app-updates/data/licenses.json');
const check = process.argv.includes('--check');

function run(cmd, args, cwd = root) {
  return execFileSync(cmd, args, {
    cwd,
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
    stdio: ['ignore', 'pipe', 'inherit'],
  });
}

function cargoEntries() {
  const meta = JSON.parse(
    run('cargo', [
      'metadata',
      '--format-version',
      '1',
      '--manifest-path',
      'apps/desktop/src-tauri/Cargo.toml',
      '--filter-platform',
      'x86_64-pc-windows-msvc',
    ]),
  );
  const used = new Set(meta.resolve.nodes.map((n) => n.id));
  return meta.packages
    .filter((p) => p.source && used.has(p.id))
    .map((p) => ({
      name: p.name,
      version: p.version,
      license: p.license ?? (p.license_file ? 'plik licencji w pakiecie' : 'nieznana'),
      source: 'cargo',
    }));
}

function npmEntries() {
  const raw = JSON.parse(
    run('pnpm', ['licenses', 'list', '--prod', '--json'], join(root, 'apps/desktop/ui')),
  );
  const entries = [];
  for (const [license, packages] of Object.entries(raw)) {
    for (const p of packages) {
      for (const version of p.versions ?? []) {
        entries.push({ name: p.name, version, license, source: 'npm' });
      }
    }
  }
  return entries;
}

const entries = [...cargoEntries(), ...npmEntries()]
  .filter(
    (e, i, all) =>
      all.findIndex(
        (o) => o.name === e.name && o.version === e.version && o.source === e.source,
      ) === i,
  )
  .sort((a, b) =>
    a.source === b.source
      ? a.name.localeCompare(b.name) || a.version.localeCompare(b.version)
      : a.source.localeCompare(b.source),
  );

const body = (generatedAt) =>
  JSON.stringify({ generated_at: generatedAt, entries }, null, 1) + '\n';

if (check) {
  const current = JSON.parse(readFileSync(out, 'utf8'));
  const same = body(current.generated_at) === readFileSync(out, 'utf8');
  console.log(
    same ? 'licenses.json aktualny' : 'licenses.json NIEAKTUALNY — uruchom gen-licenses.mjs',
  );
  process.exit(same ? 0 : 1);
}
writeFileSync(out, body(new Date().toISOString().slice(0, 10)));
console.log(`licenses.json: ${entries.length} pozycji (${out})`);
