// Atrapa głosu rozszerzonego F5 (deterministyczna, zegar atrapy): słowa wywoławcze domyślnie
// wyłączone i nieskalibrowane (włączenie tylko z „na własne ryzyko"; bramka właściciela wymaga
// profilu), test wykrywa „Hej Delta" po 1,5 s; kreator rejestracji (3 frazy); dyktowanie do
// „notepad.exe" (podgląd po 600 ms); czytanie 5 zdań co 800 ms z kolejką. Scenariusz „no-mic":
// słowa, rejestracja i dyktowanie niedostępne.
import type { VoiceFeaturesApi } from '../client-voice';
import type {
  DictationAction,
  DictationProfile,
  ReadAction,
  SpeakerAction,
  VoiceFeatures,
  WakeAction,
} from '../types-voice';
import type { FakeCore } from './core';

export const FAKE_WAKE_TEST_MS = 1_500;
export const FAKE_DICTATION_MS = 600;
export const FAKE_SENTENCE_MS = 800;
const SEGMENTS = 5;

const text = (pl: string, en: string) => ({ pl, en });

const PROMPTS = [
  text(
    'Hej Alfa, sprawdź proszę, jaka jutro będzie pogoda w Krakowie.',
    'Hey Alfa, please check what the weather will be like in Kraków tomorrow.',
  ),
  text(
    'Przypomnij mi w czwartek o spotkaniu z zespołem o dziewiątej.',
    'Remind me on Thursday about the team meeting at nine.',
  ),
  text(
    'Otwórz ostatni raport i przeczytaj mi pierwszy akapit.',
    'Open the latest report and read me the first paragraph.',
  ),
];

function initial(noMic: boolean): VoiceFeatures {
  const micReason = noMic ? text('Brak dostępu do mikrofonu.', 'No microphone access.') : null;
  return {
    wake: {
      state: noMic ? 'unavailable' : 'off',
      reason: micReason,
      enabled: false,
      risk_accepted: false,
      owner_gate: true,
      dnd: false,
      phrases: ['Hej Alfa', 'Hej Beta', 'Hej Gama', 'Hej Delta'],
      calibration: {
        measured: false,
        sufficient: false,
        passes: false,
        far_per_day: null,
        frr: null,
        threshold: 0.8,
      },
      test: { active: false, detections: 0, last_agent: null, owner_rejected: 0 },
    },
    speaker: {
      state: noMic ? 'unavailable' : 'not_enrolled',
      reason: micReason,
      done: 0,
      needed: 3,
      recording: false,
      prompts: PROMPTS,
      last_sample: null,
      required_for_risky: true,
      last_check: null,
    },
    dictation: {
      state: noMic ? 'unavailable' : 'idle',
      reason: micReason,
      app: null,
      typed_chars: 0,
      pending_chars: 0,
      can_undo: false,
      preview: null,
      shortcut: 'Ctrl+Alt+D',
      profiles: [],
    },
    read: {
      state: 'idle',
      reason: null,
      app: null,
      index: 0,
      segments: 0,
      rate: 1,
      queued: 0,
      agent: 'alfa',
      shortcut: 'Ctrl+Alt+R',
    },
    s2s: {
      available: false,
      reason: text(
        'Szybka rozmowa w chmurze wymaga klucza API dostawcy (OpenAI Realtime) — adapter w przygotowaniu.',
        'Fast cloud conversation needs a provider API key (OpenAI Realtime) — adapter in preparation.',
      ),
    },
  };
}

const fail = (message: string) => Promise.reject(new Error(message));

export class FakeVoiceFeatures {
  private view: VoiceFeatures;
  private readTimer: number | null = null;

  constructor(private readonly core: FakeCore) {
    this.view = initial(core.scenario === 'no-mic');
  }

  private set(patch: Partial<VoiceFeatures>): VoiceFeatures {
    this.view = { ...this.view, ...patch };
    this.core.emit([{ type: 'VoiceFeaturesChanged', features: this.view }]);
    return this.view;
  }

