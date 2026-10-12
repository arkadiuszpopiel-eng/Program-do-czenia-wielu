// Ocena „czy raport agentki trzyma się danych” (docs/plan/24 §2.6). Raport jest zielony, gdy każda
// liczba (kwota, data, godzina, licznik) i każda nazwa w cudzysłowie występuje w danych, które
// program podał agentce. Numery punktów listy („1.”, „2)”) nie są liczbami z danych.
//
//   import { judge } from './grounding.mjs'
//   node scripts/trial/grounding.mjs --selftest

/** Liczby z tekstu w postaci znormalizowanej: `1 234,50` → `1234.50`, `08:00` zostaje. */
export function numbersOf(text) {
  const cleaned = text
    .split(/\r?\n/)
    .map((line) => line.replace(/^\s*(?:[-*•]\s*)?\d{1,2}[.)]\s+/, ''))
    .join('\n');
  const found = cleaned.match(/\d+(?:[  ]\d{3})*(?:[.,:-]\d+)*/g) ?? [];
  return found.map((n) => n.replace(/[  ]/g, '').replace(/,/g, '.'));
}

/** Nazwy w cudzysłowach („…”, "…", «…»), bez końcowej interpunkcji. */
export function namesOf(text) {
  const found = [...text.matchAll(/[„"«]([^„”"«»\n]{2,90})[”"»]/g)].map((m) => m[1].trim());
  return [...new Set(found)];
}

const fold = (s) => s.toLowerCase().replace(/\s+/g, ' ').trim();

/**
 * `facts` — teksty, które agentka dostała (arkusz faktów, polecenie). Zwraca listę wyjść poza dane.
 * Liczba jest dozwolona także jako część liczby z danych zapisanej inaczej (np. `0` wobec `0.00`).
 */
export function judge(report, facts) {
  const source = facts.join('\n');
  const allowed = new Set(numbersOf(source));
  const allowedValues = new Set([...allowed].map(Number).filter(Number.isFinite));
  const sourceFolded = fold(source);
  const problems = [];
  for (const number of numbersOf(report)) {
    const value = Number(number);
    const ok = allowed.has(number) || (Number.isFinite(value) && allowedValues.has(value));
    if (!ok) problems.push(`liczba spoza danych: ${number}`);
  }
  for (const name of namesOf(report)) {
    if (!sourceFolded.includes(fold(name))) problems.push(`nazwa spoza danych: „${name}”`);
  }
  return [...new Set(problems)];
}

if (process.argv.includes('--selftest')) {
  const facts = [
    'Dane z programu (stan na 2026-10-12 08:00, poniedziałek).',
    '- „Kopia dokumentów” — włączony, następne uruchomienie 2026-10-12 20:00',
    'Koszty modeli: dziś 0,00 zł; bieżący miesiąc 12,50 zł; limit miesięczny 100,00 zł.',
  ];
  const cases = [
    ['1. Koszty dziś: 0,00 zł.\n2. Miesiąc: 12,50 zł z limitu 100 zł.', []],
    ['Wyzwalacz „Kopia dokumentów” ruszy 2026-10-12 o 20:00.', []],
    ['Wczoraj wydano 5000 zł.', ['liczba spoza danych: 5000']],
    ['Zadanie „Raport kwartalny” czeka.', ['nazwa spoza danych: „Raport kwartalny”']],
    ['Miesiąc: 12,5 zł, czyli 12.50 % limitu.', []],
  ];
  let failed = 0;
  for (const [report, expected] of cases) {
    const got = judge(report, facts);
    const ok = JSON.stringify(got) === JSON.stringify(expected);
    if (!ok) failed += 1;
    console.log(
      `${ok ? 'ok ' : 'ŹLE'} ${JSON.stringify(report).slice(0, 60)} → ${JSON.stringify(got)}`,
    );
  }
  process.exit(failed === 0 ? 0 : 1);
}
