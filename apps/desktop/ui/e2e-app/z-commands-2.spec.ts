// Polecenia łącza bez pokrycia do 0.0.63 (198 z 264 przećwiczonych), na PRAWDZIWYM rdzeniu.
// Odmowy dla nieistniejących celów: sprawdzone w kodzie rdzenia (2026-10-18), że odmowa pada przed
// oknem systemowym (`turns_save_code`), Brokerem (`turns_run_code`), siecią (`accounts_test`,
// `models_*_download`), magazynem sekretów (`accounts_rotate`) i otwarciem pliku (`files_act`).
// Polecenia z wynikiem działają tylko w profilu instancji testowej: Kreator agentek (podgląd, test
// na sucho, zapis tego samego hasha) i nadpisanie skrótu zmyślonej akcji z powrotem do domyślnego.
// Bez dźwięku, bez zrzutów ekranu, bez okien systemowych.
import { expect, test } from './alfa';
import { call, refused, settled } from './ipc-call';

/** Ten sam szkic co w `crates/app-core/tests/computer.rs` (`fn draft`). */
const DRAFT = {
  id: null,
  name: 'Zofia',
  forms: null,
  glyph: 'Z',
  color: 'color.agent.custom-2',
  character: 'Spokojna, konkretna.',
  voice: {
    base: 'pl-f2',
    pitch: 1.08,
    rate: 1.0,
    perceived_age: 22,
    timbre: 'ciepła',
    design_prompt: 'młoda dorosła kobieta, ok. 22 lat, ciepła, czysta polszczyzna',
  },
  role: {
    id: 'porzadkowa',
    name: 'Porządkowa',
    description: 'Porządkuje Pobrane.',
    prompt: 'Porządkuję pliki i mówię, co zrobiłam.',
    model_policy: 'conversation',
    tools: ['fs'],
    read_only: false,
    untrusted_isolated: false,
    author: false,
  },
  limits: {
    autonomy: 'L2',
    budget: null,
    fs_write: ['%USERPROFILE%\\Downloads\\**'],
    memory_scope: 'agent',
    retain_days: 30,
    triggers: [],
  },
  skills: [],
};

test('Kreator agentek: podgląd, test na sucho i zapis tego samego szkicu', async ({ alfa }) => {
  const { main } = alfa;
  const preview = await call<{ hash: string; forms: unknown[] }>(main, 'builder_preview', {
    draft: DRAFT,
  });
  expect(preview.forms).toHaveLength(7);
  const dry = await call<{ hash: string; passed: boolean }>(main, 'builder_dry_run', {
    draft: DRAFT,
  });
  expect(dry.hash).toBe(preview.hash);
  expect(dry.passed).toBe(true);
  // Zapis innego hasha niż przetestowany — odmowa; tego samego — agentka w bibliotece profilu testowego.
  await refused(main, 'builder_save', { draft: DRAFT, hash: 'inny-hash' });
  const saved = await call<{ persona: string }>(main, 'builder_save', {
    draft: DRAFT,
    hash: dry.hash,
  });
  expect(saved.persona.length).toBeGreaterThan(0);
});

test('skróty: nadpisanie, zakazy (AltGr, STOP WSZYSTKIEGO) i powrót do domyślnego', async ({
  alfa,
}) => {
  const { main } = alfa;
  // Zmyślona akcja — skróty używane przez interfejs i kolejne testy zostają bez zmian.
  const actionId = 'e2e.proba';
  await call(main, 'settings_set_shortcut', { actionId, chord: 'Ctrl+Shift+Y' });
  await refused(main, 'settings_set_shortcut', { actionId, chord: 'Ctrl+Alt+S' });
  await refused(main, 'settings_set_shortcut', { actionId, chord: 'Ctrl+Shift+F12' });
  await refused(main, 'settings_set_shortcut', { actionId: ' ', chord: null });
  await call(main, 'settings_set_shortcut', { actionId, chord: '' });
  await call(main, 'settings_set_shortcut', { actionId, chord: null });
});

