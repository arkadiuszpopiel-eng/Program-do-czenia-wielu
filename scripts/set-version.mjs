#!/usr/bin/env node
// Jedno miejsce do zmiany numeru wersji programu. Numer żyje w kilku plikach naraz: workspace Cargo,
// powłoka Tauri (dwa pliki), interfejs oraz manifesty modułów (`crates/*/module.toml`), których testy
// porównują wersję modułu z wersją crate'a. Ręczne podbicie czterech plików zostawiało manifesty
// przy starej wersji i psuło ok. 20 testów — stąd skrypt i bramka.
//
// Użycie:
//   node scripts/set-version.mjs 0.0.7    ustawia wersję wszędzie (wymaga wpisu w CHANGELOG.md)
//   node scripts/set-version.mjs --check  sprawdza spójność; kod wyjścia 1 przy rozjeździe
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const read = (path) => readFileSync(join(root, path), 'utf8');

/** Pliki z numerem wersji: ścieżka i wyrażenie z jedną grupą na sam numer (pierwsze trafienie). */
const FILES = [
  ['Cargo.toml', /^\[workspace\.package\]\r?\nversion = "([^"]+)"/m],
  ['apps/desktop/src-tauri/Cargo.toml', /^version = "([^"]+)"/m],
  ['apps/desktop/src-tauri/tauri.conf.json', /^\s*"version": "([^"]+)"/m],
  ['apps/desktop/ui/package.json', /^\s*"version": "([^"]+)"/m],
];
const MANIFEST = /^version = "([^"]+)"/m;
/** Pliki Cargo.lock (workspace i osobny workspace powłoki) z pakietem prowadzonym razem z programem.
 *  Audyt 2026-10-14 (A-17): lock powłoki został przy starej wersji i `--locked` odmawiał budowy. */
const LOCKS = [
  ['Cargo.lock', 'app-core', '.'],
  ['apps/desktop/src-tauri/Cargo.lock', 'alfa-desktop', 'apps/desktop/src-tauri/Cargo.toml'],
];
const lockVersion = (path, name) =>
  new RegExp(`^name = "${name}"\r?\nversion = "([^"]+)"`, 'm').exec(read(path))?.[1] ?? null;
const manifests = readdirSync(join(root, 'crates'))
  .map((name) => `crates/${name}/module.toml`)
  .filter((path) => existsSync(join(root, path)));

const versionIn = (path, re) => re.exec(read(path))?.[1] ?? null;
const current = versionIn(...FILES[0]);

/** Manifesty prowadzone razem z programem: te, których crate dziedziczy wersję z workspace'u. */
function followsWorkspace(manifestPath) {
  const cargo = join(root, dirname(manifestPath), 'Cargo.toml');
  return existsSync(cargo) && /^version\.workspace\s*=\s*true/m.test(readFileSync(cargo, 'utf8'));
}
/** Moduły z własną numeracją (np. 0.1.0) zostają przy swojej — ich testy nie porównują wersji. */
const tracked = manifests.filter((path) => {
  const v = versionIn(path, MANIFEST);
  return followsWorkspace(path) && (v === current || /^0\.0\.\d+$/.test(v ?? ''));
});

function check() {
  const problems = [];
  for (const [path, re] of FILES) {
    const v = versionIn(path, re);
    if (v !== current) problems.push(`${path}: ${v} (oczekiwano ${current})`);
  }
  for (const path of tracked) {
    const v = versionIn(path, MANIFEST);
    if (v !== current) problems.push(`${path}: ${v} (oczekiwano ${current})`);
  }
  for (const [path, name] of LOCKS) {
    const v = existsSync(join(root, path)) ? lockVersion(path, name) : null;
    if (v !== current)
      problems.push(`${path}: ${name} ${v} (oczekiwano ${current}; odśwież: cargo metadata)`);
  }
  if (!new RegExp(`^## ${current.replace(/\./g, '\\.')}(\\s|$)`, 'm').test(read('CHANGELOG.md')))
    problems.push(`CHANGELOG.md: brak wpisu „## ${current}”`);
  console.log(
    `set-version: ${current}; pliki: ${FILES.length}, manifesty modułów: ${tracked.length}; rozjazdy: ${problems.length}`,
  );
  for (const p of problems) console.log(`  ${p}`);
  if (problems.length) {
    console.log(`  Napraw: node scripts/set-version.mjs ${current}`);
    process.exit(1);
  }
}

function set(next) {
  if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(next)) {
    console.error(`set-version: „${next}” nie wygląda na numer wersji (np. 0.0.7)`);
    process.exit(2);
  }
  const replace = (path, re) => {
    const text = read(path);
    const match = re.exec(text);
    if (!match) throw new Error(`${path}: nie znaleziono numeru wersji`);
    const at = match.index + match[0].lastIndexOf(match[1]);
    writeFileSync(join(root, path), text.slice(0, at) + next + text.slice(at + match[1].length));
  };
  for (const [path, re] of FILES) replace(path, re);
  for (const path of tracked) replace(path, MANIFEST);
  for (const [path, , manifest] of LOCKS) {
    const args = ['metadata', '--format-version', '1', '--offline'];
    if (manifest !== '.') args.push('--manifest-path', manifest);
    const run = spawnSync('cargo', args, { cwd: root, stdio: ['ignore', 'ignore', 'pipe'] });
    if (run.status !== 0) console.error(`  ${path}: nie odświeżono (cargo metadata, kod ${run.status})`);
  }
  console.log(
    `set-version: ${current} → ${next} (${FILES.length} pliki, ${tracked.length} manifestów modułów)`,
  );
  console.log(
    '  Dalej: wpis w CHANGELOG.md, node scripts/docs-html.mjs (pliki Cargo.lock odświeżone)',
  );
}

const arg = process.argv[2];
if (!arg || arg === '--check') check();
else set(arg);
