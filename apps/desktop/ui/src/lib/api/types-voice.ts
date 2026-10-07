// Głos rozszerzony F5 (komendy `voice_features`, `voice_wake`, `voice_speaker`, `voice_dictation`,
// `voice_read`; zdarzenie `VoiceFeaturesChanged`) — kształt 1:1 z `crates/app-api/src/dto/
// voice_features.rs`. Widok nigdy nie zawiera audio, embeddingu mówcy ani czytanego tekstu;
// podgląd dyktowania to ostatnia fraza (tylko w oknie Alfy).
import type { LocalizedText } from './types';

export type WakeWordsState = 'unavailable' | 'off' | 'armed' | 'listening' | 'suspended';

/** Pomiar FAR/FRR z runnera `alfa-wake-eval` (`measured: false` — „nieskalibrowane"). */
export interface WakeCalibrationView {
  readonly measured: boolean;
  /** ≥ 24 h tła i ≥ 200 pozytywów. */
  readonly sufficient: boolean;
  /** FAR ≤ 1/dzień i FRR ≤ 5 %. */
  readonly passes: boolean;
  readonly far_per_day: number | null;
  readonly frr: number | null;
  readonly threshold: number;
}

/** Test słowa wywoławczego: wykrycia bez otwierania rozmowy. */
export interface WakeTestView {
  readonly active: boolean;
  readonly detections: number;
  readonly last_agent: string | null;
  readonly owner_rejected: number;
}

export interface WakeWordsView {
  readonly state: WakeWordsState;
  readonly reason: LocalizedText | null;
  readonly enabled: boolean;
  readonly risk_accepted: boolean;
  readonly owner_gate: boolean;
  readonly dnd: boolean;
  readonly phrases: readonly string[];
  readonly calibration: WakeCalibrationView;
  readonly test: WakeTestView;
}

export type SpeakerState = 'unavailable' | 'not_enrolled' | 'enrolling' | 'enrolled';
export type SampleQuality = 'good' | 'too_short' | 'too_quiet' | 'inconsistent' | 'failed';

/** Ostatnia nagrana fraza rejestracji (bez audio). */
export interface EnrollSampleView {
  readonly accepted: boolean;
  readonly quality: SampleQuality;
  readonly level_db: number;
  readonly duration_ms: number;
  readonly message: LocalizedText | null;
}

export type SpeakerDecisionView = 'verified' | 'likely' | 'rejected' | 'not_checked';

export interface SpeakerCheckView {
  readonly decision: SpeakerDecisionView;
  readonly score_permille: number | null;
}

export interface SpeakerView {
  readonly state: SpeakerState;
  readonly reason: LocalizedText | null;
  readonly done: number;
  readonly needed: number;
  readonly recording: boolean;
  /** Frazy do przeczytania w kreatorze. */
  readonly prompts: readonly LocalizedText[];
  readonly last_sample: EnrollSampleView | null;
  /** „Wymagaj weryfikacji głosu dla akcji ryzykownych". */
  readonly required_for_risky: boolean;
  readonly last_check: SpeakerCheckView | null;
}

export type DictationStateView = 'unavailable' | 'idle' | 'active' | 'paused';

/** Profil dyktowania dla programu (`notepad.exe`). */
export interface DictationProfile {
  readonly app: string;
  readonly capitalize_start: boolean;
  /** Terminale: „nowa linia" bez Entera. */
  readonly block_enter: boolean;
}

export interface DictationView {
  readonly state: DictationStateView;
  readonly reason: LocalizedText | null;
  readonly app: string | null;
  readonly typed_chars: number;
  readonly pending_chars: number;
  readonly can_undo: boolean;
  /** Ostatnia fraza (tylko w trakcie sesji). */
  readonly preview: string | null;
  readonly shortcut: string;
  readonly profiles: readonly DictationProfile[];
}

export type ReadStateView = 'unavailable' | 'idle' | 'speaking' | 'paused';

export interface ReadAloudView {
  readonly state: ReadStateView;
  readonly reason: LocalizedText | null;
  readonly app: string | null;
  readonly index: number;
  readonly segments: number;
  readonly rate: number;
  readonly queued: number;
  readonly agent: string;
  readonly shortcut: string;
}

/** Szybka rozmowa w chmurze (speech-to-speech) — dziś „wymaga klucza". */
export interface S2sView {
  readonly available: boolean;
  readonly reason: LocalizedText;
}

export interface VoiceFeatures {
  readonly wake: WakeWordsView;
  readonly speaker: SpeakerView;
  readonly dictation: DictationView;
  readonly read: ReadAloudView;
  readonly s2s: S2sView;
}

export type WakeAction =
  | {
      readonly kind: 'configure';
      readonly enabled: boolean;
      /** Bez pomiaru FAR/FRR włączenie wymaga jawnego „na własne ryzyko". */
      readonly accept_risk: boolean;
      readonly owner_gate: boolean;
    }
  | { readonly kind: 'test'; readonly on: boolean }
  | { readonly kind: 'set_dnd'; readonly on: boolean };

export type SpeakerAction =
  | { readonly kind: 'begin' }
  | { readonly kind: 'record_start' }
  | { readonly kind: 'record_stop' }
  | { readonly kind: 'finish' }
  | { readonly kind: 'cancel' }
  | { readonly kind: 'delete' }
  | { readonly kind: 'set_required'; readonly required: boolean };

export type DictationAction =
  | { readonly kind: 'start' }
  | { readonly kind: 'stop' }
  | { readonly kind: 'toggle' }
  | { readonly kind: 'undo' }
  | { readonly kind: 'save_profile'; readonly profile: DictationProfile }
  | { readonly kind: 'remove_profile'; readonly app: string };

export type ReadSource = 'selection' | 'document' | 'clipboard';
export type ReadControlAction =
  'pause' | 'resume' | 'next' | 'previous' | 'faster' | 'slower' | 'restart' | 'stop';

export type ReadAction =
  | { readonly kind: 'start'; readonly source: ReadSource }
  | { readonly kind: 'control'; readonly control: ReadControlAction }
  | { readonly kind: 'set_rate'; readonly rate: number };
