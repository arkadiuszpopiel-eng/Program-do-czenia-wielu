// Atrapa: Hub kont i kluczy (import/eksport `.alfa`: `api-transfer.ts`). Sekret z kreatora jest tylko sprawdzany
// (kształt) i odrzucany — atrapa, tak jak rdzeń, nigdy nie oddaje go z powrotem do UI.
import type { AlfaClient } from '../client';
import { cleanKey, isPlausibleKey, rotateStores, verdictOf } from '../../logic/api-key';
import { AZURE_OPENAI, billingProject, checkEnterprise, coreMessage } from '../../logic/enterprise';
import { CommandError } from '../command-error';
import type { Account, EnterpriseSettings, ModelInfo, TestReport } from '../types-hub';
import {
  MAX_QUEUE,
  TASK_CLASSES,
  fitsClass,
  type QueueStrategy,
  type TaskClassId,
} from '../../logic/model-queue';
import { FAKE_CATALOG } from './catalog';
import { enterpriseApi } from './api-enterprise';
import { routingApi } from './api-routing';
import { sharedQueueApi } from './api-shared-queue';
import { OPENAI_MODELS, fakeModel, purposeOf, suggestFake } from './model-info';
import type { FakeCore } from './core';

const MODELS: Readonly<Record<string, readonly ModelInfo[]>> = {
  anthropic: [
    fakeModel('claude-opus-5-5', 'Claude Opus 5.5', 1_000_000),
    fakeModel('claude-haiku-5', 'Claude Haiku 5', 200_000),
  ],
  openai: OPENAI_MODELS,
};

const genericModels = (providerId: string): readonly ModelInfo[] =>
  MODELS[providerId] ?? [fakeModel(`${providerId}-default`, `${providerId} — model domyślny`)];
/** Modele konta po udanym teście; Azure OpenAI — wdrożenia z konta (jak adapter w rdzeniu). */
const modelsOf = (a: Account): readonly ModelInfo[] =>
  a.provider_id === AZURE_OPENAI
    ? (a.enterprise?.deployments ?? []).map((d) => fakeModel(d.model ?? d.name, d.name))
    : genericModels(a.provider_id);

