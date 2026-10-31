// Polecenia łącza bez pokrycia do 0.0.64 (234 z 264 przećwiczonych), na PRAWDZIWYM rdzeniu.
// Każde wywołanie sprawdzone w kodzie rdzenia (2026-10-20): odmowa pada przed oknem systemowym
// (`attachments_pick`), schowkiem (`attachments_paste`), kamerą (`cameras_frame`,
// `attachments_add_camera` — nieznana kamera odrzucana w rejestrze, bez otwierania urządzenia),
// terminalem (`agents_open_terminal`, `bridges_open_login`), Brokerem (`gui_desktop_grant`,
// `permissions_request_level`), ustawieniami Windows (`app_open_system_settings`), siecią i
// restartem (`updates_*` tylko przy stanie, w którym rdzeń odmawia) i głośnikiem (`voice_preview`).
// Wyjątek: `quick_expand_to_main` najpierw chowa okno Szybkiego pytania, a dopiero potem odmawia
// dla nieznanej sesji — na instancji testowej to bez znaczenia (okno i tak jest ukryte).
// Polecenia z wynikiem zmieniają wyłącznie profil instancji testowej: konfiguracja kopii (z powrotem
// do stanu wyjściowego), hasło kopii (sejf `Alfa/test/…`), ustawienia głosu na wartości domyślne,
// Szybkie pytanie bez modelu (tura kończy się błędem „brak mózgu”, jak w teście dymnym).
// Bez dźwięku, bez mikrofonu, bez zrzutów ekranu, bez okien systemowych.
import type { Page } from '@playwright/test';
import { expect, test } from './alfa';
import { loadIpcSchema, schemaErrors } from './ipc';
import { call, raw, refused } from './ipc-call';

const schema = loadIpcSchema();

// Uwagi z konsoli (np. z tury „brak mózgu” Szybkiego pytania) trafiają pod nazwę tego testu.
test.beforeEach(({ alfa }, info) => {
  alfa.log.phase = info.title;
});

/**
 * Wywołanie, którego wynik zależy od silników głosu w profilu (bez modeli — odmowa): sukces musi
 * zgadzać się ze schematem polecenia, odmowa — mieć kod i opis.
 */
async function answered(page: Page, command: string, args: Record<string, unknown>) {
  const reply = await raw(page, command, args);
  if (!reply.ok) {
    expect(typeof reply.error.code, `${command}: błąd ma kod`).toBe('string');
    expect((reply.error.message ?? '').length, `${command}: błąd ma opis`).toBeGreaterThan(3);
    return;
  }
  const spec = schema.commands[command];
  if (spec?.result) {
    expect(
      schemaErrors(schema, spec.result, (reply.value ?? null) as never),
      `${command}: schemat`,
    ).toEqual([]);
  }
}

interface BackupConfig {
  enabled: boolean;
  dir: string | null;
  interval_hours: number;
  keep: number;
  include_artifacts: boolean;
  include_logs: boolean;
  skip_on_battery: boolean;
}

interface BackupView {
  config: BackupConfig;
  password_set: boolean;
}

interface UpdatesView {
  phase: string;
  ready: string | null;
  previous: string | null;
}

test('kopie zapasowe: katalog tylko z okna wyboru, za krótkie hasło, kopia bez katalogu', async ({
  alfa,
}) => {
  const { main } = alfa;
  const before = await call<BackupView>(main, 'backups_status');
  const original = before.config;
  // Katalog kopii zmienia wyłącznie okno wyboru (`backups_choose_dir`) — podany tu jest pomijany.
  const changed = await call<BackupView>(main, 'backups_configure', {
    config: {
      ...original,
      dir: 'C:\\alfa-e2e-katalog-z-pominieciem-okna',
      include_logs: !original.include_logs,
    },
  });
  try {
    expect(changed.config.dir).toBe(original.dir);
    expect(changed.config.include_logs).toBe(!original.include_logs);
  } finally {
    // Przywrócenie także po nieudanym sprawdzeniu — profil jest wspólny dla kolejnych plików.
    const restored = await call<BackupView>(main, 'backups_configure', { config: original });
    expect(restored.config).toEqual(original);
  }

  // Hasło krótsze niż 8 znaków — odmowa przed sejfem; brak hasła — kopie bez szyfrowania.
  await refused(main, 'backups_set_password', { password: 'krotkie' });
  const plain = await call<BackupView>(main, 'backups_set_password', { password: null });
  expect(plain.password_set).toBe(false);

  // Świeży profil nie ma katalogu kopii: „Kopia teraz” odmawia, zamiast pisać gdziekolwiek.
  if (original.dir === null) {
    await refused(main, 'backups_run_now', {});
  } else {
    test.info().annotations.push({
      type: 'pominięte',
      description: `backups_run_now: profil ma katalog kopii (${original.dir}) — kopia nie jest robiona`,
    });
  }
});

test('Kreator agentek: szkic z opisu bez modelu, odmowa pustego i za długiego opisu', async ({
  alfa,
}) => {
  const { main } = alfa;
  const proposal = await call<{ draft: { name: string | null }; questions: string[] }>(
    main,
    'builder_propose',
    { description: 'Agentka, która porządkuje pliki w Pobranych i mówi, co zrobiła.' },
  );
  expect(Array.isArray(proposal.questions)).toBe(true);
  await refused(main, 'builder_propose', { description: '   ' });
  await refused(main, 'builder_propose', { description: 'x'.repeat(4_001) });
});

