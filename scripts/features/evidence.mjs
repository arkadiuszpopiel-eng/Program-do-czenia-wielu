// Ślady testów funkcji (docs/plan/45-testy-ai.md §3). Dla każdej funkcji szukamy jej identyfikatora
// (nazwa polecenia, klucz ustawienia, nazwa narzędzia…) w kodzie testów pięciu poziomów:
//
//   rdzen       — testy Rust (`crates/*/tests`, `src/**/tests*`, moduły `#[cfg(test)]`)
//   interfejs   — testy jednostkowe interfejsu (vitest: `__tests__`, `*.test.ts`)
//   przegladarka — Playwright na atrapie (`apps/desktop/ui/e2e`)
//   aplikacja   — Playwright w prawdziwym oknie (`apps/desktop/ui/e2e-app`) i ślad wywołań
//                 rdzenia z ostatniego biegu (`ipc-seen.txt`)
//   eval        — zamrożone zestawy ocen (`evals/**/*.json`)
//   przeglad    — przegląd ogólny: test, który przechodzi po WSZYSTKICH pozycjach katalogu danego
//                 rodzaju albo grupy (nagłówek `// katalog-przeglad: akcja/paleta, strona`) — obejmuje też funkcje
//                 dodane później, bez dopisywania testu
//
// Ślad to wystąpienie identyfikatora w teście — dowód, że test dotyka funkcji, nie że sprawdza
// wszystko. Ślad z biegu prawdziwej aplikacji (`ipc-seen.txt`) zależy od maszyny, więc trafia
// tylko do raportu (pole `bieg`), nie do bramki.
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';

export const LEVELS = ['rdzen', 'interfejs', 'przegladarka', 'aplikacja', 'eval', 'przeglad'];

const SKIP = new Set(['target', 'node_modules', 'dist', '.git', 'test-results', 'out']);

function* walk(dir) {
  if (!existsSync(dir)) return;
  for (const name of readdirSync(dir)) {
    if (SKIP.has(name)) continue;
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* walk(path);
    else yield path;
  }
}

/**
 * Testy-spisy: wyliczają wszystkie polecenia albo typy (zgodność kształtu DTO, sygnatury łącza,
 * zapisane odpowiedzi) — nie sprawdzają działania funkcji, więc nie są jej śladem.
 */
const LISTINGS = [
  /\/tests\/dto_spec\//,
  /\/tests\/dto_roundtrip\.rs$/,
  /\/tests\/ipc_signatures\.rs$/,
  /\/tests\/fixtures\//,
  /\/tests\/feature_catalog\.rs$/,
];

/** Poziom pliku testu albo `null` (plik produkcyjny bez testów albo test-spis). */
function levelOf(rel, text) {
  const p = rel.split(sep).join('/');
  if (LISTINGS.some((re) => re.test(p))) return null;
  if (p.startsWith('evals/') && p.endsWith('.json')) return { level: 'eval', text };
  if (p.startsWith('apps/desktop/ui/e2e-app/')) return { level: 'aplikacja', text };
  if (p.startsWith('apps/desktop/ui/e2e/')) return { level: 'przegladarka', text };
  if (/\.(ts|svelte\.ts)$/.test(p) && (/\/__tests__\//.test(p) || /\.test\.ts$/.test(p))) {
    return { level: 'interfejs', text };
  }
  if (p.endsWith('.rs')) {
    if (/\/tests\//.test(p) || /\/tests?\.rs$/.test(p) || /\/benches\//.test(p)) {
      return { level: 'rdzen', text };
    }
    const marker = text.indexOf('#[cfg(test)]');
    if (marker >= 0) return { level: 'rdzen', text: text.slice(marker) };
  }
  return null;
}

/** Słowa kodu: całe ciągi z kropkami (`general.start_with_system`) i ich części (`sessions_list`). */
function addTokens(set, text) {
  for (const m of text.matchAll(/[A-Za-z_][A-Za-z0-9_]*(?:[.:-][A-Za-z0-9_]+)*/g)) {
    const token = m[0];
    set.add(token);
    if (/[.:-]/.test(token)) for (const part of token.split(/[.:-]/)) set.add(part);
  }
}

/**
 * Indeks śladów: dla każdego poziomu zbiór słów i surowy tekst (etykiety stron Ustawień są
 * szukane jako napis — testy otwierają strony po nazwie widocznej dla użytkownika).
 */
export function collectEvidence(root, runtime = {}) {
  const index = Object.fromEntries(LEVELS.map((l) => [l, { tokens: new Set(), text: [] }]));
  index.swept = new Set();
  const dirs = ['crates', 'apps/desktop/src-tauri', 'apps/desktop/ui', 'packages/ui-kit', 'evals'];
  for (const dir of dirs) {
    for (const file of walk(join(root, dir))) {
      if (!/\.(rs|ts|json)$/.test(file)) continue;
      const rel = relative(root, file);
      const hit = levelOf(rel, readFileSync(file, 'utf8'));
      if (!hit) continue;
      const sweep = /katalog-przeglad:\s*([a-z/, ]+)/.exec(hit.text);
      if (sweep) for (const kind of sweep[1].split(',')) index.swept.add(kind.trim());
      addTokens(index[hit.level].tokens, hit.text);
      if (hit.level !== 'rdzen' && hit.level !== 'eval') index[hit.level].text.push(hit.text);
    }
  }
  for (const level of LEVELS) index[level].text = index[level].text.join('\n');
  // Ślad z biegu prawdziwej aplikacji: polecenia rdzenia wywołane przez okno (tylko raport).
  index.runtime = new Set(runtime.ipcSeen ?? []);
  return index;
}

/** Poziomy, na których test dotyka funkcji. */
export function levelsFor(feature, index) {
  const found = [];
  for (const level of LEVELS) {
    if (level === 'przeglad') {
      const group = feature.sweep ? `${feature.kind}/${feature.sweep}` : null;
      if (index.swept.has(feature.kind) || (group && index.swept.has(group))) found.push(level);
      continue;
    }
    const { tokens, text } = index[level];
    const byLabel =
      feature.kind === 'strona' && feature.label && text.includes(`'${feature.label}'`);
    const byId = feature.kind !== 'strona' && tokens.has(feature.id);
    const byPage = feature.kind === 'strona' && tokens.has(`page:${feature.id}`);
    if (byId || byLabel || byPage) found.push(level);
  }
  return found;
}

/** Czy funkcję wywołano w ostatnim biegu prawdziwej aplikacji. */
export function seenInRun(feature, index) {
  return feature.kind === 'polecenie' && index.runtime.has(feature.id);
}

/** Ślad z ostatniego biegu prawdziwej aplikacji (`target/verify/app/out/ipc-seen.txt`). */
export function readRuntime(root) {
  const read = (rel) => {
    const file = join(root, rel);
    return existsSync(file)
      ? readFileSync(file, 'utf8')
          .split(/\r?\n/)
          .map((l) => l.trim())
          .filter(Boolean)
      : [];
  };
  return { ipcSeen: read('target/verify/app/out/ipc-seen.txt') };
}
