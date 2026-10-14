// Kolejne polecenia łącza na PRAWDZIWYM rdzeniu (cel pokrycia 70 %): pełne cykle kont API, mostów
// CLI i obsady agentek, a dla poleceń, które wymagają danych z działania programu (pamięć,
// umiejętności, wtyczki, naprawy, modele), czytelna odmowa z kodem dla nieistniejącego celu —
// błąd ma przejść przez uprawnienia okna, łącze i rdzeń, a nie zginąć po drodze.
// Bez okien systemowych, bez uruchamiania programów, bez sieci: konto testowe wskazuje adres
// lokalny, który odmawia połączenia, a klucz jest fałszywy (profil instancji testowej).
import { expect, test } from './alfa';
import { call, refused, settled } from './ipc-call';

interface Account {
  id: string;
  state: string;
  disabled?: boolean;
  queues?: Record<string, string[]>;
  assignments: { task_classes: string[]; agents: string[]; voice_stt: boolean; voice_tts: boolean };
}

test('konta API: dodanie, model, wyłączenie, limit, przypisania, kolejki, usunięcie', async ({
  alfa,
}) => {
  const { main } = alfa;
  const account = await call<Account>(main, 'accounts_add', {
    input: {
      provider_id: 'custom-openai-compatible',
      label: 'Konto próbne e2e',
      secret: 'alfa-e2e-falszywy-klucz-0001',
      base_url: 'http://127.0.0.1:9/v1',
    },
  });
  const accountId = account.id;
  // Ten sam klucz drugi raz — odmowa z nazwą istniejącego konta.
  await refused(main, 'accounts_add', {
    input: {
      provider_id: 'custom-openai-compatible',
      label: 'Duplikat',
      secret: 'alfa-e2e-falszywy-klucz-0001',
      base_url: 'http://127.0.0.1:9/v1',
    },
  });
  await call(main, 'accounts_set_model', { accountId, model: 'model-probny' });
  const off = await call<Account>(main, 'accounts_set_disabled', { accountId, disabled: true });
  expect(off.state).toBe('disabled');
  await call(main, 'accounts_set_disabled', { accountId, disabled: false });
  await call(main, 'accounts_set_limit', {
    accountId,
    enabled: true,
    monthly: { minor: 5_000, currency: 'PLN' },
  });
  await call(main, 'accounts_assign', {
    accountId,
    assignment: { task_classes: ['chat', 'code'], agents: [], voice_stt: false, voice_tts: false },
  });
  const queued = await call<Account>(main, 'accounts_set_queue', {
    accountId,
    taskClass: 'chat',
    models: ['model-probny'],
  });
  expect(queued.queues?.['chat']).toEqual(['model-probny']);
  const suggested = await call<string[]>(main, 'accounts_suggest_queue', {
    accountId,
    taskClass: 'chat',
    strategy: 'balanced',
  });
  expect(Array.isArray(suggested)).toBe(true);
  await call(main, 'accounts_shared_queues');
  await call(main, 'accounts_set_shared_queue', {
    taskClass: 'chat',
    entries: [{ account_id: accountId, model: 'model-probny' }],
  });
  await call(main, 'accounts_set_shared_queue', { taskClass: 'chat', entries: [] });
  // Instancja testowa nie ma kluczy w zmiennych środowiska — raport bez importu.
  const imported = await call<{ result: string }[]>(main, 'accounts_import_env');
  expect(imported.every((r) => r.result !== 'imported')).toBe(true);
  await call(main, 'accounts_remove', { accountId });
  await refused(main, 'accounts_set_model', { accountId, model: 'x' });
});

test('mosty CLI: włącznik trasy, zgoda na harmonogram, przypięcie, stan logowania', async ({
  alfa,
}) => {
  const { main } = alfa;
  const on = await call<{ enabled: boolean }>(main, 'bridges_set_enabled', {
    routeId: 'codex-cli',
    enabled: true,
  });
  expect(on.enabled).toBe(true);
  await call(main, 'bridges_set_enabled', { routeId: 'codex-cli', enabled: false });
  // Trasa zabroniona nie daje się włączyć.
  await refused(main, 'bridges_set_enabled', { routeId: 'qwen-coding-plan', enabled: true });
  const consent = await call<{ schedule_per_day: number }>(main, 'bridges_set_schedule', {
    bridge: 'claude_code',
    perDay: 99,
  });
  expect(consent.schedule_per_day).toBe(24);
  await call(main, 'bridges_set_schedule', { bridge: 'claude_code', perDay: 0 });
  await call(main, 'bridges_pin', { bridge: 'claude_code', version: null });
  await refused(main, 'bridges_pin', { bridge: 'claude_code', version: '0.0.0-nieistniejaca' });
  // Stan logowania: polecenie stanu narzędzia (jeśli jest na komputerze) — tylko stan i plan.
  const card = await call<{ account: { state: string; detail: string } | null }>(
    main,
    'bridges_check',
    { bridge: 'antigravity' },
  );
  expect(['unknown', 'not_installed']).toContain(card.account?.state);
  await refused(main, 'bridges_check', { bridge: 'nieznany' });
});