test('kolejne polecenia na nieistniejących celach kończą się czytelną odmową z kodem', async ({
  alfa,
}) => {
  const { main } = alfa;
  const missing: [string, Record<string, unknown>][] = [
    ['turns_continue', { sessionId: 'brak-sesji', turnId: 'brak-tury' }],
    ['turns_edit_and_resend', { sessionId: 'brak-sesji', turnId: 'brak-tury', text: 'x' }],
    ['turns_regenerate', { sessionId: 'brak-sesji', turnId: 'brak-tury', profile: null }],
    ['turns_run_code', { turnId: 'brak-tury', blockIndex: 0 }],
    ['turns_save_code', { turnId: 'brak-tury', blockIndex: 0 }],
    ['turns_undo_step', { undoToken: 'brak-sesji:u1' }],
    ['plugins_approve', { pluginId: 'brak', version: '1.0.0', reviewedHash: 'x' }],
    ['plugins_enable', { pluginId: 'brak', reviewedHash: 'x' }],
    ['plugins_disable', { pluginId: 'brak' }],
    ['skills_release', { skillId: 'brak', version: '1.0.0', hash: 'x' }],
    ['skills_run', { skillId: 'brak', sessionId: 'brak-sesji', agent: null, params: {} }],
    ['improver_approve', { proposalId: 999_999, digest: 'x' }],
    ['improver_rollback', { proposalId: 999_999 }],
    ['transfer_rollback', { snapshotId: 'brak-migawki' }],
    ['accounts_test', { accountId: 'brak-konta' }],
    ['accounts_rotate', { accountId: 'brak-konta', secret: 'alfa-e2e-falszywy-klucz-0002' }],
    ['agents_steer', { sessionId: 'brak-sesji', text: 'x' }],
    ['models_local_download', { modelId: 'brak-modelu' }],
    ['models_bundle_download', { bundleId: 'brak-pakietu' }],
    ['files_act', { artifactId: 'brak-pliku', action: 'open' }],
    ['bridges_mark_login', { bridge: 'nieznany', plan: null }],
  ];
  for (const [command, args] of missing) {
    await test.step(command, async () => {
      await refused(main, command, args);
    });
  }
  // Usunięcie czegoś, czego nie ma, może być bez skutku albo odmową — nigdy błędem bez opisu.
  await settled(main, 'plugins_remove', { pluginId: 'brak' });
});

test('eksport, paczki, kopie, załączniki i propozycje: odmowa przed oknem systemowym', async ({
  alfa,
}) => {
  const { main } = alfa;
  // Sprawdzone w kodzie: sesja albo uchwyt są weryfikowane przed oknem zapisu/otwarcia,
  // a niepoprawny manifest — przed modułem wtyczek i umiejętności.
  const missing: [string, Record<string, unknown>][] = [
    ['sessions_export', { sessionId: 'brak-sesji' }],
    ['sessions_export_conversation', { sessionId: 'brak-sesji', format: 'markdown', turnId: null }],
    ['transfer_inspect', { password: null, handle: 'brak-uchwytu' }],
    [
      'transfer_import',
      { request: { handle: 'brak-uchwytu', mode: 'add', resolutions: {}, password: null } },
    ],
    ['backups_restore', { file: 'brak-kopii.alfa' }],
    ['attachments_remove', { sessionId: 'brak-sesji', attachmentId: 'brak' }],
    ['attachments_add_dropped', { sessionId: 'brak-sesji' }],
    ['permissions_open_approval', { approvalId: 'brak' }],
    ['permissions_revoke_grant', { grantId: 'brak' }],
    ['skills_propose', { skill: {} }],
    ['plugins_propose', { manifest: {}, wasmB64: 'AAAA' }],
  ];
  for (const [command, args] of missing) {
    await test.step(command, async () => {
      await refused(main, command, args);
    });
  }
  // Lista zgód „zawsze w tym zakresie”: świeży profil testowy — lista albo opisana odmowa.
  await settled(main, 'permissions_grants', {});
  // Wyłączenie mikrofonu rozmowy: bez silników głosu odmowa, z nimi — mikrofon wyłączony.
  await settled(main, 'voice_set_mic_enabled', { enabled: false });
});
