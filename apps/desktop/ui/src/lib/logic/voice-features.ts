// Logika panelu Głos (F5) bez DOM: ogłoszenia dla czytników ekranu przy istotnych zmianach stanu
// (z throttlingiem — `aria-live` nie zalewa czytnika postępem czytania), opis pomiaru FAR/FRR,
// poziom nagrania kreatora (dBFS → 0..1), nazwa programu dla profilu dyktowania.
import type { MessageKey } from '../i18n/pl';
import type { Timers } from './sentence-announcer';
import { realTimers } from './sentence-announcer';
import type { VoiceFeatures, WakeCalibrationView } from '../api/types-voice';

export interface Announcement {
  readonly key: MessageKey;
  readonly params?: Readonly<Record<string, string | number>>;
}

/** Czy włączenie słów wywoławczych wymaga potwierdzenia ryzyka (brak pełnego pomiaru). */
export function needsRiskConfirmation(c: WakeCalibrationView): boolean {
  return !(c.measured && c.sufficient && c.passes);
}

/** Opis pomiaru FAR/FRR (klucz + parametry). */
export function calibrationLine(c: WakeCalibrationView): Announcement {
  if (!c.measured) return { key: 'vf.wake.uncalibrated' };
  const far = c.far_per_day === null ? '∞' : c.far_per_day.toFixed(2);
  const frr = c.frr === null ? '—' : (c.frr * 100).toFixed(1);
  if (!c.sufficient) return { key: 'vf.wake.measuredSmall', params: { far, frr } };
  return { key: c.passes ? 'vf.wake.measuredOk' : 'vf.wake.measuredFail', params: { far, frr } };
}

/** Poziom nagrania kreatora: dBFS (−60…0) → 0..1 dla miernika. */
export function levelOf(db: number): number {
  return Math.max(0, Math.min(1, (db + 60) / 60));
}

/** Nazwa programu dla profilu (`C:\\…\\Notepad.exe` → `notepad.exe`); `null` — niepoprawna. */
export function appName(input: string): string | null {
  const name = (input.split(/[\\/]/).pop() ?? '').trim().toLowerCase();
  if (!name.endsWith('.exe') || name.length > 64 || !/^[\p{L}\p{N}._\- ]+$/u.test(name))
    return null;
  return name;
}

/**
 * Ogłoszenie przy istotnej zmianie (nie przy każdym odświeżeniu): stan słów wywoławczych,
 * wykrycie w teście, wynik nagrania frazy, start/pauza/koniec dyktowania i czytania.
 */
export function announcement(prev: VoiceFeatures | null, next: VoiceFeatures): Announcement | null {
  if (!prev) return null;
  const w = next.wake;
  if (w.test.detections > prev.wake.test.detections) {
    return { key: 'vf.ann.detected', params: { agent: w.test.last_agent ?? '—' } };
  }
  if (w.test.owner_rejected > prev.wake.test.owner_rejected) return { key: 'vf.ann.ownerRejected' };
  if (w.state !== prev.wake.state && w.state !== 'unavailable') {
    return { key: `vf.wake.state.${w.state}` as const };
  }
  const sample = next.speaker.last_sample;
  if (sample && sample !== prev.speaker.last_sample && !next.speaker.recording) {
    return sample.accepted
      ? { key: 'vf.ann.sampleOk', params: { done: next.speaker.done, needed: next.speaker.needed } }
      : { key: `vf.quality.${sample.quality}` as const };
  }
  if (next.speaker.state === 'enrolled' && prev.speaker.state !== 'enrolled') {
    return { key: 'vf.ann.enrolled' };
  }
  const d = next.dictation;
  if (d.state !== prev.dictation.state) {
    if (d.state === 'active') return { key: 'vf.ann.dictating', params: { app: d.app ?? '—' } };
    if (d.state === 'paused') return { key: 'vf.ann.dictationPaused' };
    if (d.state === 'idle') return { key: 'vf.ann.dictationStopped' };
  }
  const r = next.read;
  if (r.state !== prev.read.state) {
    if (r.state === 'speaking') {
      return { key: 'vf.ann.reading', params: { n: r.index + 1, of: r.segments } };
    }
    if (r.state === 'paused') return { key: 'vf.ann.readPaused' };
    if (r.state === 'idle') return { key: 'vf.ann.readStopped' };
  }
  return null;
}

/**
 * Throttling ogłoszeń: najwyżej jedno na `minIntervalMs`; w międzyczasie zostaje tylko najnowsze
 * (czytnik dostaje aktualny stan, a nie kolejkę przeterminowanych komunikatów).
 */
export class ThrottledAnnouncer {
  private last = Number.NEGATIVE_INFINITY;
  private pending: string | null = null;
  private timer: unknown = null;

  constructor(
    private readonly emit: (text: string) => void,
    private readonly minIntervalMs = 1_500,
    private readonly timers: Timers = realTimers,
  ) {}

  push(text: string): void {
    const now = this.timers.now();
    if (now - this.last >= this.minIntervalMs && this.timer === null) {
      this.last = now;
      this.emit(text);
      return;
    }
    this.pending = text;
    if (this.timer !== null) return;
    const wait = Math.max(0, this.minIntervalMs - (now - this.last));
    this.timer = this.timers.setTimeout(() => {
      this.timer = null;
      const text = this.pending;
      this.pending = null;
      if (text !== null) {
        this.last = this.timers.now();
        this.emit(text);
      }
    }, wait);
  }

  dispose(): void {
    if (this.timer !== null) this.timers.clearTimeout(this.timer);
    this.timer = null;
    this.pending = null;
  }
}
