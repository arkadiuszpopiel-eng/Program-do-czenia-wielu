#!/usr/bin/env node
// SBOM wydania (CRA, zadanie C2.10): spis składników programu w formacie CycloneDX 1.5 JSON.
// Rust — z `cargo metadata` powłoki (tylko zależności zwykłe i budowania, bez deweloperskich);
// interfejs — zależności produkcyjne `apps/desktop/ui` (te trafiają do paczki) wraz z przechodnimi.
// Własne crate'y i pakiety workspace są pomijane jako składniki zewnętrzne (opisuje je samo
// wydanie). Wynik: `target/sbom/alfa-<wersja>.cdx.json` i podsumowanie licencji.
//
//   node scripts/sbom.mjs [--check]
// `--check`: kod 1, gdy któryś składnik nie ma licencji albo ma licencję spoza listy `deny.toml`.
import { spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const check = process.argv.includes('--check');
const version = JSON.parse(
  readFileSync(join(root, 'apps/desktop/ui/package.json'), 'utf8'),
).version;

/** Lista dozwolonych licencji z `deny.toml` (sekcja `[licenses] allow`). */
function allowedLicenses() {
  const text = readFileSync(join(root, 'deny.toml'), 'utf8');
  const block = /allow = \[([\s\S]*?)\]/.exec(text)?.[1] ?? '';
  return new Set([...block.matchAll(/"([^"]+)"/g)].map((m) => m[1]));
}

/** Czy wyrażenie SPDX da się spełnić licencjami z listy (OR — wystarczy jedna gałąź, AND — wszystkie). */
function licenseOk(expr, allowed) {
  if (!expr) return false;
  const clean = expr.replace(/[()]/g, ' ').trim();
  return clean.split(/\s+OR\s+|\//).some((alt) =>
    alt
      .split(/\s+AND\s+/)
      .map((l) => l.trim())
      .every((l) => allowed.has(l)),
  );
}

function rustComponents() {
  const res = spawnSync(
    'cargo',
    [
      'metadata',
      '--format-version',
      '1',
      '--offline',
      '--manifest-path',
      join(root, 'apps/desktop/src-tauri/Cargo.toml'),
    ],
    { encoding: 'utf8', maxBuffer: 512 * 1024 * 1024 },
  );
  if (res.status !== 0) throw new Error(`cargo metadata: ${res.stderr.slice(-400)}`);
  const meta = JSON.parse(res.stdout);
  const byId = new Map(meta.packages.map((p) => [p.id, p]));
  const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
  const seen = new Set();
  const stack = [meta.resolve.root];
  while (stack.length) {
    const id = stack.pop();
    if (!id || seen.has(id)) continue;
    seen.add(id);
    for (const dep of nodes.get(id)?.deps ?? []) {
      const runtime = dep.dep_kinds.some((k) => k.kind === null || k.kind === 'build');
      if (runtime) stack.push(dep.pkg);
    }
  }
  return [...seen]
    .map((id) => byId.get(id))
    .filter((p) => p && p.source) // `source` = null: crate własny (ścieżka w workspace)
    .map((p) => ({
      type: 'library',
      name: p.name,
      version: p.version,
      purl: `pkg:cargo/${p.name}@${p.version}`,
      licenses: p.license ? [{ expression: p.license }] : [],
      ...(p.repository ? { externalReferences: [{ type: 'vcs', url: p.repository }] } : {}),
    }));
}

/** Licencja z pliku LICENSE, gdy `package.json` jej nie podaje (rozpoznanie po nagłówku). */
function licenseFromFile(dir) {
  for (const name of ['LICENSE', 'LICENSE.md', 'license', 'LICENSE.txt']) {
    const path = join(dir, name);
    if (!existsSync(path)) continue;
    const head = readFileSync(path, 'utf8').slice(0, 300);
    if (
      /^\s*MIT License/i.test(head) ||
      /Permission is hereby granted, free of charge/.test(head)
    ) {
      return 'MIT';
    }
    if (/Apache License[\s\S]*Version 2\.0/.test(head)) return 'Apache-2.0';
    if (/^\s*ISC License/i.test(head)) return 'ISC';
  }
  return undefined;
}

/** Licencje npm dopuszczone poza listą `deny.toml` (bardziej liberalne niż MIT). */
const NPM_EXTRA = ['0BSD'];

function npmComponents() {
  const uiDir = join(root, 'apps/desktop/ui');
  const pkg = JSON.parse(readFileSync(join(uiDir, 'package.json'), 'utf8'));
  const out = new Map();
  const visit = (name, fromDir) => {
    let pkgPath;
    try {
      pkgPath = createRequire(join(fromDir, 'package.json')).resolve(`${name}/package.json`);
    } catch {
      // Pakiet bez eksportu `package.json` — szukamy w drzewie `node_modules` w górę.
      for (let dir = fromDir; dir !== dirname(dir); dir = dirname(dir)) {
        const candidate = join(dir, 'node_modules', name, 'package.json');
        if (existsSync(candidate)) {
          pkgPath = candidate;
          break;
        }
      }
    }
    if (!pkgPath) return;
    const real = realpathSync(pkgPath);
    const data = JSON.parse(readFileSync(real, 'utf8'));
    const key = `${data.name}@${data.version}`;
    if (out.has(key)) return;
    const own = String(data.name).startsWith('@alfa/');
    if (!own) {
      const license =
        (typeof data.license === 'string' ? data.license : data.license?.type) ??
        licenseFromFile(dirname(real));
      out.set(key, {
        type: 'library',
        name: data.name,
        version: data.version,
        purl: `pkg:npm/${String(data.name).replace('@', '%40')}@${data.version}`,
        licenses: license ? [{ expression: license }] : [],
      });
    }
    for (const dep of Object.keys(data.dependencies ?? {})) visit(dep, dirname(real));
    if (own) out.delete(key);
  };
  for (const dep of Object.keys(pkg.dependencies ?? {})) visit(dep, uiDir);
  return [...out.values()];
}

const allowed = new Set([...allowedLicenses(), ...NPM_EXTRA]);
const rust = rustComponents();
const npm = npmComponents();
const components = [...rust, ...npm].sort((a, b) => a.purl.localeCompare(b.purl));
const bom = {
  bomFormat: 'CycloneDX',
  specVersion: '1.5',
  serialNumber: `urn:uuid:${randomUUID()}`,
  version: 1,
  metadata: {
    timestamp: new Date().toISOString(),
    tools: { components: [{ type: 'application', name: 'alfa-sbom', version: '1' }] },
    component: { type: 'application', name: 'Alfa', version, 'bom-ref': 'alfa' },
  },
  components,
};
const outDir = join(root, 'target', 'sbom');
mkdirSync(outDir, { recursive: true });
const file = join(outDir, `alfa-${version}.cdx.json`);
writeFileSync(file, `${JSON.stringify(bom, null, 2)}\n`);

const tally = new Map();
const problems = [];
for (const c of components) {
  const expr = c.licenses[0]?.expression ?? '';
  tally.set(expr || '(brak)', (tally.get(expr || '(brak)') ?? 0) + 1);
  // Wyrażenia nieobjęte listą sprawdza `cargo deny` (wyjątki i progi pewności) — tu tylko brak licencji
  // i licencje npm, których `cargo deny` nie widzi.
  if (!expr || (c.purl.startsWith('pkg:npm') && !licenseOk(expr, allowed))) {
    problems.push(`${c.purl}: ${expr || 'brak licencji'}`);
  }
}
const top = [...tally.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8);
console.log(
  `sbom: ${components.length} składników (Rust ${rust.length}, interfejs ${npm.length}) → ${file}`,
);
console.log(`licencje: ${top.map(([l, n]) => `${l} ×${n}`).join(', ')}`);
if (problems.length) {
  console.log(`do sprawdzenia (${problems.length}):\n  ${problems.join('\n  ')}`);
  if (check) process.exit(1);
}
