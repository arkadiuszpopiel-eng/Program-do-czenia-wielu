// Ocena raportu agentki z zadania w tle (docs/plan/24 §2.6):
//  • `judge` — czy raport trzyma się danych: każda liczba (kwota, data, godzina) i każda nazwa
//    w cudzysłowie występuje w danych, które program podał agentce;
//  • `formProblems` — czy to w ogóle raport: bez odgrywania ról obsady, bez pytań do nieobecnego
//    użytkownika, nie pusty.
// Numery punktów listy („1.”, „2)”) nie są liczbami z danych.
//
//   import { judge, formProblems } from './grounding.mjs'
//   node scripts/trial/grounding.mjs --selftest

/** Liczby z tekstu w postaci znormalizowanej: `1 234,50` → `1234.50`, `08:00` zostaje. */
export function numbersOf(text) {
  const cleaned = text
    .split(/\r?\n/)
    .map((line) => line.replace(/^\s*(?:[-*•]\s*)?\d{1,2}[.)]\s+/, ''))
    .join('\n');
  const found = cleaned.match(/\d+(?:[ \u00a0]\d{3})*(?:[.,:-]\d+)*/g) ?? [];
  return found.map((n) => n.replace(/[ \u00a0]/g, '').replace(/,/g, '.'));
}

