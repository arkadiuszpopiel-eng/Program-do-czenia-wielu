// Interfejs głosu rozszerzonego F5 (część `AlfaClient`; komendy `voice_features`, `voice_wake`,
// `voice_speaker`, `voice_dictation`, `voice_read`, `voice_engines` w COMMANDS.md). Każda akcja zwraca nowy widok;
// zmiany w tle (wykrycia, postęp czytania, dyktowanie) przychodzą zdarzeniem `VoiceFeaturesChanged`.
import type {
  DictationAction,
  EnginesAction,
  ReadAction,
  SpeakerAction,
  VoiceFeatures,
  WakeAction,
} from './types-voice';

export interface VoiceFeaturesApi {
  /** Stan: słowa wywoławcze, weryfikacja głosu, dyktowanie, czytanie, S2S. */
  features(): Promise<VoiceFeatures>;
  /** Słowa wywoławcze: jawne włączenie (bez pomiaru — z „na własne ryzyko"), test, DND. */
  wake(action: WakeAction): Promise<VoiceFeatures>;
  /** Kreator rejestracji głosu, usunięcie profilu, „wymagaj weryfikacji". */
  speaker(action: SpeakerAction): Promise<VoiceFeatures>;
  /** Dyktowanie do okna na pierwszym planie; profile aplikacji. */
  dictation(action: DictationAction): Promise<VoiceFeatures>;
  /** Czytanie zaznaczenia / dokumentu / schowka; sterowanie i tempo. */
  read(action: ReadAction): Promise<VoiceFeatures>;
  /** Silniki rozmowy: odczyt stanu albo zamknięcie bezpiecznika rozpoznawania. */
  engines(action: EnginesAction): Promise<VoiceFeatures>;
}