  private wake(action: WakeAction): Promise<VoiceFeatures> {
    const w = this.view.wake;
    if (w.state === 'unavailable') return fail('Słowa wywoławcze: brak mikrofonu.');
    if (action.kind === 'set_dnd') {
      const state = action.on && w.enabled ? 'suspended' : w.enabled ? 'armed' : w.state;
      return this.core.reply(this.set({ wake: { ...w, dnd: action.on, state } }));
    }
    if (action.kind === 'test') {
      const test = { ...w.test, active: action.on };
      this.set({ wake: { ...w, test, state: action.on ? 'armed' : w.enabled ? 'armed' : 'off' } });
      if (action.on) {
        this.core.scheduler.setTimeout(() => {
          const cur = this.view.wake;
          if (!cur.test.active) return;
          const gated = cur.owner_gate && this.view.speaker.state !== 'enrolled';
          this.set({
            wake: {
              ...cur,
              test: gated
                ? { ...cur.test, owner_rejected: cur.test.owner_rejected + 1 }
                : { ...cur.test, detections: cur.test.detections + 1, last_agent: 'delta' },
            },
          });
        }, FAKE_WAKE_TEST_MS);
      }
      return this.core.reply(this.view);
    }
    if (action.enabled && !w.calibration.passes && !action.accept_risk) {
      return fail(
        'Słowa wywoławcze są nieskalibrowane (brak pomiaru FAR/FRR na korpusie) — potwierdź, że włączasz je na własne ryzyko.',
      );
    }
    if (action.enabled && action.owner_gate && this.view.speaker.state !== 'enrolled') {
      return fail(
        'Bramka właściciela wymaga zarejestrowanego głosu — najpierw przejdź kreator „Rozpoznawanie mojego głosu” albo wyłącz bramkę.',
      );
    }
    return this.core.reply(
      this.set({
        wake: {
          ...w,
          enabled: action.enabled,
          risk_accepted: action.accept_risk,
          owner_gate: action.owner_gate,
          state: action.enabled ? (w.dnd ? 'suspended' : 'armed') : 'off',
          reason: null,
        },
      }),
    );
  }

  private speaker(action: SpeakerAction): Promise<VoiceFeatures> {
    const s = this.view.speaker;
    if (s.state === 'unavailable') return fail('Rozpoznawanie głosu: brak mikrofonu.');
    switch (action.kind) {
      case 'set_required':
        return this.core.reply(
          this.set({ speaker: { ...s, required_for_risky: action.required } }),
        );
      case 'begin':
        return this.core.reply(
          this.set({ speaker: { ...s, state: 'enrolling', done: 0, last_sample: null } }),
        );
      case 'record_start':
        return this.core.reply(
          this.set({ speaker: { ...s, state: 'enrolling', recording: true } }),
        );
      case 'record_stop': {
        if (!s.recording) return fail('Nagranie nie trwa.');
        const sample = {
          accepted: true,
          quality: 'good' as const,
          level_db: -24,
          duration_ms: 2_800,
          message: null,
        };
        return this.core.reply(
          this.set({ speaker: { ...s, recording: false, done: s.done + 1, last_sample: sample } }),
        );
      }
      case 'finish':
        if (s.done < s.needed)
          return fail(`Rozpoznawanie głosu: za mało wypowiedzi: ${s.done} z ${s.needed}`);
        return this.core.reply(
          this.set({ speaker: { ...s, state: 'enrolled', last_sample: null } }),
        );
      case 'cancel':
      case 'delete':
        return this.core.reply(
          this.set({
            speaker: { ...s, state: 'not_enrolled', done: 0, recording: false, last_sample: null },
          }),
        );
    }
  }

  private dictation(action: DictationAction): Promise<VoiceFeatures> {
    const d = this.view.dictation;
    const live = d.state === 'active' || d.state === 'paused';
    switch (action.kind) {
      case 'save_profile': {
        const app = action.profile.app.split(/[\\/]/).pop()?.trim().toLowerCase() ?? '';
        if (!app.endsWith('.exe')) {
          return fail(
            `Profil dyktowania: „${action.profile.app}” nie jest nazwą programu (np. notepad.exe).`,
          );
        }
        const profile: DictationProfile = { ...action.profile, app };
        const profiles = [...d.profiles.filter((p) => p.app !== app), profile].sort((a, b) =>
          a.app.localeCompare(b.app),
        );
        return this.core.reply(this.set({ dictation: { ...d, profiles } }));
      }
      case 'remove_profile':
        return this.core.reply(
          this.set({
            dictation: { ...d, profiles: d.profiles.filter((p) => p.app !== action.app) },
          }),
        );
      case 'undo':
        if (!live) return fail('Dyktowanie nie trwa.');
        return this.core.reply(this.set({ dictation: { ...d, typed_chars: 0, can_undo: false } }));
      case 'stop':
        return this.core.reply(this.stopDictation());
      case 'toggle':
        if (live) return this.core.reply(this.stopDictation());
        return this.startDictation();
      case 'start':
        return live ? this.core.reply(this.view) : this.startDictation();
    }
  }

