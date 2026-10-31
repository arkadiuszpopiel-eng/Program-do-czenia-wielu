import { describe, expect, it } from 'vitest';
import type { Account } from '../../api/types-hub';
import type { ModelItem } from '../../api/types-models';
import type { BridgeAccount, BridgeCard } from '../../api/types-tasks';
import { pl } from '../../i18n/pl';
import { en } from '../../i18n/en';
import {
  accountRow,
  bridgeRow,
  connectionRows,
  connectionSummary,
  engineRow,
  loginResult,
  systemRows,
} from '../connections';

const account = (patch: Partial<Account>): Account =>
  ({
    id: 'a1',
    provider_id: 'openai',
    label: 'OpenAI — praca',
    state: 'ok',
    key_stored: true,
    models: [{ id: 'gpt-6-sol' }, { id: 'gpt-6-luna' }],
    last_tested_at: '2026-10-14T10:00:00Z',
    last_test: null,
    ...patch,
  }) as unknown as Account;

const card = (patch: Partial<BridgeCard>): BridgeCard =>
  ({
    route_id: 'claude-code-cli',
    bridge: 'claude_code',
    name: 'Claude Code (CLI)',
    detected: true,
    version: '2.1.3',
    version_ok: true,
    enabled: true,
    account: null,
    ...patch,
  }) as unknown as BridgeCard;

const acc = (state: BridgeAccount['state'], plan: string | null = null): BridgeAccount => ({
  state,
  plan,
  method: null,
  detail: 'opis rdzenia',
  checked_at: '2026-10-14T19:00:00Z',
});

const engine = (kind: ModelItem['kind'], state: ModelItem['state'], id = kind): ModelItem =>
  ({ id, kind, name: id, state }) as unknown as ModelItem;

