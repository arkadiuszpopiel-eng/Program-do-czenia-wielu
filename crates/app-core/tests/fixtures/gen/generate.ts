// Generator fixture'ów JSON dla testów round-trip DTO w `app-core` (tests/dto_roundtrip.rs).
// Uruchamia atrapę UI (`FakeAlfaClient`, wirtualny zegar) i dla każdej komendy z COMMANDS.md zapisuje
// argumenty dokładnie tak, jak wysyła je `TauriAlfaClient` (camelCase), oraz wynik atrapy; osobno —
// próbki zdarzeń `alfa://events`. Instrukcja uruchomienia: crates/app-core/README.md.
import { TauriAlfaClient } from '../../../../../apps/desktop/ui/src/lib/api/tauri-client';
import {
  FakeAlfaClient,
  VirtualScheduler,
} from '../../../../../apps/desktop/ui/src/lib/api/fake/fake-client';
import type { AlfaEvent } from '../../../../../apps/desktop/ui/src/lib/api/types-system';
import { runComputer } from './generate-computer';
import { runPlugins } from './generate-plugins';
import { runModels } from './generate-models';
import { runFiles } from './generate-files';
import { runBroker } from './generate-broker';
import { runUpdates } from './generate-updates';
import { runVoice } from './generate-voice';
import { runWork } from './generate-work';
import { invocations } from './tauri-mock';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

interface Entry {
  readonly command: string;
  readonly args: Record<string, unknown>;
  readonly result: unknown;
}

const out = process.argv[2];
if (!out) throw new Error('użycie: node generate.js <katalog wyjściowy>');

const scheduler = new VirtualScheduler();
const fake = new FakeAlfaClient({ scheduler });
const tauri = new TauriAlfaClient();
const events: AlfaEvent[] = [];
fake.subscribe((batch) => events.push(...batch));
const entries = new Map<string, Entry[]>();
const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

type Api = Record<string, (...args: never[]) => Promise<unknown>>;

async function both(ns: string, method: string, ...args: unknown[]): Promise<unknown> {
  const f = (fake as unknown as Record<string, Api>)[ns]?.[method];
  const t = (tauri as unknown as Record<string, Api>)[ns]?.[method];
  if (!f || !t) throw new Error(`brak metody ${ns}.${method}`);
  const result = await (f as (...a: unknown[]) => Promise<unknown>)(...args);
  await (t as (...a: unknown[]) => Promise<unknown>)(...args);
  const call = invocations[invocations.length - 1];
  if (!call) throw new Error('brak wywołania invoke');
  const list = entries.get(ns) ?? [];
  list.push({ command: call.command, args: call.args, result: result ?? null });
  entries.set(ns, list);
  return result;
}