test('obsada agentek i katalog roboczy sesji', async ({ alfa }) => {
  const { main } = alfa;
  const session = await call<{ id: string }>(main, 'sessions_create', { template: 'empty' });
  const sessionId = session.id;
  await call(main, 'agents_apply_cast', { sessionId, template: 'coding' });
  const agents = await call<{ id: string; role_ids: string[] }[]>(main, 'agents_list', {
    sessionId,
  });
  const delta = agents.find((a) => a.id === 'delta') ?? agents[0];
  expect(delta).toBeTruthy();
  await call(main, 'agents_set_roles', {
    sessionId,
    agent: delta?.id,
    roleIds: delta?.role_ids ?? [],
  });
  await call(main, 'sessions_choose_workdir', { sessionId, choice: 'none' });
  await settled(main, 'turns_stop', { sessionId });
  await call(main, 'sessions_remove', { sessionId });
});

test('polecenia na nieistniejących celach kończą się czytelną odmową z kodem', async ({ alfa }) => {
  const { main } = alfa;
  const missing: [string, Record<string, unknown>][] = [
    ['memory_approve', { entryId: 'brak-wpisu' }],
    [
      'memory_edit',
      { entryId: 'brak-wpisu', edit: { text: 'x', subject: null, confidence: null } },
    ],
    ['memory_promote', { entryId: 'brak-wpisu', to: { kind: 'global', id: null } }],
    ['memory_undo', { scope: 'global', changeId: 'brak-zmiany' }],
    ['memory_forget_preview', { target: { target: 'entry', id: 'brak-wpisu' } }],
    ['memory_forget', { target: { target: 'entry', id: 'brak-wpisu' } }],
    ['skills_approve', { skillId: 'brak', version: '1.0.0', hash: 'x' }],
    ['skills_reject', { skillId: 'brak', version: '1.0.0' }],
    ['skills_disable', { skillId: 'brak' }],
    ['plugins_reject', { pluginId: 'brak', version: '1.0.0' }],
    ['marshal_approve', { proposalId: 999_999 }],
    ['health_reject', { repairId: 999_999 }],
    ['improver_reject', { proposalId: 999_999 }],
    ['files_preview', { artifactId: 'brak-pliku' }],
    ['cameras_rename', { id: 'brak-kamery', name: 'x' }],
    ['terminal_resize', { terminal: 999_999, cols: 80, rows: 24 }],
    ['turns_rate', { turnId: 'brak-tury', rating: 'up' }],
    ['turns_set_hidden', { turnId: 'brak-tury', hidden: true }],
    ['turns_remember', { turnId: 'brak-tury', scope: 'session' }],
    ['models_repair', { itemId: 'brak-pozycji' }],
    ['models_trust_hash', { itemId: 'brak-pozycji', hashes: {} }],
    ['models_bundle_verify', { bundleId: 'brak-pakietu' }],
    ['backups_verify', { file: 'brak-kopii.alfa' }],
  ];
  for (const [command, args] of missing) {
    await test.step(command, async () => {
      await refused(main, command, args);
    });
  }
  // Uszkodzony moduł wtyczki: raport z opisem błędu (nie odmowa), bez instalowania czegokolwiek.
  const inspected = await call<{ ok: boolean; error: string | null }>(main, 'plugins_inspect', {
    wasmB64: 'AAAA',
  });
  expect(inspected.ok).toBe(false);
  expect((inspected.error ?? '').length).toBeGreaterThan(3);
  // Polecenia bez celu, które mogą się udać albo odmówić (zależnie od stanu profilu).
  await settled(main, 'marshal_reject', { proposalId: 999_999 });
  await settled(main, 'updates_cancel', {});
  await settled(main, 'models_local_cancel', { modelId: null });
});
