#!/usr/bin/env node
// Katalog funkcji i bramka testów (docs/plan/45-testy-ai.md). Wylicza każdą funkcję programu
// z kodu (scripts/features/sources.mjs), szuka jej śladów w testach (scripts/features/evidence.mjs)
// i pilnuje zasady: NOWA funkcja bez testu = błąd. Funkcje bez testu sprzed wprowadzenia bramki
// są spisane w `docs/features/dlug.json` — ta lista może tylko maleć.
//
//   node scripts/features.mjs                 bramka + docs/features/KATALOG.md + target/features/katalog.json
//   node scripts/features.mjs --sprawdz       sama bramka (bez zapisu plików; pre-commit, verify szybki)
//   node scripts/features.mjs --kolejka [N]   N funkcji bez testu — kolejka pracy sesji testującej
//   node scripts/features.mjs --przytnij      usuwa z długu funkcje, które mają już test
//
// Dopisanie funkcji do długu (zgoda na funkcję bez testu) to decyzja właściciela — skrypt tego
// nie robi; wyjątek: pierwsze utworzenie pliku (`--zalozenie-dlugu`, gdy pliku nie ma).
// Kod wyjścia: 0 — zielono, 1 — nowa funkcja bez testu albo brak spisu narzędzi, 2 — użycie.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  collectEvidence,
  LEVELS,
  levelsFor,
  readRuntime,
  seenInRun,
} from './features/evidence.mjs';
import { collectFeatures, KINDS } from './features/sources.mjs';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const has = (flag) => args.includes(flag);
const DEBT = join(root, 'docs/features/dlug.json');
const CATALOG_MD = join(root, 'docs/features/KATALOG.md');
const CATALOG_JSON = join(root, 'target/features/katalog.json');

/** Poziomy, które zaliczają funkcję danego rodzaju (wystarczy jeden). */
const REQUIRED = {
  // Polecenia rdzenia i narzędzia agentek mają skutki poza oknem — wymagają testu działania;
  // przegląd ogólny (sama odpowiedź bez błędu) tu nie wystarcza.
  polecenie: ['rdzen', 'aplikacja'],
  akcja: ['interfejs', 'przegladarka', 'aplikacja', 'przeglad'],
  strona: ['przegladarka', 'aplikacja', 'przeglad'],
  ustawienie: LEVELS,
  narzedzie: ['rdzen', 'eval'],
  zdarzenie: ['rdzen', 'interfejs', 'przegladarka', 'aplikacja'],
};

const LEVEL_NAMES = {
  rdzen: 'rdzeń',
  interfejs: 'interfejs',
  przegladarka: 'przeglądarka',
  aplikacja: 'aplikacja',
  eval: 'eval',
  przeglad: 'przegląd ogólny',
};

const key = (f) => `${f.kind}:${f.id}`;

function readDebt() {
  if (!existsSync(DEBT)) return null;
  return new Set(JSON.parse(readFileSync(DEBT, 'utf8')).funkcje);
}

function writeDebt(keys) {
  const body = {
    opis:
      'Funkcje bez testu sprzed bramki katalogu (docs/plan/45-testy-ai.md). Lista tylko maleje: ' +
      '`node scripts/features.mjs --przytnij` usuwa pozycje z testem; dopisanie = decyzja właściciela.',
    funkcje: [...keys].sort(),
  };
  writeFileSync(DEBT, `${JSON.stringify(body, null, 2)}\n`);
}

function analyse(features, index) {
  return features.map((f) => {
    const levels = levelsFor(f, index);
    const tested = levels.some((l) => REQUIRED[f.kind].includes(l));
    return { ...f, levels, tested, run: seenInRun(f, index) };
  });
}

function table(rows) {
  const lines = [];
  for (const [kind, title] of Object.entries(KINDS)) {
    const own = rows.filter((r) => r.kind === kind);
    if (own.length === 0) continue;
    const tested = own.filter((r) => r.tested).length;
    const per = LEVELS.map((l) => own.filter((r) => r.levels.includes(l)).length);
    const pct = Math.round((tested / own.length) * 1000) / 10;
    lines.push(`| ${title} | ${own.length} | ${tested} (${pct} %) | ${per.join(' | ')} |`);
  }
  return lines;
}