async function run(): Promise<void> {
  await both('app', 'bootstrap');
  await both('app', 'completeOnboarding');
  await both('app', 'openSystemSettings', 'ms-settings:privacy-microphone');
  await both('app', 'saveLayout', {
    left_width: 260,
    right_width: 380,
    left_collapsed: false,
    sessions: { 's-q3': { left_open: true, right_open: true, right_tab: 'timeline' } },
  });
  await both('app', 'setActiveSession', 's-q3');
  await both('app', 'setActiveSession', null);

  await both('sessions', 'list');
  const created = (await both('sessions', 'create', 'coding')) as { id: string };
  await both('sessions', 'rename', created.id, 'API płatności — refaktor');
  await both('sessions', 'setPinned', created.id, true);
  await both('sessions', 'setArchived', created.id, false);
  const ticket = (await both('sessions', 'remove', created.id)) as { token: string };
  await both('sessions', 'undoRemove', ticket.token);
  await both('sessions', 'duplicateAsTemplate', 's-q3');
  await both('sessions', 'exportSession', 's-q3');
  await both('sessions', 'markRead', 's-shop');
  await both('sessions', 'saveDraft', 's-q3', 'Szkic: dopisz wnioski');
  await both('sessions', 'getDraft', 's-q3');
  await both('sessions', 'workdir', 's-q3');
  await both('sessions', 'chooseWorkdir', 's-trip', 'dialog');
  await both('sessions', 'chooseWorkdir', 's-trip', 'default');
  await both('sessions', 'chooseWorkdir', 's-trip', 'none');

  await both('turns', 'list', 's-q3');
  const sent = (await both('turns', 'send', 's-q3', {
    parent_id: 'q6',
    text: 'Delta, uporządkuj pliki w folderze Pobrane.',
    addressed_to: 'delta',
    profile: 'cloud',
  })) as { assistant_turn_id: string | null };
  scheduler.runAll();
  await flush();
  await both('turns', 'regenerate', 's-q3', 'q4b', 'local');
  scheduler.runAll();
  await flush();
  await both('turns', 'editAndResend', 's-q3', 'q3', 'Raport Q3 tylko dla zarządu, krótko.');
  scheduler.runAll();
  await flush();
  await both('turns', 'continueTurn', 's-q3', sent.assistant_turn_id ?? 'q6');
  await both('turns', 'stop', 's-q3');
  await flush();
  await both('turns', 'rate', 'q4b', 'up');
  await both('turns', 'rate', 'q4a', null);
  await both('turns', 'setHidden', 'q4a', true);
  await both('turns', 'remember', 'q4b', 'session');
  await both('turns', 'readAloud', 'q4b');
  await both('turns', 'saveCode', 'q4b', 2);
  await both('turns', 'runCode', 'q4b', 2);
  await both('turns', 'undoStep', 's-q3:u5');
  await both('sessions', 'search', 'raport');
  await both('turns', 'list', 's-q3');

  await both('agents', 'list', 's-q3');
  await both('agents', 'setRoles', 's-q3', 'delta', ['operator', 'coder']);
  await both('agents', 'applyCast', 's-q3', 'research');
  await both('agents', 'steer', 's-q3', 'Pomiń pliki PDF');
  const runs = (await both('agents', 'runs', 's-q3')) as {
    steps: { id: string; intent: { kind: string } | null }[];
  }[];
  const terminal = runs
    .flatMap((r) => r.steps)
    .find((step) => step.intent?.kind === 'open_in_terminal');
  if (!terminal) throw new Error('brak kroku z intencją terminala');
  await both('agents', 'openTerminal', terminal.id);

  await both('costs', 'summary', 's-q3');
  await both('costs', 'summary', null);
  await both('costs', 'setMonthlyLimit', true, { minor: 25_000, currency: 'PLN' });

  await both('settings', 'schema');
  await both('settings', 'values');
  await both('settings', 'set', 'ui.theme', 'dark');
  await both('settings', 'set', 'general.destroy_webview_after', 15);
  await both('settings', 'set', 'composer.enter_sends', false);
  await both('settings', 'reset', 'ui.theme');
  await both('settings', 'setShortcut', 'palette.open', 'Ctrl+Shift+P');
  await both('settings', 'setShortcut', 'focus.toggle', '');
  await both('settings', 'setShortcut', 'palette.open', null);

  await both('timeline', 'list', 's-q3', { kinds: [], min_level: 'trace' });
  await both('timeline', 'list', 's-q3', { kinds: ['model_call', 'tool'], min_level: 'info' });

  await both('files', 'list', 's-q3');
  await both('files', 'preview', 'a2');
  await both('files', 'preview', 'a1');
  await both('files', 'act', 'a1', 'reveal');

  await both('accounts', 'catalog');
  await both('accounts', 'list');
  const account = (await both('accounts', 'add', {
    provider_id: 'openai',
    label: 'OpenAI — praca',
    secret: 'sk-test-0000000000000000',
    base_url: null,
  })) as { id: string };
  await both('accounts', 'test', account.id);
  await both('accounts', 'assign', account.id, {
    task_classes: ['chat', 'code'],
    agents: ['beta'],
    voice_stt: true,
    voice_tts: false,
  });
  await both('accounts', 'setLimit', account.id, true, { minor: 5_000, currency: 'PLN' });
  await both('accounts', 'remove', account.id);

  await both('transfer', 'exportPackage', {
    scope: {
      config_common: true,
      personas: true,
      casts: true,
      sessions: ['s-q3'],
      artifacts: false,
      logs: false,
      config_machine: false,
    },
    password: null,
  });
  await both('transfer', 'inspect', null, 'C:\\Users\\Ty\\Pobrane\\laptop-encrypted.alfa');
  await both('transfer', 'inspect', 'hasło', null);
  await both('transfer', 'importPackage', {
    path: 'C:\\Users\\Ty\\Pobrane\\laptop.alfa',
    mode: 'merge',
    resolutions: { 'session:s-q3': 'keep_both' },
    password: null,
  });
  await both('transfer', 'rollback', 'snap-1');

  await both('permissions', 'get', 's-q3');
  await both('permissions', 'requestLevel', 'L4', 's-q3');
  await both('permissions', 'requestLevel', 'L1', null);
  await both('permissions', 'openApproval', 'ap-1');

  await both('models', 'localList');
  await both('models', 'localDownload', null);
  scheduler.advance(600);
  await flush();
  await both('models', 'localCancel', null);
  await both('models', 'localDownload', 'bielik-4.5b-v3.0-instruct-q4_k_m');
  scheduler.runAll();
  await flush();
  await both('models', 'localList');

  await both('device', 'profile');
  await both('device', 'measure');

  await both('voice', 'devices');
  await both('voice', 'startMicTest', 'mic-1');
  scheduler.advance(100);
  await flush();
  await both('voice', 'stopMicTest');
  await both('voice', 'setMicEnabled', true);
  await both('voice', 'setMuted', false);
  await both('voice', 'stopSpeech');
  await both('voice', 'status');
  scheduler.advance(2_000);
  await flush();
  await both('voice', 'ptt', true);
  await both('voice', 'ptt', false);
  await both('voice', 'preview', 'beta');

  fake.setOnline(false);
  await both('system', 'status');
  await both('turns', 'send', 's-api', {
    parent_id: 's-api-2',
    text: 'Dodaj testy.',
    addressed_to: null,
    profile: null,
  });
  await both('system', 'retryQueue');
  await flush();
  fake.setOnline(true);
  fake.setRateLimited(3);
  scheduler.runAll();
  await flush();
  await both('turns', 'send', 's-trip', {
    parent_id: null,
    text: 'Sprawdź pociągi do Gdańska.',
    addressed_to: null,
    profile: 'hybrid',
  });
  scheduler.runAll();
  await flush();
  await both('system', 'status');
  fake.setRateLimited(null);

  const quick = (await both('quick', 'ask', 'Ile to 17% z 2 400 zł?')) as { session_id: string };
  scheduler.runAll();
  await flush();
  await both('quick', 'expandToMain', quick.session_id);
  await both('quick', 'hide');
  await flush();

  await runWork(both, scheduler, flush);
  await runComputer(both, scheduler, flush);
  await runUpdates(both, scheduler, flush);
  await runBroker(both);
  await runPlugins(both);
  await runVoice(both, scheduler, flush);
  await runModels(both, scheduler, flush);
  await runFiles(both, scheduler, flush);
}

function sampleEvents(all: readonly AlfaEvent[]): AlfaEvent[] {
  const perType = new Map<string, number>();
  const picked: AlfaEvent[] = [];
  for (const event of all) {
    const seen = perType.get(event.type) ?? 0;
    if (seen >= 3) continue;
    perType.set(event.type, seen + 1);
    picked.push(event);
  }
  return picked;
}

await run();
mkdirSync(out, { recursive: true });
for (const [ns, list] of entries) {
  writeFileSync(join(out, `${ns}.json`), `${JSON.stringify(list, null, 2)}\n`);
}
writeFileSync(join(out, 'events.json'), `${JSON.stringify(sampleEvents(events), null, 2)}\n`);