/** Nazwy w cudzysłowach („…”, "…", «…»). */
export function namesOf(text) {
  const found = [...text.matchAll(/[„"«]([^„”"«»\n]{2,90})[”"»]/g)].map((m) => m[1].trim());
  return [...new Set(found)];
}

const fold = (s) => s.toLowerCase().replace(/\s+/g, ' ').trim();

/**
 * `facts` — teksty, które agentka dostała (arkusz faktów, polecenie). Zwraca listę wyjść poza dane.
 * Liczba jest dozwolona także w innym zapisie (`0` wobec `0.00`); mała liczba całkowita może być
 * policzeniem pozycji z danych („4 razy”) — do `maxCount`.
 */
export function judge(report, facts, maxCount = 0) {
  const source = facts.join('\n');
  const allowed = new Set(numbersOf(source));
  const allowedValues = new Set([...allowed].map(Number).filter(Number.isFinite));
  const sourceFolded = fold(source);
  const problems = [];
  for (const number of numbersOf(report)) {
    const value = Number(number);
    const counted = Number.isInteger(value) && value >= 0 && value <= maxCount;
    const known = allowed.has(number) || (Number.isFinite(value) && allowedValues.has(value));
    if (!counted && !known) problems.push(`liczba spoza danych: ${number}`);
  }
  for (const name of namesOf(report)) {
    if (!sourceFolded.includes(fold(name))) problems.push(`nazwa spoza danych: „${name}”`);
  }
  return [...new Set(problems)];
}

/** Ślady odgrywania obsady i rozmowy zamiast raportu. */
const ROLE_PLAY = [
  /(Dyryg[a-ząćęłńóśźż]*tka|Mówczyni|Krytyczka|Przewodnicząca)\s*\**\s*:/gi,
  /Przekazuj[ęe]\s+[A-ZĄĆĘŁŃÓŚŹŻ][a-ząćęłńóśźż]+/g,
  /Przekazałam\s+[A-ZĄĆĘŁŃÓŚŹŻ][a-ząćęłńóśźż]+/g,
  /Czy (mogę|potrzebujesz)[^?\n]*\?/gi,
];

/**
 * Wady formy raportu: odgrywanie ról obsady, pytania do nieobecnego użytkownika, raport pusty
 * albo urwany. Zadanie w tle ma zostawić gotowy raport, nie rozmowę.
 */
export function formProblems(report) {
  const problems = [];
  const text = report.trim();
  if (text.length < 40) problems.push('raport pusty albo urwany');
  const hits = ROLE_PLAY.flatMap((pattern) => text.match(pattern) ?? []).map((m) =>
    m.replace(/[*\s]+/g, ' ').trim(),
  );
  const roles = [...new Set(hits)];
  if (roles.length > 0) {
    problems.push(`odgrywanie ról albo rozmowa zamiast raportu: ${roles.slice(0, 4).join(' | ')}`);
  }
  // Odmowa zamiast raportu (Bielik po przypomnieniu o urwanej odpowiedzi, 1.11).
  if (/nie mogę wykonać|nie mam uprawnień/i.test(text)) problems.push('odmowa zamiast raportu');
  // Podwójnie zakodowany koniec wiersza („\n” jako dwa znaki) i wymyślone znaczniki XML.
  if (/\\n/.test(text)) problems.push('dosłowne „\\n” zamiast nowych wierszy');
  const tags = text.match(/<\/?[a-ząćęłńóśźż][\wąćęłńóśźż-]*>/gi) ?? [];
  if (tags.length > 0)
    problems.push(`znaczniki w raporcie: ${[...new Set(tags)].slice(0, 3).join(' ')}`);
  return problems;
}

if (process.argv.includes('--selftest')) {
  const facts = [
    'Dane z programu (stan na 2026-10-12 08:00, poniedziałek).',
    '- „Kopia dokumentów” — włączony, następne uruchomienie 2026-10-12 20:00',
    'Koszty modeli: dziś 0,00 zł; bieżący miesiąc 12,50 zł; limit miesięczny 100,00 zł.',
  ];
  const cases = [
    ['1. Koszty dziś: 0,00 zł.\n2. Miesiąc: 12,50 zł z limitu 100 zł.', 0, []],
    ['Wyzwalacz „Kopia dokumentów” ruszy 2026-10-12 o 20:00.', 0, []],
    ['Wczoraj wydano 5000 zł.', 0, ['liczba spoza danych: 5000']],
    ['Zadanie „Raport kwartalny” czeka.', 0, ['nazwa spoza danych: „Raport kwartalny”']],
    ['Miesiąc: 12,5 zł, czyli 12.50 % limitu.', 0, []],
    ['Wyzwalacz ruszał 4 razy.', 5, []],
    ['Wyzwalacz ruszał 40 razy.', 5, ['liczba spoza danych: 40']],
  ];
  const form = [
    ['1. Koszty dziś: 0,00 zł.\n2. Bieżący miesiąc: 12,50 zł.\n3. Limit: 100,00 zł.', 0],
    ['**Dyrygentka:** Gotowość.\n**Mówczyni:** Koszty modeli: 0,00 zł, limit 100,00 zł.', 1],
    ['Koszty: 0,00 zł dziś i 12,50 zł w miesiącu. Przekazuję Delcie do potwierdzenia.', 1],
    ['Koszty: 0,00 zł dziś i 12,50 zł w miesiącu. Czy mogę jeszcze coś zrobić?', 1],
    ['Gotowe.', 1],
    ['Przepraszam, ale nie mogę wykonać tego polecenia. Jako Alfa nie mam uprawnień.', 1],
    ['1. Katalog roboczy: brak\\n2. Pliki tymczasowe: brak\\n3. Duplikaty: brak', 1],
    ['<raport>\n1. Pliki tymczasowe: brak.\n2. Duplikaty plików: brak.\n</raport>', 1],
  ];
  let failed = 0;
  for (const [report, maxCount, expected] of cases) {
    const got = judge(report, facts, maxCount);
    const ok = JSON.stringify(got) === JSON.stringify(expected);
    if (!ok) failed += 1;
    console.log(
      `${ok ? 'ok ' : 'ŹLE'} dane  ${JSON.stringify(report).slice(0, 56)} → ${JSON.stringify(got)}`,
    );
  }
  for (const [report, expected] of form) {
    const got = formProblems(report);
    const ok = got.length === expected;
    if (!ok) failed += 1;
    console.log(
      `${ok ? 'ok ' : 'ŹLE'} forma ${JSON.stringify(report).slice(0, 56)} → ${JSON.stringify(got)}`,
    );
  }
  process.exit(failed === 0 ? 0 : 1);
}