/** Model rozmowy wybierany automatycznie (jak `best_chat_model` w rdzeniu). */
const autoModel = (models: readonly ModelInfo[]): string | null =>
  suggestFake(models, 'chat', 'balanced')[0] ?? null;

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
      // OpenRouter sprawdza klucz osobno (`GET /key`) i podaje limit i zużycie klucza.
      ...(outcome === 'ok' && account.provider_id === 'openrouter'
        ? { detail: 'limit klucza 50,00 USD, zużyto 1,23 USD' }
        : {}),
    };
    // Lista modeli zmienia się tylko po jej udanym odczycie (jak w rdzeniu).
    const models = outcome === 'ok' ? modelsOf(account) : account.models;
    // Jak rdzeń (0.0.69): brak sieci albo limit czasu nie wyłącza konta, którego poprzedni test
    // potwierdził klucz.
    const kept =
      (outcome === 'network' || outcome === 'timeout') &&
      account.state === 'ok' &&
      account.last_test?.ok === true;
    const state: Account['state'] =
      outcome === 'unsupported' || kept ? account.state : ok ? 'ok' : 'invalid';
    const stored: Partial<Account> = {
      state: account.state === 'disabled' ? 'disabled' : state,
      models,
      auto_model: autoModel(models),
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
  /** Konto dostawcy pod tym adresem, w tym projekcie OpenAI, z tym kluczem (jak `find_by_secret`). */
  const twin = (key: string, id: string, url: string | null, e?: EnterpriseSettings, ex?: string) =>
    core.accounts.find(
      (a) =>
        a.id !== ex &&
        a.provider_id === id &&
        sameUrl(urls.get(a.id) ?? '', url ?? '') &&
        billingProject(a.enterprise) === billingProject(e) &&
        keys.get(a.id) === key,
    );
  const urls = new Map<string, string>();
  return {
    ...sharedQueueApi(core),
    ...routingApi(core),
    catalog: () => core.reply(FAKE_CATALOG),
    list: () => core.reply(core.accounts),
    add: (input) => {
      // Te same reguły co rdzeń (accounts-hub `check_new`), z tymi samymi komunikatami.
      const key = cleanKey(input.secret);
      const rejected = checkNewAccount(input.provider_id, key, input.base_url);
      if (rejected) return Promise.reject(new CommandError('invalid_input', rejected));
      if (!isPlausibleKey(key)) return Promise.reject(new CommandError('invalid_input', NOT_A_KEY));
      const url = input.base_url?.trim() || null;
      const same = twin(key, input.provider_id, url, input.enterprise);
      if (same)
        return Promise.reject(
          new CommandError(
            'invalid_input',
            `Ten klucz jest już dodany jako konto „${same.label}”. Jeśli chcesz go sprawdzić, użyj „Testuj połączenie” przy tym koncie.`,
          ),
        );
      // Konto firmowe sprawdza hub przy zapisie (`EnterpriseSettings::checked`) — po duplikacie.
      const firm = checkEnterprise(input.provider_id, url, input.enterprise);
      if (!firm.ok)
        return Promise.reject(
          new CommandError('invalid_input', coreMessage(firm.problem, firm.subject)),
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
        key_saved_at: core.isoNow(),
        ...(url ? { base_url: url } : {}),
        ...(Object.keys(firm.value).length ? { enterprise: firm.value } : {}),
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
      const same = twin(key, account.provider_id, urls.get(id) || null, account.enterprise, id);
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
        update(id, { ...stored, key_stored: true, key_saved_at: core.isoNow() });
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
    setEnterprise: enterpriseApi(core, { keys, urls }),
    setModel: (id, model) => {
      const account = core.accounts.find((a) => a.id === id);
      if (!account) return missing();
      const name = model.trim();
      if (!name)
        return Promise.reject(
          new CommandError('invalid_input', 'Podaj nazwę modelu, np. gpt-4o-mini.'),
        );
      const models = [fakeModel(name)];
      return core.reply(update(id, { models, auto_model: name }) ?? account);
    },
    setQueue: (id, taskClass, models) => {
      const account = core.accounts.find((a) => a.id === id);
      if (!account) return missing();
      const refuse = (message: string) =>
        Promise.reject(new CommandError('invalid_input', message));
      const known = TASK_CLASSES.find((c) => c === taskClass);
      if (!known) return refuse(`Nieznana klasa zadań „${taskClass}”.`);
      if (models.length > MAX_QUEUE)
        return refuse(`Kolejka może mieć najwyżej ${MAX_QUEUE} modeli.`);
      const twice = models.find((m, at) => models.indexOf(m) !== at);
      if (twice) return refuse(`Model „${twice}” jest w kolejce dwa razy.`);
      const unfit = models.find((m) => !fitsClass(known, purposeOf(m)));
      if (unfit) return refuse(`„${unfit}” nie nadaje się do tej klasy zadań.`);
      const queues: Record<string, readonly string[]> = { ...account.queues };
      if (models.length) queues[known] = [...models];
      else delete queues[known];
      return core.reply(update(id, { queues }) ?? account);
    },
    // Atrapa: zmienne z katalogu; klucz OpenAI „już jest”, Anthropic „dodany”, reszta nieustawiona.
    importEnv: () =>
      core.reply(
        [
          ['openai', 'OpenAI', 'OPENAI_API_KEY', 'already_present'],
          ['anthropic', 'Anthropic', 'ANTHROPIC_API_KEY', 'imported'],
          ['google', 'Google', 'GEMINI_API_KEY', 'not_set'],
          ['mistral', 'Mistral', 'MISTRAL_API_KEY', 'not_set'],
          ['openrouter', 'OpenRouter', 'OPENROUTER_API_KEY', 'not_set'],
        ].map(([provider, provider_name, name, result]) => ({
          provider: provider ?? '',
          provider_name: provider_name ?? '',
          var: name ?? '',
          result: result as 'imported' | 'already_present' | 'not_set',
          account_id: result === 'not_set' ? null : `acc-${provider}`,
        })),
      ),
    suggestQueue: (id, taskClass, strategy) => {
      const account = core.accounts.find((a) => a.id === id);
      if (!account) return missing();
      return core.reply(
        suggestFake(account.models, taskClass as TaskClassId, strategy as QueueStrategy),
      );
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
