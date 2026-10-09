// Atrapa: Hub kont i kluczy oraz import/eksport `.alfa`. Sekret z kreatora jest tylko sprawdzany
// (kształt) i odrzucany — atrapa, tak jak rdzeń, nigdy nie oddaje go z powrotem do UI.
import type { AlfaClient } from '../client';
import { cleanKey, isPlausibleKey, rotateStores, verdictOf } from '../../logic/api-key';
import { CommandError } from '../command-error';
import type { Account, DryRunItem, ModelInfo, TestReport } from '../types-hub';
import { FAKE_CATALOG } from './catalog';
import type { FakeCore } from './core';
import {
  HANDLE_EXPIRED,
  REVIEW_HANDLE_TTL_MS,
  issueHandle,
  takeHandle,
  type HandleTarget,
} from './transfer-handles';

const MODELS: Readonly<Record<string, readonly ModelInfo[]>> = {
  anthropic: [
    {
      id: 'claude-opus-5-5',
      name: 'Claude Opus 5.5',
      kinds: ['chat', 'vision'],
      context_tokens: 1_000_000,
    },
    { id: 'claude-haiku-5', name: 'Claude Haiku 5', kinds: ['chat'], context_tokens: 200_000 },
  ],
  openai: [
    { id: 'gpt-6-sol', name: 'GPT-6 Sol', kinds: ['chat', 'vision'], context_tokens: 400_000 },
    { id: 'gpt-6-luna', name: 'GPT-6 Luna', kinds: ['chat'], context_tokens: 200_000 },
  ],
};

const genericModels = (providerId: string): readonly ModelInfo[] =>
  MODELS[providerId] ?? [
    {
      id: `${providerId}-default`,
      name: `${providerId} — model domyślny`,
      kinds: ['chat'],
      context_tokens: null,
    },
  ];