  private stopDictation(): VoiceFeatures {
    const d = this.view.dictation;
    return this.set({
      dictation: {
        ...d,
        state: 'idle',
        app: null,
        preview: null,
        pending_chars: 0,
        can_undo: false,
      },
    });
  }

  private startDictation(): Promise<VoiceFeatures> {
    const d = this.view.dictation;
    if (d.state === 'unavailable') return fail('Dyktowanie: brak mikrofonu.');
    const view = this.set({
      dictation: { ...d, state: 'active', app: 'notepad.exe', typed_chars: 0, reason: null },
    });
    this.core.scheduler.setTimeout(() => {
      const cur = this.view.dictation;
      if (cur.state !== 'active') return;
      this.set({
        dictation: { ...cur, preview: 'dzień dobry kropka', typed_chars: 12, can_undo: true },
      });
    }, FAKE_DICTATION_MS);
    return this.core.reply(view);
  }

  private tick(): void {
    const r = this.view.read;
    if (r.state !== 'speaking') {
      this.readTimer = null;
      return;
    }
    if (r.index + 1 < r.segments) {
      this.set({ read: { ...r, index: r.index + 1 } });
    } else if (r.queued > 0) {
      this.set({ read: { ...r, index: 0, queued: r.queued - 1 } });
    } else {
      this.set({ read: { ...r, state: 'idle', index: 0, segments: 0, app: null } });
      this.readTimer = null;
      return;
    }
    this.readTimer = this.core.scheduler.setTimeout(() => this.tick(), FAKE_SENTENCE_MS);
  }

  private read(action: ReadAction): Promise<VoiceFeatures> {
    const r = this.view.read;
    const live = r.state === 'speaking' || r.state === 'paused';
    if (action.kind === 'set_rate') {
      const rate = Math.min(2, Math.max(0.5, Math.round(action.rate * 10) / 10));
      return this.core.reply(this.set({ read: { ...r, rate } }));
    }
    if (action.kind === 'start') {
      if (live) return this.core.reply(this.set({ read: { ...r, queued: r.queued + 1 } }));
      const app = action.source === 'clipboard' ? 'Schowek' : 'notepad.exe';
      const view = this.set({
        read: { ...r, state: 'speaking', app, index: 0, segments: SEGMENTS, reason: null },
      });
      this.readTimer = this.core.scheduler.setTimeout(() => this.tick(), FAKE_SENTENCE_MS);
      return this.core.reply(view);
    }
    if (!live && action.control !== 'stop') return fail('Nic nie jest teraz czytane.');
    const step = (d: number) => Math.min(2, Math.max(0.5, Math.round((r.rate + d) * 10) / 10));
    switch (action.control) {
      case 'stop':
        if (this.readTimer !== null) this.core.scheduler.clearTimeout(this.readTimer);
        this.readTimer = null;
        return this.core.reply(
          this.set({ read: { ...r, state: 'idle', index: 0, segments: 0, queued: 0, app: null } }),
        );
      case 'pause':
        return this.core.reply(this.set({ read: { ...r, state: 'paused' } }));
      case 'resume': {
        const view = this.set({ read: { ...r, state: 'speaking' } });
        if (this.readTimer === null) {
          this.readTimer = this.core.scheduler.setTimeout(() => this.tick(), FAKE_SENTENCE_MS);
        }
        return this.core.reply(view);
      }
      case 'next':
        return this.core.reply(
          this.set({ read: { ...r, index: Math.min(r.segments - 1, r.index + 1) } }),
        );
      case 'previous':
        return this.core.reply(this.set({ read: { ...r, index: Math.max(0, r.index - 1) } }));
      case 'restart':
        return this.core.reply(this.set({ read: { ...r, index: 0 } }));
      case 'faster':
        return this.core.reply(this.set({ read: { ...r, rate: step(0.1) } }));
      case 'slower':
        return this.core.reply(this.set({ read: { ...r, rate: step(-0.1) } }));
    }
  }

  /** Esc w oknie głównym zatrzymuje też czytanie (jak `voice_stop_speech` w rdzeniu). */
  stopAll(): void {
    if (this.view.read.state !== 'idle') void this.read({ kind: 'control', control: 'stop' });
  }

  api(): VoiceFeaturesApi {
    return {
      features: () => this.core.reply(this.view),
      wake: (action) => this.wake(action),
      speaker: (action) => this.speaker(action),
      dictation: (action) => this.dictation(action),
      read: (action) => this.read(action),
    };
  }
}
