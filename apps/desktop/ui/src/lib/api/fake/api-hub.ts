// Atrapa: Hub kont i kluczy oraz import/eksport `.alfa`. Sekret z kreatora jest tylko sprawdzany
// (kształt) i odrzucany — atrapa, tak jak rdzeń, nigdy nie oddaje go z powrotem do UI.
import type { AlfaClient } from '../client';
import { CommandError } from '../command-error';
import type { Account, DryRunItem, ModelInfo } from '../types-hub';
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
  const lower = url.toLowerCase();
  const local = /^http:\/\/(localhost|127\.0\.0\.1|\[::1\])([:/]|$)/.test(lower);
  if ((lower.startsWith('https://') && lower.length > 'https://'.length) || local) return null;
  return `niepoprawne dane: endpoint \`${url}\` musi zaczynać się od https:// (http:// tylko dla localhost)`;
}

export function accountsApi(core: FakeCore): AlfaClient['accounts'] {
  const update = (id: string, patch: Partial<Account>): Account | undefined => {
    const index = core.accounts.findIndex((a) => a.id === id);
    const current = core.accounts[index];
    if (!current) return undefined;
    const next = { ...current, ...patch };
    core.accounts[index] = next;
    core.emit([{ type: 'AccountChanged', account: next }]);
    return next;
  };
  return {
    catalog: () => core.reply(FAKE_CATALOG),
    list: () => core.reply(core.accounts),
    add: (input) => {
      // Te same reguły co rdzeń (accounts-hub `check_new`), z tymi samymi komunikatami.
      const rejected = checkNewAccount(input.provider_id, input.secret, input.base_url);
      if (rejected) return Promise.reject(new CommandError('invalid_input', rejected));
      const account: Account = {
        id: core.nextId('acc'),
        provider_id: input.provider_id,
        label: input.label,
        state: 'unconfigured',
        key_stored: input.secret.trim().length > 0,
        models: [],
        assignments: { task_classes: ['chat'], agents: [], voice_stt: false, voice_tts: false },
        cost_limit: { enabled: true, monthly: { minor: 10_000, currency: 'PLN' } },
        last_tested_at: null,
      };
      core.accounts.push(account);
      if (!core.status.keys_configured)
        core.setStatus({ keys_configured: true, profile: 'hybrid' });
      return core.reply(account);
    },
    test: (id) => {
      const account = core.accounts.find((a) => a.id === id);
      const ok = Boolean(account?.key_stored) && core.status.online;
      const models = ok && account ? genericModels(account.provider_id) : [];
      update(id, { state: ok ? 'ok' : 'invalid', models, last_tested_at: core.isoNow() });
      return core.reply({
        ok,
        latency_ms: ok ? 312 : null,
        models,
        error: ok
          ? null
          : core.status.online
            ? 'Klucz odrzucony (401).'
            : 'Brak połączenia z internetem.',
      });
    },
    assign: (id, assignment) => core.reply(void update(id, { assignments: assignment })),
    setLimit: (id, enabled, monthly) =>
      core.reply(void update(id, { cost_limit: { enabled, monthly } })),
    remove: (id) => {
      core.accounts = core.accounts.filter((a) => a.id !== id);
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
      });
    },
    rollback: () => core.reply(undefined),
  };
}