/** Odmowa zapisu konta jak w rdzeniu albo `null`. */
function checkNewAccount(
  providerId: string,
  secret: string,
  baseUrl: string | null,
): string | null {
  const provider = FAKE_CATALOG.find((p) => p.id === providerId);
  if (!provider) return `nieznany dostawca \`${providerId}\``;
  if (!secret.trim()) return 'niepoprawne dane: klucz jest pusty albo zawiera niedozwolone znaki';
  const url = baseUrl?.trim() ?? '';
  if (!url) {
    return provider.needs_base_url
      ? 'niepoprawne dane: katalog nie zna endpointu tego dostawcy — podaj go'
      : null;
  }
  // Jak `validate_base_url`: host bez danych logowania (`http://localhost@inny.host`), http tylko lokalnie.
  const match = /^(https?):\/\/([^/?#]*)/.exec(url.toLowerCase());
  const authority = match?.[2] ?? '';
  const host = authority.startsWith('[') ? authority.split(']')[0] + ']' : authority.split(':')[0];
  const plain = authority !== '' && !/[@\\\s]/.test(authority);
  const local = host === 'localhost' || host === '127.0.0.1' || host === '[::1]';
  if (plain && (match?.[1] === 'https' || local)) return null;
  return `niepoprawne dane: endpoint \`${url}\` musi zaczynać się od https:// (http:// tylko dla localhost)`;
}

/** Fragment klucza → wynik testu w atrapie (np. `…-restricted-…`). */
const FAKE_OUTCOMES: readonly (readonly [string, string])[] = [
  ['restricted', 'restricted'],
  ['nocredit', 'no_credit'],
  ['region', 'refused'],
  ['refused', 'refused'],
  ['unsupported', 'unsupported'],
  ['ratelimit', 'rate_limited'],
  ['timeout', 'timeout'],
  ['bad', 'invalid_key'],
];
const FAKE_DETAILS: Readonly<
  Record<string, { error?: string; status?: number; code?: string; detail?: string }>
> = {
  invalid_key: {
    error: 'Dostawca odrzucił ten klucz.',
    status: 401,
    code: 'invalid_api_key',
    detail: 'Incorrect API key provided: [•••] You can find your API key at the provider page.',
  },
  restricted: {
    status: 401,
    detail: 'You have insufficient permissions for this operation. Missing scopes: api.model.read.',
  },
  no_credit: {
    error: 'Klucz jest poprawny, ale konto u dostawcy nie ma środków albo aktywnych płatności.',
    status: 429,
    code: 'insufficient_quota',
    detail: 'You exceeded your current quota, please check your plan and billing details.',
  },
  refused: {
    error:
      'Dostawca odmówił dostępu z powodu innego niż sam klucz (kraj, wyłączone API albo zapora).',
    status: 403,
    detail: 'Access denied.',
  },
  unsupported: {
    error: 'ten adres nie udostępnia listy modeli — klucza nie da się sprawdzić z góry',
    status: 404,
  },
  rate_limited: { status: 429, code: 'rate_limit_exceeded' },
  timeout: { error: 'Dostawca nie odpowiedział w wyznaczonym czasie.' },
};
/** Odmowa regionu ma własny kod dostawcy. */
const REGION = {
  code: 'unsupported_country_region_territory',
  detail: 'Country, region, or territory not supported',
};
const NOT_A_KEY =
  'To nie wygląda na klucz API: po kluczu zostały spacje albo znaki spoza alfabetu łacińskiego. Wklej sam klucz, bez opisu i dodatkowego tekstu.';
/** Ten sam adres mimo końcowego ukośnika i wielkości liter (jak w rdzeniu). */
const sameUrl = (a: string, b: string) =>
  a.replace(/\/+$/, '').toLowerCase() === b.replace(/\/+$/, '').toLowerCase();
const works = (outcome: string) =>
  outcome === 'ok' || outcome === 'restricted' || outcome === 'rate_limited';

export function accountsApi(core: FakeCore): AlfaClient['accounts'] {
  /** Klucze kont tej atrapy (tylko w pamięci karty) — od nich zależy wynik testu. */
  const keys = new Map<string, string>();
  const update = (id: string, patch: Partial<Account>): Account | undefined => {
    const index = core.accounts.findIndex((a) => a.id === id);
    const current = core.accounts[index];
    if (!current) return undefined;
    const next = { ...current, ...patch };
    core.accounts[index] = next;
    core.emit([{ type: 'AccountChanged', account: next }]);
    return next;
  };
  const missing = () => Promise.reject(new CommandError('not_found', 'Nie ma takiego konta.'));
  const outcomeOf = (key: string): string =>
    core.status.online
      ? (FAKE_OUTCOMES.find(([mark]) => key.includes(mark))?.[1] ?? 'ok')
      : 'network';
  /** Raport testu danego klucza i to, co po nim zapisuje się przy koncie. */
  const probe = (account: Account, key: string) => {
    const outcome = outcomeOf(key);
    const ok = works(outcome);
    const extra = { ...FAKE_DETAILS[outcome], ...(key.includes('region') ? REGION : {}) };
    const shared = {
      ...(extra.status ? { status: extra.status } : {}),
      ...(extra.code ? { provider_code: extra.code } : {}),
      ...(extra.detail ? { detail: extra.detail } : {}),
    };
    // Lista modeli zmienia się tylko po jej udanym odczycie (jak w rdzeniu).
    const models = outcome === 'ok' ? genericModels(account.provider_id) : account.models;
    const state: Account['state'] =
      outcome === 'unsupported' ? account.state : ok ? 'ok' : 'invalid';
    const stored: Partial<Account> = {
      state: account.state === 'disabled' ? 'disabled' : state,
      models,
      last_tested_at: core.isoNow(),
      last_test: { ok, reason: outcome, latency_ms: ok ? 312 : null, ...shared },
    };
    const report: TestReport = {
      ok,
      latency_ms: ok ? 312 : null,
      models,
      error: ok ? null : (extra.error ?? 'Brak połączenia z internetem.'),
      ...(outcome === 'ok' ? {} : { reason: outcome }),
      ...shared,
    };
    return { ok, stored, report };
  };
  /** Konto tego dostawcy pod tym samym adresem z tym samym kluczem (jak `find_by_secret`). */
  const twin = (providerId: string, baseUrl: string | null, key: string, except?: string) =>
    core.accounts.find(
      (a) =>
        a.id !== except &&
        a.provider_id === providerId &&
        sameUrl(urls.get(a.id) ?? '', baseUrl ?? '') &&
        keys.get(a.id) === key,
    );
  const urls = new Map<string, string>();
  return {
    catalog: () => core.reply(FAKE_CATALOG),
    list: () => core.reply(core.accounts),
    add: (input) => {
      // Te same reguły co rdzeń (accounts-hub `check_new`), z tymi samymi komunikatami.
      const key = cleanKey(input.secret);
      const rejected = checkNewAccount(input.provider_id, key, input.base_url);
      if (rejected) return Promise.reject(new CommandError('invalid_input', rejected));
      if (!isPlausibleKey(key)) return Promise.reject(new CommandError('invalid_input', NOT_A_KEY));
      const url = input.base_url?.trim() || null;
      const same = twin(input.provider_id, url, key);
      if (same)
        return Promise.reject(
          new CommandError(
            'invalid_input',
            `Ten klucz jest już dodany jako konto „${same.label}”. Jeśli chcesz go sprawdzić, użyj „Testuj połączenie” przy tym koncie.`,
          ),
        );
      const account: Account = {
        id: core.nextId('acc'),
        provider_id: input.provider_id,
        label: input.label,
        state: 'ok',
        key_stored: true,
        models: [],
        assignments: { task_classes: ['chat'], agents: [], voice_stt: false, voice_tts: false },
        cost_limit: { enabled: false, monthly: { minor: 0, currency: 'PLN' } },
        last_tested_at: null,
      };
      keys.set(account.id, key);
      urls.set(account.id, url ?? '');
      core.accounts.push(account);
      if (!core.status.keys_configured)
        core.setStatus({ keys_configured: true, profile: 'hybrid' });
      return core.reply(account);
    },
    test: (id) => {
      const account = core.accounts.find((a) => a.id === id);
      if (!account) return missing();
      const { stored, report } = probe(account, keys.get(id) ?? (account.key_stored ? '' : 'bad'));
      update(id, stored);
      return core.reply(report);
    },
    rotate: (id, secret) => {
      const account = core.accounts.find((a) => a.id === id);
      if (!account) return missing();
      const key = cleanKey(secret);
      if (!isPlausibleKey(key)) return Promise.reject(new CommandError('invalid_input', NOT_A_KEY));
      const same = twin(account.provider_id, urls.get(id) || null, key, id);
      if (same)
        return Promise.reject(
          new CommandError(
            'invalid_input',
            `Ten klucz jest już używany przez konto „${same.label}”.`,
          ),
        );
      // Najpierw test kandydata; zapis tylko wtedy, gdy dostawca go rozpoznał albo nie da się
      // go sprawdzić (jak `accepts_new_key` w rdzeniu) — inaczej stary klucz i stan konta zostają.
      const { stored, report } = probe(account, key);
      if (rotateStores(verdictOf(report))) {
        keys.set(id, key);
        update(id, { ...stored, key_stored: true });
      }
      return core.reply(report);
    },
    setDisabled: (id, disabled) => {
      const account = core.accounts.find((a) => a.id === id);
      if (!account) return missing();
      if (disabled) return core.reply(update(id, { state: 'disabled' }) ?? account);
      // Włączenie przywraca stan z ostatniego testu; konto niewyłączone zostaje, jakie było.
      if (account.state !== 'disabled') return core.reply(account);
      const last = account.last_test;
      const state = !last || last.ok || last.reason === 'unsupported' ? 'ok' : 'invalid';
      return core.reply(update(id, { state }) ?? account);
    },
    setModel: (id, model) => {
      const account = core.accounts.find((a) => a.id === id);
      if (!account) return missing();
      const name = model.trim();
      if (!name)
        return Promise.reject(
          new CommandError('invalid_input', 'Podaj nazwę modelu, np. gpt-4o-mini.'),
        );
      const models = [{ id: name, name, kinds: ['chat'] as const, context_tokens: null }];
      return core.reply(update(id, { models }) ?? account);
    },
    assign: (id, assignment) => core.reply(void update(id, { assignments: assignment })),
    setLimit: (id, enabled, monthly) =>
      core.reply(void update(id, { cost_limit: { enabled, monthly } })),
    remove: (id) => {
      core.accounts = core.accounts.filter((a) => a.id !== id);
      keys.delete(id);
      urls.delete(id);
      if (core.accounts.length === 0) core.setStatus({ keys_configured: false, profile: 'local' });
      return core.reply(undefined);
    },
  };
}

const DRY_RUN: readonly DryRunItem[] = [
  { key: 'config.common', kind: 'config', label: 'Konfiguracja wspólna', diff: 'changed' },
  { key: 'personas', kind: 'persona', label: 'Agentki i biblie głosów', diff: 'same' },
  { key: 'casts', kind: 'cast', label: 'Obsady ról (4)', diff: 'new' },
  { key: 'session:s-q3', kind: 'session', label: 'Raport Q3', diff: 'collision' },
  { key: 'session:s-laptop-1', kind: 'session', label: 'Notatki z laptopa', diff: 'new' },
];

export function transferApi(core: FakeCore): AlfaClient['transfer'] {
  return {
    exportPackage: (request) => {
      const files = 3 + request.scope.sessions.length * 3;
      return core.reply({
        status: 'saved' as const,
        path: 'C:\\Users\\Ty\\Documents\\alfa-eksport-2026-09-30.alfa',
        files,
        bytes: 12_000 + files * 4_096,
      });
    },
    inspect: (password, handle) => {
      // Bez uchwytu — „natywny dialog” (atrapa: paczka z laptopa); inaczej uchwyt zużywany.
      const dialog: HandleTarget = {
        path: 'C:\\Users\\Ty\\Pobrane\\laptop.alfa',
        encrypted: false,
      };
      const target = handle === null ? dialog : takeHandle(core, handle);
      if (!target) return Promise.reject(new Error(HANDLE_EXPIRED));
      const next = issueHandle(core, target, REVIEW_HANDLE_TTL_MS);
      if (target.encrypted && !password) {
        return core.reply({ status: 'needs_password' as const, handle: next });
      }
      return core.reply({
        status: 'inspected' as const,
        handle: next,
        manifest: {
          schema_version: '1.0.0',
          app_version: '0.1.0-f1',
          created_at: new Date(core.scheduler.now() - 86_400_000).toISOString(),
          source_machine: 'LAPTOP-ALFA (laptop-cuda)',
          encrypted: Boolean(password),
        },
        items: DRY_RUN,
        warnings: ['Nakładka maszyny z laptopa nie zostanie zaimportowana.'],
        migrations: [],
      });
    },
    importPackage: (request) => {
      if (!takeHandle(core, request.handle)) return Promise.reject(new Error(HANDLE_EXPIRED));
      return core.reply({
        snapshot_id: core.nextId('snap'),
        imported: DRY_RUN.filter((i) => i.diff !== 'same').length,
        skipped: request.mode === 'add' ? 1 : 0,
        failed: 0,
      });
    },
    rollback: () => core.reply(undefined),
  };
}
