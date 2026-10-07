import { describe, expect, it } from 'vitest';
import { FAKE_DICTATION_MS, FAKE_SENTENCE_MS, FAKE_WAKE_TEST_MS } from '../fake/api-voice-features';
import { FakeAlfaClient, VirtualScheduler } from '../fake/fake-client';
import type { AlfaEvent } from '../types-system';

const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

function setup(scenario?: 'no-mic') {
  const scheduler = new VirtualScheduler();
  const client = new FakeAlfaClient({ scheduler, scenario });
  const events: AlfaEvent[] = [];
  client.subscribe((batch) => events.push(...batch));
  const api = client.voiceFeatures;
  return { client, scheduler, events, api };
}

describe('atrapa: głos rozszerzony F5', () => {
  it('słowa wywoławcze: domyślnie wyłączone, tylko jawnie i z ryzykiem, bramka wymaga głosu', async () => {
    const { api, scheduler, events } = setup();
    const v = await api.features();
    expect(v.wake.enabled).toBe(false);
    expect(v.wake.calibration.measured).toBe(false);
    const on = { kind: 'configure', enabled: true, accept_risk: false, owner_gate: false } as const;
    await expect(api.wake(on)).rejects.toThrow(/nieskalibrowane/);
    await expect(api.wake({ ...on, accept_risk: true, owner_gate: true })).rejects.toThrow(
      /Bramka właściciela/,
    );
    const armed = await api.wake({ ...on, accept_risk: true });
    expect([armed.wake.enabled, armed.wake.state]).toEqual([true, 'armed']);
    const dnd = await api.wake({ kind: 'set_dnd', on: true });
    expect(dnd.wake.state).toBe('suspended');
    await api.wake({ kind: 'test', on: true });
    scheduler.advance(FAKE_WAKE_TEST_MS);
    await flush();
    const last = events.filter((e) => e.type === 'VoiceFeaturesChanged').at(-1);
    expect(last?.type === 'VoiceFeaturesChanged' && last.features.wake.test.detections).toBe(1);
  });

  it('kreator rejestracji: 3 frazy → profil; za mało fraz → odmowa', async () => {
    const { api } = setup();
    await api.speaker({ kind: 'begin' });
    await expect(api.speaker({ kind: 'finish' })).rejects.toThrow(/za mało/);
    for (let i = 0; i < 3; i += 1) {
      const rec = await api.speaker({ kind: 'record_start' });
      expect(rec.speaker.recording).toBe(true);
      const done = await api.speaker({ kind: 'record_stop' });
      expect(done.speaker.last_sample?.quality).toBe('good');
    }
    const enrolled = await api.speaker({ kind: 'finish' });
    expect(enrolled.speaker.state).toBe('enrolled');
    const off = await api.speaker({ kind: 'set_required', required: false });
    expect(off.speaker.required_for_risky).toBe(false);
  });

  it('dyktowanie z podglądem i profilami; czytanie z kolejką i stopem', async () => {
    const { api, scheduler } = setup();
    await expect(
      api.dictation({
        kind: 'save_profile',
        profile: { app: 'notepad', capitalize_start: false, block_enter: true },
      }),
    ).rejects.toThrow(/nazwą programu/);
    const saved = await api.dictation({
      kind: 'save_profile',
      profile: { app: 'C:\\Windows\\NOTEPAD.EXE', capitalize_start: false, block_enter: true },
    });
    expect(saved.dictation.profiles.map((p) => p.app)).toEqual(['notepad.exe']);
    const started = await api.dictation({ kind: 'toggle' });
    expect(started.dictation.state).toBe('active');
    scheduler.advance(FAKE_DICTATION_MS);
    await flush();
    expect((await api.features()).dictation.preview).toBe('dzień dobry kropka');
    const stopped = await api.dictation({ kind: 'toggle' });
    expect([stopped.dictation.state, stopped.dictation.preview]).toEqual(['idle', null]);

    const reading = await api.read({ kind: 'start', source: 'clipboard' });
    expect([reading.read.state, reading.read.app]).toEqual(['speaking', 'Schowek']);
    expect((await api.read({ kind: 'start', source: 'selection' })).read.queued).toBe(1);
    scheduler.advance(FAKE_SENTENCE_MS * 2);
    await flush();
    expect((await api.features()).read.index).toBe(2);
    const halted = await api.read({ kind: 'control', control: 'stop' });
    expect([halted.read.state, halted.read.queued]).toEqual(['idle', 0]);
    await expect(api.read({ kind: 'control', control: 'next' })).rejects.toThrow();
    expect((await api.read({ kind: 'set_rate', rate: 3 })).read.rate).toBe(2);
  });

  it('bez mikrofonu: słowa, rejestracja i dyktowanie niedostępne', async () => {
    const { api } = setup('no-mic');
    const v = await api.features();
    expect([v.wake.state, v.speaker.state, v.dictation.state]).toEqual([
      'unavailable',
      'unavailable',
      'unavailable',
    ]);
    expect(v.s2s.available).toBe(false);
    await expect(api.dictation({ kind: 'start' })).rejects.toThrow();
  });
});
