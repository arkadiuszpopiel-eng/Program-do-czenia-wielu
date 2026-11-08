#!/usr/bin/env node
// Bramka rozmiaru (AGENTS.md: plik ≤ 400 linii, crate ≤ 8 000 linii; audyt 2026-10-14, A-16).
// Pliki śledzone przez Git: Rust, TypeScript, Svelte, skrypty. Pliki generowane są pomijane.
// Wyjątki są jawne (`scripts/size-exceptions.json`: ścieżka → najwięcej linii i powód) i nie
// mogą rosnąć; wyjątek, który już mieści się w limicie, też jest błędem (do usunięcia z listy).
// Kod wyjścia 1 przy każdym naruszeniu.
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const FILE_LIMIT = 400;
const CRATE_LIMIT = 8000;
const GENERATED = [/\/generated\//, /\.d\.ts$/, /^docs\//, /^packages\/schemas\//];

const exceptions = JSON.parse(readFileSync(join(root, 'scripts', 'size-exceptions.json'), 'utf8'));
const files = execFileSync('git', ['ls-files', '*.rs', '*.ts', '*.svelte', '*.mjs'], {
  cwd: root,
  encoding: 'utf8',
  maxBuffer: 64 * 1024 * 1024,
})
  .split('\n')
  .filter(Boolean)
  .filter((f) => !GENERATED.some((re) => re.test(f)));

const lines = (path) => {
  const text = readFileSync(join(root, path), 'utf8');
  if (!text) return 0;
  return text.split('\n').length - (text.endsWith('\n') ? 1 : 0);
};

const problems = [];
const crates = new Map();
for (const file of files) {
  let n;
  try {
    n = lines(file);
  } catch {
    continue; // usunięty w kopii roboczej, a jeszcze w indeksie
  }
  const crate = /^crates\/([^/]+)\/src\//.exec(file)?.[1];
  if (crate) crates.set(crate, (crates.get(crate) ?? 0) + n);
  const allowed = exceptions.files[file];
  if (allowed) {
    if (n > allowed.max)
      problems.push(
        `${file}: ${n} linii — wyjątek pozwala najwyżej ${allowed.max} (${allowed.reason})`,
      );
    else if (n <= FILE_LIMIT)
      problems.push(
        `${file}: ${n} linii — mieści się w limicie, usuń wyjątek z size-exceptions.json`,
      );
  } else if (n > FILE_LIMIT) {
    problems.push(`${file}: ${n} linii (limit ${FILE_LIMIT}) — podziel plik`);
  }
}
for (const [crate, n] of crates) {
  if (n > CRATE_LIMIT)
    problems.push(`crates/${crate}/src: ${n} linii (limit crate'a ${CRATE_LIMIT})`);
}
for (const file of Object.keys(exceptions.files)) {
  if (!files.includes(file)) problems.push(`${file}: wyjątek dla pliku, którego nie ma — usuń go`);
}
console.log(
  `check-size: ${files.length} plików, ${crates.size} crate'ów, wyjątków ${Object.keys(exceptions.files).length}; naruszenia: ${problems.length}`,
);
for (const p of problems) console.log(`  ${p}`);
process.exit(problems.length ? 1 : 0);
