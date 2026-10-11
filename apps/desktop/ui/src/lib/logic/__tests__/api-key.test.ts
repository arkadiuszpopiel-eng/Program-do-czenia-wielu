// Klucz API wklejany przez człowieka: sprzątanie, ocena przed wysłaniem i wyjaśnienie wyniku testu.
import { describe, expect, it } from 'vitest';
import type { TestReport } from '../../api/types-hub';
import {
  FIX_STEPS,
  KEY_HELP,
  PROVIDER_NAMES,
  actionFor,
  blocksSubmit,
  checkKey,
  cleanKey,
  isPlausibleKey,
  keyAgeDays,
  keyRotationDue,
  rotateStores,
  type TestVerdict,
  verdictOf,
} from '../api-key';
import { en } from '../../i18n/en';
import { pl } from '../../i18n/pl';
// Ta sama tabela przypadków, na której rdzeń sprawdza `SecretString::from_input`.
import inputs from '../../../../../../../crates/accounts-hub-contract/tests/fixtures/key-inputs.json';

/** Zmyślony klucz o długości prawdziwego (to nie jest sekret). */
const fake = (prefix: string) => `${prefix}${'XXXX'.repeat(8)}`;
const KEY = fake('sk-');
const fill = (pattern: string) => pattern.replaceAll('{key}', inputs.key);

const report = (patch: Partial<TestReport>): TestReport => ({
  ok: false,
  latency_ms: null,
  models: [],
  error: null,
  ...patch,
});

describe('cleanKey — wspólna tabela z rdzeniem', () => {
  it('zostawia sam klucz z tego, co dokleja schowek', () => {
    for (const pattern of inputs.cleaned)
      expect(cleanKey(fill(pattern)), JSON.stringify(pattern)).toBe(inputs.key);
  });

  it('zdania i tekstu z polskimi znakami nie uznaje za klucz', () => {
    for (const pattern of inputs.rejected)
      expect(isPlausibleKey(cleanKey(fill(pattern))), JSON.stringify(pattern)).toBe(false);
  });

  it('nie rusza tego, co już jest samym kluczem', () => {
    for (const text of inputs.unchanged) {
      expect(cleanKey(text), text).toBe(text);
      expect(isPlausibleKey(text), text).toBe(true);
    }
  });

  it('skleja klucz złamany na dwa wiersze, ale nie dwa osobne wpisy', () => {
    expect(cleanKey(`${KEY.slice(0, 10)}\r\n${KEY.slice(10)}`)).toBe(KEY);
    expect(cleanKey(`abcdefgh\nijklmnop==`)).toBe('abcdefghijklmnop==');
    const env = cleanKey(`OPENAI_API_KEY=${KEY}\nANTHROPIC_API_KEY=${fake('sk-ant-')}`);
    expect(isPlausibleKey(env)).toBe(false);
    expect(checkKey('openai', `${KEY}\n${KEY}`).kind).toBe('not_a_key');
  });

  it('nazwy zmiennej z literami spoza ASCII nie traktuje jak przypisania (jak rdzeń)', () => {
    expect(isPlausibleKey(cleanKey('straßen_key=abcdefgh1234'))).toBe(false);
  });
});

describe('checkKey', () => {
  it('rozpoznaje klucz innego dostawcy', () => {
    expect(checkKey('openai', fake('sk-ant-'))).toEqual({
      kind: 'other_provider',
      provider: 'anthropic',
    });
    expect(checkKey('anthropic', fake('AIza'))).toEqual({
      kind: 'other_provider',
      provider: 'google',
    });
    expect(checkKey('anthropic', fake('sk-ant-')).kind).toBe('ok');
  });

  it('zwraca uwagę na nietypowy początek i za krótki klucz, ale ich nie blokuje', () => {
    const prefix = checkKey('openai', fake('pk-'));
    expect(prefix).toEqual({ kind: 'unexpected_prefix', expected: 'sk-' });
    const short = checkKey('openai', 'sk-krotki');
    expect(short.kind).toBe('too_short');
    expect(blocksSubmit(prefix) || blocksSubmit(short)).toBe(false);
    expect(checkKey('openai', `"${KEY}"\n`)).toEqual({ kind: 'ok' });
  });

  it('blokuje tylko to, co kluczem na pewno nie jest', () => {
    for (const [text, kind] of [
      ['  ', 'empty'],
      ['to jest zdanie, nie klucz', 'not_a_key'],
      [`Mój klucz: ${KEY}`, 'not_a_key'],
      [fake('sk-ant-oat01-'), 'cli_token'],
    ] as const) {
      const check = checkKey('anthropic', text);
      expect(check.kind, text).toBe(kind);
      expect(blocksSubmit(check), text).toBe(true);
    }
  });

  it('dla dostawcy spoza listy nie zgaduje formatu', () => {
    expect(checkKey('custom-openai-compatible', fake('sk-ant-')).kind).toBe('ok');
    expect(checkKey('mistral', fake('dowolny-')).kind).toBe('ok');
  });

  it('każdy rozpoznawany dostawca ma nazwę do komunikatu', () => {
    for (const prefix of ['sk-ant-', 'sk-or-', 'AIza', 'xai-']) {
      const check = checkKey('openai', fake(prefix));
      expect(check.kind, prefix).toBe('other_provider');
      if (check.kind === 'other_provider')
        expect(PROVIDER_NAMES[check.provider], prefix).toBeTruthy();
    }
  });
});

