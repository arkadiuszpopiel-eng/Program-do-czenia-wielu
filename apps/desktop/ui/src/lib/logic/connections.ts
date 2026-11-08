// Stan połączeń (Ustawienia → Stan połączeń): jedno miejsce, w którym widać, co jest zalogowane
// i działa — konta API, mosty CLI (Claude Code, Codex, Antigravity), modele lokalne, silniki głosu,
// Broker i sieć. Tu tylko czyste przekształcenie danych z istniejących poleceń w wiersze; teksty
// wierszy to klucze i18n z parametrami (`conn.*`).
import type { Account } from '../api/types-hub';
import type { BrokerStatusView } from '../api/types-broker';
import type { ModelItem, ModelItemKind } from '../api/types-models';
import type { SystemStatus } from '../api/types-system';
import type { BridgeCard } from '../api/types-tasks';

/** Stan wiersza: działa · wymaga uwagi · wyłączone/brak · nie sprawdzono · sprawdzam. */
export type ConnState = 'ok' | 'attention' | 'off' | 'unknown' | 'checking';

export type ConnGroupId = 'accounts' | 'bridges' | 'local' | 'voice' | 'system';

/** Co można zrobić z wierszem. */
export type ConnAction =
  | { readonly kind: 'test'; readonly accountId: string }
  | { readonly kind: 'check'; readonly bridge: string }
  | { readonly kind: 'login'; readonly bridge: string }
  | { readonly kind: 'open'; readonly page: string };

export interface ConnRow {
  readonly id: string;
  readonly group: ConnGroupId;
  /** Nazwa z danych (konto, model); gdy jest `nameKey`, nazwę daje tłumaczenie. */
  readonly name: string;
  readonly nameKey?: string;
  readonly state: ConnState;
  /** Klucz i18n zdania o stanie i jego parametry. */
  readonly text: string;
  readonly vars: Readonly<Record<string, string | number>>;
  /** Kiedy ostatnio sprawdzono (ISO) — `null`, gdy nie dotyczy albo nie sprawdzano. */
  readonly checkedAt: string | null;
  /** Główna akcja (sprawdź / zaloguj / przejdź do strony). */
  readonly action: ConnAction | null;
  /** Strona Ustawień ze szczegółami. */
  readonly page: string;
}

export interface ConnSources {
  readonly accounts: readonly Account[] | null;
  readonly bridges: readonly BridgeCard[] | null;
  readonly engines: readonly ModelItem[] | null;
  readonly broker: BrokerStatusView | null;
  readonly system: SystemStatus | null;
  /** Wiersze w trakcie sprawdzania (identyfikatory `ConnRow.id`). */
  readonly checking?: ReadonlySet<string>;
}

const row = (r: Omit<ConnRow, 'vars' | 'checkedAt'> & Partial<ConnRow>): ConnRow => ({
  vars: {},
  checkedAt: null,
  ...r,
});

/** Konto API: stan z ostatniego testu klucza. */
export function accountRow(a: Account): ConnRow {
  const base = {
    id: `account:${a.id}`,
    group: 'accounts' as const,
    name: a.label,
    checkedAt: a.last_tested_at,
    action: { kind: 'test', accountId: a.id } as const,
    page: 'providers',
  };
  const models = a.models.length;
  switch (a.state) {
    case 'ok':
      return row({ ...base, state: 'ok', text: 'conn.account.ok', vars: { models } });
    case 'testing':
      return row({ ...base, state: 'checking', text: 'conn.account.testing' });
    case 'disabled':
      return row({ ...base, state: 'off', text: 'conn.account.disabled', action: null });
    case 'unconfigured':
      return row({ ...base, state: 'off', text: 'conn.account.noKey', action: null });
    case 'rate_limited':
      return row({ ...base, state: 'attention', text: 'conn.account.rateLimited' });
    case 'invalid': {
      const reason = a.last_test?.reason ?? 'invalid_key';
      return row({ ...base, state: 'attention', text: `conn.reason.${reason}` });
    }
  }
}

