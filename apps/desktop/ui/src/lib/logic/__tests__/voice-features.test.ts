import { describe, expect, it } from 'vitest';
import { FakeAlfaClient, VirtualScheduler } from '../../api/fake/fake-client';
import type { VoiceFeatures } from '../../api/types-voice';
import {
  ThrottledAnnouncer,
  announcement,
  appName,
  calibrationLine,
  levelOf,
  needsRiskConfirmation,
} from '../voice-features';

async function base(): Promise<VoiceFeatures> {
  const client = new FakeAlfaClient({ scheduler: new VirtualScheduler() });
  return client.voiceFeatures.features();
}

describe('logika panelu Głos', () => {
  it('pomiar FAR/FRR: brak pomiaru = ryzyko, pełny pomiar ze spełnionymi progami — nie', () => {
    const none = {
      measured: false,
      sufficient: false,
      passes: false,
      far_per_day: null,
      frr: null,
      threshold: 0.8,
    };
    expect(needsRiskConfirmation(none)).toBe(true);
    expect(calibrationLine(none).key).toBe('vf.wake.uncalibrated');
    const ok = {
      ...none,
      measured: true,
      sufficient: true,
      passes: true,
      far_per_day: 0.5,
      frr: 0.03,
    };
    expect(needsRiskConfirmation(ok)).toBe(false);
    expect(calibrationLine(ok)).toEqual({
      key: 'vf.wake.measuredOk',
      params: { far: '0.50', frr: '3.0' },
    });
    expect(needsRiskConfirmation({ ...ok, sufficient: false })).toBe(true);
    expect(calibrationLine({ ...ok, passes: false }).key).toBe('vf.wake.measuredFail');
  });

  it('nazwa programu i poziom nagrania', () => {
    expect(appName('C:\\Windows\\Notepad.EXE')).toBe('notepad.exe');
    expect(appName('notepad')).toBeNull();
    expect(appName('a<b>.exe')).toBeNull();
    expect(levelOf(-90)).toBe(0);
    expect(levelOf(0)).toBe(1);
    expect(levelOf(-30)).toBeCloseTo(0.5);
  });

  it('ogłoszenia tylko przy istotnych zmianach', async () => {
    const v = await base();
    expect(announcement(null, v)).toBeNull();
    expect(announcement(v, v)).toBeNull();
    const detected = {
      ...v,
      wake: { ...v.wake, test: { ...v.wake.test, detections: 1, last_agent: 'delta' } },
    };
    expect(announcement(v, detected)).toEqual({
      key: 'vf.ann.detected',
      params: { agent: 'delta' },
    });
    const reading = { ...v, read: { ...v.read, state: 'speaking' as const, segments: 4 } };
    expect(announcement(v, reading)?.key).toBe('vf.ann.reading');
    // Postęp zdań nie jest ogłaszany (tylko zmiany stanu).
    expect(announcement(reading, { ...reading, read: { ...reading.read, index: 2 } })).toBeNull();
    const dictating = {
      ...v,
      dictation: { ...v.dictation, state: 'active' as const, app: 'x.exe' },
    };
    expect(announcement(v, dictating)).toEqual({
      key: 'vf.ann.dictating',
      params: { app: 'x.exe' },
    });
  });

  it('throttling: najwyżej jedno ogłoszenie na okres, w międzyczasie tylko najnowsze', () => {
    let now = 0;
    const pending: (() => void)[] = [];
    const timers = {
      now: () => now,
      setTimeout: (cb: () => void) => {
        pending.push(cb);
        return pending.length;
      },
      clearTimeout: () => undefined,
    };
    const out: string[] = [];
    const a = new ThrottledAnnouncer((t) => out.push(t), 1_000, timers);
    a.push('a');
    a.push('b');
    a.push('c');
    expect(out).toEqual(['a']);
    now = 1_000;
    pending.shift()?.();
    expect(out).toEqual(['a', 'c']);
    a.dispose();
  });
});
