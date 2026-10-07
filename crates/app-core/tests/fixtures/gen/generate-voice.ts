// Fixture'y głosu rozszerzonego F5 (`voice_features`, `voice_wake`, `voice_speaker`,
// `voice_dictation`, `voice_read`; zdarzenie `VoiceFeaturesChanged`) — część generatora
// `generate.ts` (atrapa + `TauriAlfaClient` z atrapą `invoke`).
import {
  FAKE_DICTATION_MS,
  FAKE_SENTENCE_MS,
  FAKE_WAKE_TEST_MS,
} from '../../../../../apps/desktop/ui/src/lib/api/fake/api-voice-features';
import type { VirtualScheduler } from '../../../../../apps/desktop/ui/src/lib/api/fake/fake-client';

type Both = (ns: string, method: string, ...args: unknown[]) => Promise<unknown>;

export async function runVoice(
  both: Both,
  scheduler: VirtualScheduler,
  flush: () => Promise<void>,
): Promise<void> {
  const ns = 'voiceFeatures';
  await both(ns, 'features');
  await both(ns, 'wake', { kind: 'test', on: true });
  scheduler.advance(FAKE_WAKE_TEST_MS);
  await flush();
  await both(ns, 'wake', { kind: 'test', on: false });
  await both(ns, 'speaker', { kind: 'begin' });
  for (let i = 0; i < 3; i += 1) {
    await both(ns, 'speaker', { kind: 'record_start' });
    await both(ns, 'speaker', { kind: 'record_stop' });
  }
  await both(ns, 'speaker', { kind: 'finish' });
  await both(ns, 'speaker', { kind: 'set_required', required: true });
  await both(ns, 'wake', { kind: 'configure', enabled: true, accept_risk: true, owner_gate: true });
  await both(ns, 'wake', { kind: 'set_dnd', on: false });
  await both(ns, 'dictation', {
    kind: 'save_profile',
    profile: { app: 'notepad.exe', capitalize_start: false, block_enter: true },
  });
  await both(ns, 'dictation', { kind: 'start' });
  scheduler.advance(FAKE_DICTATION_MS);
  await flush();
  await both(ns, 'dictation', { kind: 'undo' });
  await both(ns, 'dictation', { kind: 'toggle' });
  await both(ns, 'dictation', { kind: 'remove_profile', app: 'notepad.exe' });
  await both(ns, 'read', { kind: 'set_rate', rate: 1.2 });
  await both(ns, 'read', { kind: 'start', source: 'clipboard' });
  await both(ns, 'read', { kind: 'start', source: 'selection' });
  scheduler.advance(FAKE_SENTENCE_MS);
  await flush();
  await both(ns, 'read', { kind: 'control', control: 'pause' });
  await both(ns, 'read', { kind: 'control', control: 'stop' });
  await both(ns, 'speaker', { kind: 'delete' });
  await flush();
}
