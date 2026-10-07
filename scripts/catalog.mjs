#!/usr/bin/env node
// Generuje docs/CATALOG.md: każdy crate i moduł Alfy z grupą, falą, rodzajem, rozmiarem,
// liczbą testów i odsyłaczem do SPEC, plus lista niespójności (SPEC bez crate'a, moduł bez
// SPEC, brak README, przekroczone limity z AGENTS.md). Źródła: crates/*, crates/*/module.toml,
// indeks w docs/modules/README.md. Uruchomienie: `node scripts/catalog.mjs`.
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const CRATES = join(ROOT, 'crates');
const OUT = join(ROOT, 'docs', 'CATALOG.md');
const FILE_LIMIT = 400;
const CRATE_LIMIT = 8000;
const ROLES = ['contract', 'impl', 'fake'];

/** Grupy dla crate'ów, które nie są modułem ze SPEC (kolejność = kolejność w katalogu). */
const EXTRA_GROUPS = [
  { title: 'Kompozycja aplikacji (`app-*`)', test: (b) => b.startsWith('app-') },
  { title: 'Biblioteki wspólne (`lib-*`)', test: (b) => b.startsWith('lib-') },
  { title: 'Wzorce i spike’i', test: (b) => b.startsWith('example-') || b.startsWith('spike-') },
  { title: 'Pozostałe', test: () => true },
];

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    if (name === 'target' || name === 'node_modules') continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* walk(p);
    else yield p;
  }
}

const tomlValue = (text, key) => new RegExp(`^${key}\\s*=\\s*"([^"]*)"`, 'm').exec(text)?.[1] ?? '';

/** Indeks SPEC-ów: sekcje `### Grupa` i wiersze `| [`id`](…) | fala | opis |`. */
function readSpecIndex() {
  const text = readFileSync(join(ROOT, 'docs', 'modules', 'README.md'), 'utf8');
  const specs = new Map();
  const groups = [];
  let group = '';
  for (const line of text.split('\n')) {
    const heading = /^### (.+)$/.exec(line);
    if (heading) {
      group = heading[1].trim();
      groups.push(group);
      continue;
    }
    const row = /^\| \[`([^`]+)`\]\([^)]*\)\s*\|([^|]*)\|(.*)\|\s*$/.exec(line);
    if (row) specs.set(row[1], { group, wave: row[2].trim(), about: row[3].trim() });
  }
  return { specs, groups };
}

function readCrate(name) {
  const dir = join(CRATES, name);
  const role = ROLES.find((r) => name.endsWith(`-${r}`)) ?? 'single';
  const base = role === 'single' ? name : name.slice(0, -role.length - 1);
  let lines = 0;
  let srcLines = 0;
  let tests = 0;
  const long = [];
  for (const file of walk(dir)) {
    if (!file.endsWith('.rs')) continue;
    const src = readFileSync(file, 'utf8');
    const n = (src.match(/\n/g) ?? []).length;
    lines += n;
    if (relative(dir, file).startsWith('src')) srcLines += n;
    tests += (src.match(/#\[(?:tokio::)?test\b/g) ?? []).length;
    if (n > FILE_LIMIT) long.push({ file: relative(ROOT, file).replaceAll('\\', '/'), lines: n });
  }
  const manifestPath = join(dir, 'module.toml');
  const manifest = existsSync(manifestPath) ? readFileSync(manifestPath, 'utf8') : '';
  return {
    name,
    role,
    base,
    lines,
    srcLines,
    tests,
    long,
    readme: existsSync(join(dir, 'README.md')),
    manifest: manifest
      ? {
          id: tomlValue(manifest, 'id'),
          kind: tomlValue(manifest, 'kind'),
          isolation: tomlValue(manifest, 'isolation'),
          lifecycle: tomlValue(manifest, 'lifecycle'),
        }
      : null,
  };
}

/** SPEC modułu: dokładna nazwa, a gdy jej nie ma — najdłuższy SPEC będący prefiksem nazwy. */
function specFor(base, specs) {
  if (specs.has(base)) return base;
  const prefixes = [...specs.keys()].filter((id) => base.startsWith(`${id}-`));
  return prefixes.sort((a, b) => b.length - a.length)[0] ?? '';
}

const { specs, groups } = readSpecIndex();
const crates = readdirSync(CRATES)
  .filter((n) => existsSync(join(CRATES, n, 'Cargo.toml')))
  .sort()
  .map(readCrate);

// Moduł = wspólna baza nazwy (`router` dla router-contract/-impl/-fake).
const modules = new Map();
for (const c of crates) {
  const m = modules.get(c.base) ?? { base: c.base, crates: [] };
  m.crates.push(c);
  modules.set(c.base, m);
}
for (const m of modules.values()) {
  m.spec = specFor(m.base, specs);
  m.manifest = m.crates.find((c) => c.manifest)?.manifest ?? null;
  m.lines = m.crates.reduce((s, c) => s + c.lines, 0);
  m.tests = m.crates.reduce((s, c) => s + c.tests, 0);
  // Kontrakt dzielony przez kilka modułów (np. `providers` dla providers-api i providers-local).
  m.sharedBy = m.spec ? [] : [...specs.keys()].filter((id) => id.startsWith(`${m.base}-`));
  const home = m.spec || m.sharedBy[0];
  m.group = home ? specs.get(home).group : EXTRA_GROUPS.find((g) => g.test(m.base)).title;
}

const num = (n) => n.toLocaleString('pl-PL').replaceAll(' ', ' ');
const parts = (m) =>
  m.crates.every((c) => c.role === 'single')
    ? 'pojedynczy'
    : ROLES.filter((r) => m.crates.some((c) => c.role === r))
        .map((r) => r[0].toUpperCase())
        .join(' · ');
const specCell = (m) => {
  if (m.spec) return `[${m.spec}](modules/${m.spec}/SPEC.md)`;
  if (m.sharedBy.length) return `kontrakt wspólny: ${m.sharedBy.join(', ')}`;
  return '—';
};

const out = [];
const totalLines = crates.reduce((s, c) => s + c.lines, 0);
const totalTests = crates.reduce((s, c) => s + c.tests, 0);
out.push(
  '# Katalog modułów i crate’ów Alfy',
  '',
  '<!-- Plik generowany: `node scripts/catalog.mjs`. Nie edytuj ręcznie. -->',
  '',
  'Jedno miejsce, w którym widać cały kod Rust: do jakiej grupy należy moduł, w której fali powstał, z jakich',
  'crate’ów się składa, jak jest duży i gdzie leży jego specyfikacja. Grupy i fale pochodzą z indeksu',
  '[`docs/modules/README.md`](modules/README.md), rodzaj i izolacja z `module.toml`, liczby z kodu.',
  '',
  '| Miara | Wartość |',
  '| --- | --- |',
  `| Crate’y | ${crates.length} |`,
  `| Moduły (wspólna baza nazwy) | ${modules.size} |`,
  `| Specyfikacje w indeksie | ${specs.size} |`,
  `| Manifesty \`module.toml\` | ${crates.filter((c) => c.manifest).length} |`,
  `| Linie Rust (z testami) | ${num(totalLines)} |`,
  `| Testy (\`#[test]\`, \`#[tokio::test]\`) | ${num(totalTests)} |`,
  '',
  'Kolumna „Crate’y”: C — `-contract`, I — `-impl`, F — `-fake`; „pojedynczy” — crate bez trójki.',
  'Stan kryteriów akceptacji: [`STATUS.md`](STATUS.md). Zasady zależności między crate’ami: [`crates/README.md`](../crates/README.md).',
  '',
);