function markdown(rows, debt) {
  const total = rows.length;
  const tested = rows.filter((r) => r.tested).length;
  const out = [
    '# Katalog funkcji i testów',
    '',
    'Plik generowany: `node scripts/features.mjs` (nie edytuj ręcznie). Zasady i system testów:',
    '[45-testy-ai.md](../plan/45-testy-ai.md). Ślad = identyfikator funkcji w kodzie testu danego poziomu;',
    '„bieg aplikacji” = polecenie wywołane w ostatnim biegu prawdziwej aplikacji na tej maszynie (tylko informacja).',
    '',
    `**Razem: ${total} funkcji, z testem ${tested} (${Math.round((tested / total) * 1000) / 10} %), w długu ${debt.size}.**`,
    '',
    `| Rodzaj | Funkcji | Z testem | ${LEVELS.map((l) => LEVEL_NAMES[l]).join(' | ')} |`,
    `| --- | ---: | ---: | ${LEVELS.map(() => '---:').join(' | ')} |`,
    ...table(rows),
    '',
  ];
  for (const [kind, title] of Object.entries(KINDS)) {
    const own = rows.filter((r) => r.kind === kind);
    if (own.length === 0) continue;
    out.push(`## ${title}`, '', '| Funkcja | Testy | Uwagi |', '| --- | --- | --- |');
    for (const r of own) {
      const names = r.levels.map((l) => LEVEL_NAMES[l]);
      if (r.run) names.push('bieg aplikacji');
      const levels = names.join(', ') || '—';
      const notes = [
        r.label,
        r.note,
        r.soon ? 'wkrótce' : null,
        r.tested ? null : debt.has(key(r)) ? '**dług**' : '**BRAK TESTU**',
      ].filter(Boolean);
      out.push(`| \`${r.id}\` | ${levels} | ${notes.join('; ')} |`);
    }
    out.push('');
  }
  return `${out.join('\n')}\n`;
}

/** Gdzie dopisać test funkcji danego rodzaju — wskazówka dla sesji testującej. */
const HINT = {
  polecenie:
    'test rdzenia w crates/app-core/tests (wywołanie `core.<polecenie>`), potem scenariusz e2e-app',
  akcja: 'test w apps/desktop/ui/e2e (paleta albo skrót) lub vitest w src/lib/**/__tests__',
  strona: 'test w apps/desktop/ui/e2e otwierający stronę po nazwie, ze sprawdzeniem dostępności',
  ustawienie: 'test zmiany ustawienia: settings_set + skutek w rdzeniu albo w interfejsie',
  narzedzie: 'test narzędzia w crates/<moduł>-impl/tests albo zadanie w evals (za zgodą)',
  zdarzenie: 'test rdzenia, który oczekuje zdarzenia, albo test interfejsu, który je obsługuje',
};

async function main() {
  const features = await collectFeatures(root);
  if (!features.some((f) => f.kind === 'narzedzie')) {
    console.error(
      'Brak docs/features/narzedzia.txt — ALFA_UPDATE_FEATURES=1 cargo test -p app-core --test feature_catalog',
    );
    process.exit(1);
  }
  const rows = analyse(features, collectEvidence(root, readRuntime(root)));
  let debt = readDebt();
  if (debt === null) {
    if (!has('--zalozenie-dlugu')) {
      console.error('Brak docs/features/dlug.json — pierwsze założenie: --zalozenie-dlugu');
      process.exit(2);
    }
    debt = new Set(rows.filter((r) => !r.tested && !r.soon).map(key));
    writeDebt(debt);
    console.log(`Założono dług: ${debt.size} funkcji bez testu.`);
  }
  const untested = rows.filter((r) => !r.tested && !r.soon);
  const fresh = untested.filter((r) => !debt.has(key(r)));
  const repaid = rows.filter((r) => r.tested && debt.has(key(r)));
  const gone = [...debt].filter((k) => !rows.some((r) => key(r) === k));

  if (has('--przytnij')) {
    for (const r of repaid) debt.delete(key(r));
    for (const k of gone) debt.delete(k);
    writeDebt(debt);
    console.log(`Dług: usunięto ${repaid.length + gone.length}, zostało ${debt.size}.`);
  }

  if (has('--kolejka')) {
    const n = Number(args[args.indexOf('--kolejka') + 1]) || 20;
    const queue = [...fresh, ...untested.filter((r) => debt.has(key(r)))].slice(0, n);
    for (const r of queue) {
      console.log(`${r.kind}\t${r.id}${r.label ? ` (${r.label})` : ''}\t→ ${HINT[r.kind]}`);
    }
    return;
  }

  const total = rows.length;
  const tested = rows.filter((r) => r.tested).length;
  console.log(
    `Katalog funkcji: ${total}, z testem ${tested} (${Math.round((tested / total) * 1000) / 10} %), dług ${debt.size}`,
  );
  if (repaid.length > 0 && !has('--przytnij')) {
    console.log(
      `Do usunięcia z długu (mają już test): ${repaid.length} — node scripts/features.mjs --przytnij`,
    );
  }
  if (!has('--sprawdz')) {
    writeFileSync(CATALOG_MD, markdown(rows, debt));
    mkdirSync(dirname(CATALOG_JSON), { recursive: true });
    writeFileSync(
      CATALOG_JSON,
      `${JSON.stringify({ total, tested, debt: debt.size, rows }, null, 2)}\n`,
    );
  }
  if (fresh.length > 0) {
    console.error(
      `\nNOWE FUNKCJE BEZ TESTU (${fresh.length}) — każda nowa funkcja potrzebuje testu:`,
    );
    for (const r of fresh) console.error(`  ${r.kind} \`${r.id}\` → ${HINT[r.kind]}`);
    process.exit(1);
  }
}

await main();
