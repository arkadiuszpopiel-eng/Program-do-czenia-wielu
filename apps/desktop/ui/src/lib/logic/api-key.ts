// Klucz API wklejany przez człowieka: sprzątanie, rozpoznanie dostawcy po początku klucza
// i wyjaśnienie wyniku testu. Czysta logika — kreator tylko ją wyświetla.
import type { TestReport } from '../api/types-hub';

/** Co wiemy o kluczach danego dostawcy: gdzie się je tworzy i po czym je poznać. */
export interface KeyHelp {
  /** Strona, na której tworzy się klucz. */
  readonly url: string;
  /** Początki kluczy tego dostawcy (najbardziej szczegółowe najpierw). */
  readonly prefixes: readonly string[];
  /** Abonament czatu u tego dostawcy nie obejmuje API (częsta pomyłka). */
  readonly separateBilling?: boolean;
}

export const KEY_HELP: Readonly<Record<string, KeyHelp>> = {
  openai: {
    url: 'https://platform.openai.com/api-keys',
    prefixes: ['sk-'],
    separateBilling: true,
  },
  anthropic: {
    url: 'https://console.anthropic.com/settings/keys',
    prefixes: ['sk-ant-'],
    separateBilling: true,
  },
  google: { url: 'https://aistudio.google.com/apikey', prefixes: ['AIza'] },
  xai: { url: 'https://console.x.ai', prefixes: ['xai-'] },
  mistral: { url: 'https://console.mistral.ai/api-keys', prefixes: [] },
  deepseek: { url: 'https://platform.deepseek.com/api_keys', prefixes: ['sk-'] },
  openrouter: { url: 'https://openrouter.ai/keys', prefixes: ['sk-or-'] },
  elevenlabs: { url: 'https://elevenlabs.io/app/settings/api-keys', prefixes: [] },
};

/** Nazwy dostawców rozpoznawanych po początku klucza (do komunikatu „to wygląda na klucz …”). */
export const PROVIDER_NAMES: Readonly<Record<string, string>> = {
  anthropic: 'Anthropic',
  openrouter: 'OpenRouter',
  google: 'Google',
  xai: 'xAI',
};

/** Początki kluczy, po których da się jednoznacznie wskazać dostawcę. */
const DISTINCT: readonly (readonly [prefix: string, provider: string])[] = [
  ['sk-ant-', 'anthropic'],
  ['sk-or-', 'openrouter'],
  ['AIza', 'google'],
  ['xai-', 'xai'],
];

/** Token logowania narzędzia Claude Code — to nie jest klucz API i Alfa go nie przyjmuje. */
const CLI_TOKEN = 'sk-ant-oat';