const order = [...groups, ...EXTRA_GROUPS.map((g) => g.title)];
for (const group of order) {
  const rows = [...modules.values()].filter((m) => m.group === group);
  if (!rows.length) continue;
  rows.sort((a, b) => a.base.localeCompare(b.base));
  out.push(
    `## ${group}`,
    '',
    '| Moduł | Fala | Rodzaj | Izolacja | Crate’y | Linie | Testy | SPEC | Opis |',
    '| --- | --- | --- | --- | --- | ---: | ---: | --- | --- |',
  );
  for (const m of rows) {
    const spec = specs.get(m.spec);
    out.push(
      `| \`${m.base}\` | ${spec?.wave || '—'} | ${m.manifest?.kind || '—'} | ${m.manifest?.isolation || '—'} | ${parts(m)} | ${num(m.lines)} | ${num(m.tests)} | ${specCell(m)} | ${spec && m.spec === m.base ? spec.about : ''} |`,
    );
  }
  out.push('');
}

// Niespójności — to jest lista zadań porządkowych, nie błędy builda.
const specWithoutCrate = [...specs.keys()].filter(
  (id) => ![...modules.values()].some((m) => m.spec === id || m.sharedBy.includes(id)),
);
const moduleWithoutSpec = [...modules.values()]
  .filter((m) => !m.spec && !m.sharedBy.length && m.crates.some((c) => c.role !== 'single'))
  .map((m) => m.base);
const implWithoutManifest = crates
  .filter((c) => c.role === 'impl' && !c.manifest)
  .map((c) => c.name);
const withoutReadme = crates.filter((c) => !c.readme).map((c) => c.name);
const longFiles = crates.flatMap((c) => c.long).sort((a, b) => b.lines - a.lines);
const bigCrates = crates.filter((c) => c.srcLines > CRATE_LIMIT);
const list = (items) => (items.length ? items.map((i) => `\`${i}\``).join(', ') : 'brak');

out.push(
  '## Niespójności do uporządkowania',
  '',
  '| Reguła | Liczba | Pozycje |',
  '| --- | ---: | --- |',
  `| SPEC w indeksie bez crate’a w \`crates/\` (moduły UI i powłoki są poza \`crates/\`) | ${specWithoutCrate.length} | ${list(specWithoutCrate)} |`,
  `| Moduł z trójką crate’ów bez SPEC | ${moduleWithoutSpec.length} | ${list(moduleWithoutSpec)} |`,
  `| Crate \`-impl\` bez \`module.toml\` | ${implWithoutManifest.length} | ${list(implWithoutManifest)} |`,
  `| Crate bez \`README.md\` | ${withoutReadme.length} | ${list(withoutReadme)} |`,
  `| Plik Rust powyżej ${FILE_LIMIT} linii | ${longFiles.length} | ${longFiles.map((f) => `\`${f.file}\` (${f.lines})`).join(', ') || 'brak'} |`,
  `| Crate powyżej ${num(CRATE_LIMIT)} linii w \`src/\` | ${bigCrates.length} | ${bigCrates.map((c) => `\`${c.name}\` (${num(c.srcLines)})`).join(', ') || 'brak'} |`,
  '',
);

writeFileSync(OUT, out.join('\n'));
console.log(
  `catalog: ${crates.length} crate'ów, ${modules.size} modułów, ${specs.size} SPEC-ów → docs/CATALOG.md`,
);