/** Most CLI: zainstalowany? zalogowany (plan)? włączony i z zatwierdzoną wersją? */
export function bridgeRow(c: BridgeCard): ConnRow | null {
  const bridge = c.bridge;
  if (!bridge) return null;
  const base = {
    id: `bridge:${bridge}`,
    group: 'bridges' as const,
    name: c.name,
    checkedAt: c.account?.checked_at ?? null,
    page: 'bridges',
  };
  const check = { kind: 'check', bridge } as const;
  const login = { kind: 'login', bridge } as const;
  const account = c.account;
  if (!c.detected || account?.state === 'not_installed') {
    return row({ ...base, state: 'off', text: 'conn.bridge.notInstalled', action: check });
  }
  if (!account) {
    return row({ ...base, state: 'unknown', text: 'conn.bridge.unchecked', action: check });
  }
  const plan = account.plan ?? '';
  switch (account.state) {
    case 'logged_out':
      return row({ ...base, state: 'attention', text: 'conn.bridge.loggedOut', action: login });
    case 'error':
      return row({
        ...base,
        state: 'attention',
        text: 'conn.bridge.error',
        vars: { detail: account.detail },
        action: check,
      });
    case 'unknown':
      return row({ ...base, state: 'unknown', text: 'conn.bridge.noStatus', action: login });
    case 'logged_in': {
      const who = plan ? 'Plan' : '';
      if (!c.enabled) {
        return row({
          ...base,
          state: 'off',
          text: `conn.bridge.loggedInOff${who}`,
          vars: { plan },
          action: { kind: 'open', page: 'bridges' },
        });
      }
      if (!c.version_ok) {
        return row({
          ...base,
          state: 'attention',
          text: `conn.bridge.loggedInVersion${who}`,
          vars: { plan, version: c.version ?? '—' },
          action: { kind: 'open', page: 'bridges' },
        });
      }
      return row({
        ...base,
        state: 'ok',
        text: `conn.bridge.ready${who}`,
        vars: { plan },
        action: check,
      });
    }
  }
}

const LOCAL_KINDS: readonly ModelItemKind[] = ['llm', 'embed'];
const VOICE_KINDS: readonly ModelItemKind[] = ['stt', 'tts', 'vad', 'wake', 'speaker'];

/** Model albo silnik: tylko pozycje, które są na komputerze albo mają kłopot. */
export function engineRow(m: ModelItem): ConnRow | null {
  const group = VOICE_KINDS.includes(m.kind) ? 'voice' : 'local';
  const base = {
    id: `engine:${m.id}`,
    group,
    name: m.name,
    page: 'models',
    action: { kind: 'open', page: 'models' },
  } as const;
  switch (m.state) {
    case 'installed':
    case 'external':
      return row({ ...base, state: 'ok', text: 'conn.engine.ready', action: null });
    case 'downloading':
    case 'queued':
    case 'installing':
      return row({ ...base, state: 'checking', text: 'conn.engine.downloading' });
    case 'paused':
      return row({ ...base, state: 'attention', text: 'conn.engine.paused' });
    case 'needs_trust':
      return row({ ...base, state: 'attention', text: 'conn.engine.needsTrust' });
    case 'corrupt':
      return row({ ...base, state: 'attention', text: 'conn.engine.corrupt' });
    default:
      return null;
  }
}

/** Gdy w grupie nie ma żadnej pozycji — jeden wiersz „brak” ze wskazaniem strony. */
function missing(group: 'local' | 'voice'): ConnRow {
  return row({
    id: `engine:${group}-none`,
    group,
    name: '',
    nameKey: `conn.${group}.name`,
    state: 'off',
    text: `conn.${group}.none`,
    action: { kind: 'open', page: 'models' },
    page: 'models',
  });
}