const QUOTES = /^["'`„”“’‘«»]+|["'`„”“’‘«»]+$/g;
const INVISIBLE = /[\u0085\u00A0\u00AD\u200B-\u200F\u202A-\u202E\u2060-\u2069\uFEFF]/g;
const WRAPPED = /[\r\n\t]/;
const PREFIXES = ['export ', 'set ', 'setx ', '$env:'] as const;

const stripPrefix = (text: string, prefix: string): string | null =>
  text.slice(0, prefix.length).toLowerCase() === prefix ? text.slice(prefix.length) : null;

/**
 * Sam klucz z tego, co trafiło do pola. Te same reguły co w rdzeniu (`SecretString::from_input`,
 * wspólna tabela przypadków: `crates/accounts-hub-contract/tests/fixtures/key-inputs.json`):
 * zdejmujemy tylko to, co da się rozpoznać jednoznacznie — cudzysłowy, `NAZWA_KEY=`, `set`,
 * `$env:`, `Authorization: Bearer`, przecinek z pliku JSON, znak nowej linii. Zwykłe zdanie zostaje
 * zdaniem (ze spacjami) i nie jest potem traktowane jak klucz.
 */
export function cleanKey(raw: string): string {
  let text = raw.replace(INVISIBLE, '').trim();
  for (let round = 0; round < 6; round++) {
    const before = text;
    text = text.replace(/[,;]+$/, '').trim();
    text = text.replace(QUOTES, '').trim();
    for (const prefix of PREFIXES) text = (stripPrefix(text, prefix) ?? text).trimStart();
    text = (stripPrefix(text, 'authorization:') ?? text).trimStart();
    text = (stripPrefix(text, 'bearer ') ?? text).trimStart();
    const at = text.search(/[=: ]/);
    if (at > 0) {
      const name = text.slice(0, at).trim().replace(QUOTES, '');
      const after = text.slice(at + 1);
      const value = after.trim().replace(QUOTES, '');
      const isVariable =
        /^[A-Za-z0-9_]+$/.test(name) &&
        /(KEY|TOKEN)$/i.test(name) &&
        // `apikey:tajne` bywa całym kluczem — dwukropek zdejmujemy tylko po nazwie z podkreśleniem
        // albo gdy po nim jest odstęp lub cudzysłów (`"apiKey": "…"`).
        (text[at] !== ':' || name.includes('_') || /^[ "']/.test(after)) &&
        // Reszta ma wyglądać jak wartość: `…KEY==` to koniec klucza base64.
        value.length >= 8 &&
        /[^=]/.test(value);
      if (isVariable) text = after.trim();
    }
    if (text === before) break;
  }
  // Zawinięty wiersz sklejamy; dwa osobne wpisy (dwie linie `.env`, dwa razy ten sam klucz) zostają
  // rozdzielone i nie są kluczem — sklejenie wysłałoby klucz jednego dostawcy do drugiego.
  const parts = text
    .split(WRAPPED)
    .map((part) => part.trim())
    .filter(Boolean);
  const separate = parts.some(
    (part, at) =>
      at > 0 &&
      (/[ :]/.test(part) || part.replace(/=+$/, '').includes('=') || parts.indexOf(part) < at),
  );
  return parts.join(separate ? '\n' : '');
}

/** Czy tekst po sprzątaniu może być kluczem: same drukowalne znaki ASCII, bez spacji. */
export const isPlausibleKey = (key: string): boolean =>
  key.length > 0 && key.length <= 4096 && /^[\x21-\x7E]+$/.test(key);

export type KeyCheck =
  | { readonly kind: 'empty' }
  | { readonly kind: 'ok' }
  /** Po sprzątaniu zostały spacje albo znaki spoza ASCII — to zdanie, nie klucz. */
  | { readonly kind: 'not_a_key' }
  /** Token logowania Claude Code zamiast klucza API. */
  | { readonly kind: 'cli_token' }
  /** Klucz wygląda na klucz innego dostawcy. */
  | { readonly kind: 'other_provider'; readonly provider: string }
  /** Dostawca ma znany początek klucza, a ten zaczyna się inaczej. */
  | { readonly kind: 'unexpected_prefix'; readonly expected: string }
  | { readonly kind: 'too_short'; readonly length: number };

/** Ocena klucza przed wysłaniem. Blokują tylko `empty`, `not_a_key` i `cli_token`. */
export function checkKey(providerId: string, raw: string): KeyCheck {
  const key = cleanKey(raw);
  if (!key) return { kind: 'empty' };
  if (!isPlausibleKey(key)) return { kind: 'not_a_key' };
  if (key.startsWith(CLI_TOKEN)) return { kind: 'cli_token' };
  const other = DISTINCT.find(([prefix]) => key.startsWith(prefix))?.[1];
  if (other && other !== providerId && providerId in KEY_HELP)
    return { kind: 'other_provider', provider: other };
  const expected = KEY_HELP[providerId]?.prefixes ?? [];
  const first = expected[0];
  if (first && !expected.some((prefix) => key.startsWith(prefix)))
    return { kind: 'unexpected_prefix', expected: first };
  if (key.length < 20) return { kind: 'too_short', length: key.length };
  return { kind: 'ok' };
}

/** Czy ocena blokuje wysłanie (reszta to podpowiedzi — formaty kluczy się zmieniają). */
export const blocksSubmit = (check: KeyCheck): boolean =>
  check.kind === 'empty' || check.kind === 'not_a_key' || check.kind === 'cli_token';

/** Rodzaje wyniku testu, dla których interfejs ma osobne wyjaśnienie i kroki. */
export type TestVerdict =
  | 'ok'
  | 'restricted'
  | 'rate_limited'
  | 'invalid_key'
  | 'no_credit'
  | 'region'
  | 'refused'
  | 'timeout'
  | 'network'
  | 'unsupported';

const KNOWN: readonly TestVerdict[] = [
  'restricted',
  'rate_limited',
  'invalid_key',
  'no_credit',
  'refused',
  'timeout',
  'network',
  'unsupported',
];

/** Wynik testu → jedno z wyjaśnień. Nieznany powód traktujemy jak błąd połączenia. */
export function verdictOf(
  report: Pick<TestReport, 'ok' | 'reason' | 'provider_code'>,
): TestVerdict {
  if (report.provider_code === 'unsupported_country_region_territory') return 'region';
  if (!report.reason || report.reason === 'ok') return report.ok ? 'ok' : 'network';
  const known = KNOWN.find((verdict) => verdict === report.reason);
  return known ?? (report.ok ? 'ok' : 'network');
}

/**
 * Co wolno z kontem po danym wyniku:
 * `works` — można iść dalej (klucz działa albo nie da się go sprawdzić z góry); `keep` — klucz może
 * być dobry (brak środków, sieć, limit zapytań, odmowa z innego powodu), więc konto zostaje i warto
 * sprawdzić ponownie; `fix` — trzeba innego klucza.
 */
export function actionFor(verdict: TestVerdict): 'works' | 'keep' | 'fix' {
  if (verdict === 'ok' || verdict === 'restricted' || verdict === 'unsupported') return 'works';
  return verdict === 'invalid_key' ? 'fix' : 'keep';
}

/**
 * Czy wymiana klucza zapisała kandydata (jak `TestOutcome::accepts_new_key` w rdzeniu): dostawca go
 * rozpoznał — także przy braku środków — albo klucza nie da się sprawdzić z góry. Po limicie
 * zapytań, błędzie sieci i odmowie konto zostaje przy poprzednim kluczu.
 */
export const rotateStores = (verdict: TestVerdict): boolean =>
  verdict === 'ok' ||
  verdict === 'restricted' ||
  verdict === 'no_credit' ||
  verdict === 'unsupported';

/** Ile kroków „co zrobić” ma dane wyjaśnienie (teksty: `wiz.fix.<werdykt>.<n>`). */
export const FIX_STEPS: Readonly<Record<TestVerdict, number>> = {
  ok: 0,
  restricted: 2,
  rate_limited: 1,
  invalid_key: 4,
  no_credit: 2,
  region: 1,
  refused: 2,
  timeout: 2,
  network: 2,
  unsupported: 1,
};