describe('verdictOf i actionFor', () => {
  it('mapuje powód z rdzenia na wyjaśnienie', () => {
    expect(verdictOf(report({ ok: true }))).toBe('ok');
    expect(verdictOf(report({ ok: true, reason: 'ok' }))).toBe('ok');
    expect(verdictOf(report({ ok: true, reason: 'restricted' }))).toBe('restricted');
    expect(verdictOf(report({ ok: true, reason: 'rate_limited' }))).toBe('rate_limited');
    expect(verdictOf(report({ reason: 'invalid_key', status: 401 }))).toBe('invalid_key');
    expect(verdictOf(report({ reason: 'no_credit' }))).toBe('no_credit');
    expect(verdictOf(report({ reason: 'refused', status: 403 }))).toBe('refused');
    expect(verdictOf(report({ reason: 'timeout' }))).toBe('timeout');
    expect(verdictOf(report({ reason: 'unsupported' }))).toBe('unsupported');
    expect(verdictOf(report({ reason: 'coś_nowego' }))).toBe('network');
    expect(verdictOf(report({}))).toBe('network');
  });

  it('blokadę regionu pokazuje jako osobny przypadek', () => {
    const blocked = report({
      reason: 'refused',
      status: 403,
      provider_code: 'unsupported_country_region_territory',
    });
    expect(verdictOf(blocked)).toBe('region');
  });

  it('nowego klucza wymaga tylko klucz odrzucony; przy reszcie konto może zostać', () => {
    const actions = Object.fromEntries(
      (Object.keys(FIX_STEPS) as TestVerdict[]).map((verdict) => [verdict, actionFor(verdict)]),
    );
    expect(actions).toEqual({
      ok: 'works',
      restricted: 'works',
      rate_limited: 'keep',
      invalid_key: 'fix',
      no_credit: 'keep',
      region: 'keep',
      refused: 'keep',
      timeout: 'keep',
      network: 'keep',
      unsupported: 'works',
    });
  });

  it('wymiana zapisuje klucz rozpoznany przez dostawcę albo niesprawdzalny, nie „może dobry”', () => {
    const stored = (Object.keys(FIX_STEPS) as TestVerdict[]).filter(rotateStores);
    expect(stored.sort()).toEqual(['no_credit', 'ok', 'restricted', 'unsupported']);
  });
});

describe('teksty kreatora', () => {
  for (const [name, text] of [
    ['pl', pl as Record<string, unknown>],
    ['en', en as Record<string, unknown>],
  ] as const) {
    it(`${name}: każde wyjaśnienie ma tytuł, opis, kroki i etykietę stanu`, () => {
      for (const [verdict, steps] of Object.entries(FIX_STEPS)) {
        expect(text[`wiz.verdict.${verdict}`], `tytuł ${verdict}`).toBeTruthy();
        expect(text[`hub.reason.${verdict}`], `stan ${verdict}`).toBeTruthy();
        if (verdict !== 'ok') expect(text[`wiz.why.${verdict}`], `opis ${verdict}`).toBeTruthy();
        for (let n = 1; n <= steps; n++)
          expect(text[`wiz.fix.${verdict}.${n}`], `krok ${verdict}.${n}`).toBeTruthy();
        expect(
          text[`wiz.fix.${verdict}.${steps + 1}`],
          `nadmiarowy krok ${verdict}`,
        ).toBeUndefined();
      }
    });
  }

  it('adresy stron z kluczami są adresami https', () => {
    for (const [id, help] of Object.entries(KEY_HELP))
      expect(help.url, id).toMatch(/^https:\/\/[a-z0-9.-]+(\/|$)/);
  });
});

describe('wiek klucza', () => {
  const now = Date.parse('2026-10-11T12:00:00Z');

  it('liczy pełne dni od zapisu klucza', () => {
    expect(keyAgeDays('2026-10-11T08:00:00Z', now)).toBe(0);
    expect(keyAgeDays('2026-10-10T11:59:00Z', now)).toBe(1);
    expect(keyAgeDays('2026-04-14T12:00:00Z', now)).toBe(180);
    // Zegar cofnięty albo data z przyszłości: nigdy liczba ujemna.
    expect(keyAgeDays('2026-12-01T00:00:00Z', now)).toBe(0);
  });

  it('brak albo nieczytelna data to „nie wiadomo”, nie zero', () => {
    expect(keyAgeDays(null, now)).toBeNull();
    expect(keyAgeDays(undefined, now)).toBeNull();
    expect(keyAgeDays('wczoraj', now)).toBeNull();
  });

  it('przypomnienie o wymianie: od progu, a próg 0 je wyłącza', () => {
    expect(keyRotationDue(179, 180)).toBe(false);
    expect(keyRotationDue(180, 180)).toBe(true);
    expect(keyRotationDue(999, 0)).toBe(false);
    expect(keyRotationDue(null, 180)).toBe(false);
  });
});