test('Szybkie pytanie bez modelu: tura w sesji szybkich pytań, odmowy dla celów w tej sesji', async ({
  alfa,
}) => {
  const { main } = alfa;
  // Bez modelu i kluczy (profil instancji testowej) odpowiedź kończy się błędem „brak mózgu”.
  const asked = await call<{ session_id: string; user_turn_id: string }>(main, 'quick_ask', {
    text: 'Pytanie testowe E2E — bez modelu.',
  });
  expect(asked.session_id.length).toBeGreaterThan(0);
  expect(asked.user_turn_id.length).toBeGreaterThan(0);
  const sessionId = asked.session_id;

  // Istniejąca sesja, ale cel nie istnieje: krok bez propozycji terminala, nieznana kamera,
  // niepoprawna nazwa agentki — każda odmowa przed terminalem, kamerą i Brokerem.
  await refused(main, 'agents_open_terminal', { stepId: `${sessionId}:r1:s1` });
  await refused(main, 'attachments_add_camera', { sessionId, cameraId: 'brak-kamery-e2e' });
  await refused(main, 'gui_desktop_grant', { sessionId, agent: '../nie-agentka' });
});

test('okna, terminal, kamera, Sędzia Cieni i schowek: odmowa przed skutkiem dla złych celów', async ({
  alfa,
}) => {
  const { main } = alfa;
  const cases: [string, Record<string, unknown>][] = [
    ['agents_open_terminal', { stepId: 'zly-krok' }],
    ['agents_open_terminal', { stepId: 'brak-sesji:r1:s1' }],
    ['app_open_system_settings', { uri: 'ms-settings:e2e-spoza-listy' }],
    ['attachments_pick', { sessionId: 'brak-sesji' }],
    ['attachments_paste', { sessionId: 'brak-sesji' }],
    ['attachments_add_camera', { sessionId: 'brak-sesji', cameraId: 'brak-kamery-e2e' }],
    ['cameras_frame', { id: 'brak-kamery-e2e', maxSide: 64 }],
    ['bridges_open_login', { bridge: 'nieznany-most' }],
    ['gui_desktop_grant', { sessionId: 'brak-sesji', agent: 'alfa' }],
    ['permissions_request_level', { level: 'L1', sessionId: 'zla:sesja' }],
    ['quick_expand_to_main', { sessionId: 'brak-sesji' }],
    ['voice_preview', { agent: 'nie-ma-takiej-agentki' }],
  ];
  for (const [command, args] of cases) {
    await test.step(command, async () => {
      await refused(main, command, args);
    });
  }
  // Ostatni zrzut agentki trzymany tylko w pamięci procesu — bez sterowania pulpitem nie ma żadnego.
  // Polecenie nie robi nowego zrzutu ekranu (`app-gui::GuiApp::screenshot`).
  const shot = await call<unknown>(main, 'gui_screenshot');
  expect(shot).toBeNull();
});

test('aktualizacje: bez dostępnej wersji nie ma pobierania, restartu ani przywrócenia', async ({
  alfa,
}) => {
  const { main } = alfa;
  // Bez sprawdzenia dostępności (sieć — poza testem) rdzeń odmawia; przy innym stanie krok jest
  // pomijany, żeby test nigdy nie pobrał, nie uruchomił ponownie ani nie przywrócił wersji.
  const status = await call<UpdatesView>(main, 'updates_status');
  const skip = (command: string, why: string) =>
    test.info().annotations.push({ type: 'pominięte', description: `${command}: ${why}` });
  if (status.phase !== 'available' && status.phase !== 'failed') {
    await refused(main, 'updates_download', {});
  } else {
    skip('updates_download', `etap ${status.phase}`);
  }
  if (status.ready === null) {
    await refused(main, 'updates_restart', {});
  } else {
    skip('updates_restart', `przygotowana wersja ${status.ready}`);
  }
  if (status.previous === null) {
    await refused(main, 'updates_rollback', {});
  } else {
    skip('updates_rollback', `poprzednia wersja ${status.previous}`);
  }
});

test('głos rozszerzony: wartości domyślne i odmowy bez trwającej funkcji', async ({ alfa }) => {
  const { main } = alfa;
  // Bez urządzeń audio (runner) głos jest niepodłączony i każda akcja odmawia z kodem; z nimi
  // akcje „wyłączające” zwracają stan zgodny ze schematem (`answered`).
  // Puszczenie przycisku mówienia bez rozmowy głosowej — bez skutku.
  await answered(main, 'voice_ptt', { pressed: false });
  // Tryb „nie przeszkadzać” wyłączony (domyślnie); włączenie słów wywoławczych bez modelu,
  // pomiaru i zgody na ryzyko — odmowa.
  await answered(main, 'voice_wake', { action: { kind: 'set_dnd', on: false } });
  await refused(main, 'voice_wake', {
    action: { kind: 'configure', enabled: true, accept_risk: false, owner_gate: false },
  });
  // Weryfikacja głosu nie jest wymagana (domyślnie); anulowanie kreatora, którego nikt nie zaczął.
  await answered(main, 'voice_speaker', { action: { kind: 'set_required', required: false } });
  await answered(main, 'voice_speaker', { action: { kind: 'cancel' } });
  // Dyktowanie: zatrzymanie bez dyktowania jest bez skutku, cofnięcie — odmowa.
  await answered(main, 'voice_dictation', { action: { kind: 'stop' } });
  await refused(main, 'voice_dictation', { action: { kind: 'undo' } });
  // Czytanie na głos: zatrzymanie zawsze się udaje, pauza bez czytania — odmowa.
  await answered(main, 'voice_read', { action: { kind: 'control', control: 'stop' } });
  await refused(main, 'voice_read', { action: { kind: 'control', control: 'pause' } });
});