/** Broker (strażnik zgód) i sieć. */
export function systemRows(broker: BrokerStatusView | null, sys: SystemStatus | null): ConnRow[] {
  const out: ConnRow[] = [];
  if (broker) {
    const connected = broker.state === 'connected';
    const state: ConnState = !connected
      ? 'attention'
      : broker.mode === 'in_process'
        ? 'attention'
        : 'ok';
    out.push(
      row({
        id: 'system:broker',
        group: 'system',
        name: '',
        nameKey: 'conn.broker.name',
        state,
        text: connected ? `conn.broker.${broker.mode}` : `conn.broker.${broker.state}`,
        action: { kind: 'open', page: 'permissions' },
        page: 'permissions',
      }),
    );
  }
  if (sys) {
    out.push(
      row({
        id: 'system:network',
        group: 'system',
        name: '',
        nameKey: 'conn.network.name',
        state: !sys.online || sys.rate_limit ? 'attention' : 'ok',
        text: !sys.online
          ? 'conn.network.offline'
          : sys.rate_limit
            ? 'conn.network.rateLimit'
            : 'conn.network.online',
        vars: { queued: sys.queued_messages, provider: sys.rate_limit?.provider ?? '' },
        action: null,
        page: 'general',
      }),
      row({
        id: 'system:mic',
        group: 'system',
        name: '',
        nameKey: 'conn.mic.name',
        state: sys.mic === 'ok' ? 'ok' : 'attention',
        text: `conn.mic.${sys.mic}`,
        action: sys.mic === 'ok' ? null : { kind: 'open', page: 'voice' },
        page: 'voice',
      }),
    );
  }
  return out;
}

/** Wszystkie wiersze w stałej kolejności grup; źródło `null` (nie wczytane) daje zero wierszy. */
export function connectionRows(src: ConnSources): ConnRow[] {
  const rows: ConnRow[] = [];
  for (const a of src.accounts ?? []) rows.push(accountRow(a));
  for (const c of src.bridges ?? []) {
    const r = bridgeRow(c);
    if (r) rows.push(r);
  }
  if (src.engines) {
    const engines = src.engines.filter((m) => m.kind !== 'sidecar');
    for (const group of ['local', 'voice'] as const) {
      const kinds = group === 'local' ? LOCAL_KINDS : VOICE_KINDS;
      const found = engines
        .filter((m) => kinds.includes(m.kind))
        .map(engineRow)
        .filter((r): r is ConnRow => r !== null);
      rows.push(...(found.length ? found : [missing(group)]));
    }
  }
  rows.push(...systemRows(src.broker, src.system));
  const checking = src.checking;
  return checking?.size
    ? rows.map((r) => (checking.has(r.id) ? { ...r, state: 'checking' as const } : r))
    : rows;
}

export interface ConnSummary {
  readonly ok: number;
  readonly attention: number;
  readonly off: number;
  readonly unknown: number;
}

/** Liczniki do nagłówka („5 działa · 1 wymaga uwagi …”); „sprawdzam” liczy się jak nieznane. */
export function connectionSummary(rows: readonly ConnRow[]): ConnSummary {
  const s = { ok: 0, attention: 0, off: 0, unknown: 0 };
  for (const r of rows) {
    if (r.state === 'checking') s.unknown += 1;
    else s[r.state] += 1;
  }
  return s;
}

export const CONN_GROUPS: readonly ConnGroupId[] = [
  'accounts',
  'bridges',
  'local',
  'voice',
  'system',
];

/** Komunikat po sprawdzeniu logowania mostu (po „Gotowe” w oknie logowania). */
export function loginResult(c: BridgeCard): {
  readonly kind: 'success' | 'warning';
  readonly key: string;
  readonly vars: Readonly<Record<string, string>>;
  readonly withPlan: boolean;
} {
  const a = c.account;
  const plan = a?.plan ?? '';
  if (a?.state === 'logged_in') {
    const key = c.enabled ? 'conn.loginResult.ok' : 'conn.loginResult.off';
    return {
      kind: c.enabled ? 'success' : 'warning',
      key,
      vars: { name: c.name, plan },
      withPlan: plan !== '',
    };
  }
  return {
    kind: 'warning',
    key: 'conn.loginResult.bad',
    vars: { name: c.name, detail: a?.detail ?? '' },
    withPlan: false,
  };
}