describe('stan połączeń', () => {
  it('konto API: działa, klucz odrzucony z powodem, wyłączone, bez klucza', () => {
    const ok = accountRow(account({}));
    expect([ok.state, ok.text, ok.vars.models]).toEqual(['ok', 'conn.account.ok', 2]);
    expect(ok.action).toEqual({ kind: 'test', accountId: 'a1' });
    const bad = accountRow(
      account({ state: 'invalid', last_test: { ok: false, reason: 'no_credit', latency_ms: 1 } }),
    );
    expect([bad.state, bad.text]).toEqual(['attention', 'conn.reason.no_credit']);
    expect(accountRow(account({ state: 'disabled' })).action).toBeNull();
    expect(accountRow(account({ state: 'unconfigured' })).state).toBe('off');
    expect(accountRow(account({ state: 'rate_limited' })).state).toBe('attention');
    expect(accountRow(account({ state: 'testing' })).state).toBe('checking');
  });

  it('most: zalogowany z planem i gotowy — „Alfa może zlecać zadania”', () => {
    const r = bridgeRow(card({ account: acc('logged_in', 'Max') }));
    expect(r?.state).toBe('ok');
    expect(r?.text).toBe('conn.bridge.readyPlan');
    expect(r?.vars.plan).toBe('Max');
    expect(r?.checkedAt).toBe('2026-10-14T19:00:00Z');
  });

  it('most: zalogowany, ale wyłączony albo z niezatwierdzoną wersją', () => {
    expect(bridgeRow(card({ enabled: false, account: acc('logged_in', 'Max') }))?.text).toBe(
      'conn.bridge.loggedInOffPlan',
    );
    const v = bridgeRow(card({ version_ok: false, account: acc('logged_in') }));
    expect([v?.state, v?.text, v?.vars.version]).toEqual([
      'attention',
      'conn.bridge.loggedInVersion',
      '2.1.3',
    ]);
  });

  it('most: niezalogowany → Zaloguj; bez stanu (agy) → Zaloguj; brak narzędzia; nie sprawdzono', () => {
    expect(bridgeRow(card({ account: acc('logged_out') }))?.action).toEqual({
      kind: 'login',
      bridge: 'claude_code',
    });
    const agy = bridgeRow(card({ bridge: 'antigravity', account: acc('unknown') }));
    expect([agy?.state, agy?.text, agy?.action?.kind]).toEqual([
      'unknown',
      'conn.bridge.noStatus',
      'login',
    ]);
    expect(bridgeRow(card({ detected: false }))?.text).toBe('conn.bridge.notInstalled');
    expect(bridgeRow(card({ account: acc('not_installed') }))?.state).toBe('off');
    expect(bridgeRow(card({}))?.text).toBe('conn.bridge.unchecked');
    const err = bridgeRow(card({ account: acc('error') }));
    expect([err?.state, err?.vars.detail]).toEqual(['attention', 'opis rdzenia']);
    expect(bridgeRow(card({ bridge: null }))).toBeNull();
  });

  it('modele i głos: gotowe, uszkodzone, brak — wiersz „brak” ze wskazaniem strony', () => {
    expect(engineRow(engine('llm', 'installed'))?.state).toBe('ok');
    expect(engineRow(engine('stt', 'corrupt'))?.group).toBe('voice');
    expect(engineRow(engine('llm', 'missing'))).toBeNull();
    const rows = connectionRows({
      accounts: [],
      bridges: [],
      engines: [engine('llm', 'missing'), engine('sidecar', 'installed')],
      broker: null,
      system: null,
    });
    expect(rows.map((r) => r.id)).toEqual(['engine:local-none', 'engine:voice-none']);
    expect(rows[0]?.nameKey).toBe('conn.local.name');
  });

  it('Sędzia Cieni, sieć, mikrofon', () => {
    const rows = systemRows(
      {
        mode: 'in_process',
        state: 'connected',
        approval_window: true,
        watchdog: false,
        isolated: false,
        detail: null,
      },
      {
        online: false,
        queued_messages: 3,
        rate_limit: null,
        keys_configured: true,
        profile: 'hybrid',
        mic: 'denied',
        disk: { free_bytes: 1, low: false },
      } as never,
    );
    expect(rows.map((r) => [r.id, r.state, r.text])).toEqual([
      ['system:broker', 'attention', 'conn.broker.in_process'],
      ['system:network', 'attention', 'conn.network.offline'],
      ['system:mic', 'attention', 'conn.mic.denied'],
    ]);
  });

  it('podsumowanie i „sprawdzam”', () => {
    const rows = connectionRows({
      accounts: [account({}), account({ id: 'a2', state: 'invalid' })],
      bridges: [card({ account: acc('logged_in', 'Max') })],
      engines: null,
      broker: null,
      system: null,
      checking: new Set(['account:a2']),
    });
    expect(rows.find((r) => r.id === 'account:a2')?.state).toBe('checking');
    expect(connectionSummary(rows)).toEqual({ ok: 2, attention: 0, off: 0, unknown: 1 });
  });

  it('komunikat po logowaniu', () => {
    const ok = loginResult(card({ account: acc('logged_in', 'Max') }));
    expect([ok.kind, ok.key, ok.withPlan]).toEqual(['success', 'conn.loginResult.ok', true]);
    expect(loginResult(card({ enabled: false, account: acc('logged_in') })).key).toBe(
      'conn.loginResult.off',
    );
    expect(loginResult(card({ account: acc('logged_out') })).kind).toBe('warning');
  });

  it('każdy klucz tekstu wiersza istnieje w obu językach', () => {
    const keys = new Set<string>();
    const states: BridgeAccount['state'][] = [
      'logged_in',
      'logged_out',
      'unknown',
      'not_installed',
      'error',
    ];
    for (const s of states) {
      for (const plan of [null, 'Max']) {
        for (const enabled of [true, false]) {
          for (const version_ok of [true, false]) {
            const r = bridgeRow(card({ enabled, version_ok, account: acc(s, plan) }));
            if (r) keys.add(r.text);
          }
        }
      }
    }
    for (const st of ['ok', 'testing', 'disabled', 'unconfigured', 'rate_limited'] as const) {
      keys.add(accountRow(account({ state: st })).text);
    }
    for (const reason of [
      'ok',
      'invalid_key',
      'restricted',
      'no_credit',
      'rate_limited',
      'timeout',
      'network',
      'unsupported',
    ]) {
      keys.add(`conn.reason.${reason}`);
    }
    for (const st of ['paused', 'needs_trust', 'corrupt', 'downloading', 'installed'] as const) {
      const r = engineRow(engine('llm', st));
      if (r) keys.add(r.text);
    }
    for (const k of ['service', 'portable', 'in_process', 'unavailable', 'connecting', 'lost'])
      keys.add(`conn.broker.${k}`);
    for (const k of ['ok', 'denied', 'missing']) keys.add(`conn.mic.${k}`);
    for (const k of ['ok', 'attention', 'off', 'unknown', 'checking']) keys.add(`conn.state.${k}`);
    for (const k of keys) {
      expect(pl, k).toHaveProperty([k]);
      expect(en, k).toHaveProperty([k]);
    }
  });
});
