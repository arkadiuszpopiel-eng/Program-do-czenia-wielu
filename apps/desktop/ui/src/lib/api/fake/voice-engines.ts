// Atrapa silników rozmowy (`voice_engines`, pole `engines` w `VoiceFeatures`). Scenariusz domyślny
// pokazuje bezpiecznik po zadziałaniu — żeby dało się obejrzeć i wypróbować „Zamknij bezpiecznik";
// bez mikrofonu nie ma strażnika, a lepsze silniki czekają na pobranie.
import type { EnginesAction, VoiceEnginesView } from '../types-voice';

export function initialEngines(noMic: boolean): VoiceEnginesView {
  if (noMic) {
    return {
      stt: {
        engine: 'whisper',
        gpu: false,
        fallback: null,
        missing: ['qwen3-asr-1.7b-q8'],
        guard: null,
      },
      turn: { engine: 'text', missing: ['smart-turn-v3.2', 'onnxruntime-cpu'] },
      tts: [
        { engine: 'supertonic', ready: false },
        { engine: 'pocket', ready: false },
        { engine: 'piper', ready: true },
      ],
    };
  }
  return {
    stt: {
      engine: 'qwen_asr',
      gpu: true,
      fallback: 'whisper',
      missing: [],
      guard: {
        breaker: 'open',
        failures: 3,
        failures_to_open: 3,
        opened_count: 1,
        retry_in_s: 18,
        median_ms: 280,
        p95_ms: 910,
        samples: 24,
        fallbacks: 3,
        flagged: 1,
      },
    },
    turn: { engine: 'smart_turn', missing: [] },
    tts: [
      { engine: 'supertonic', ready: true },
      { engine: 'pocket', ready: false },
      { engine: 'piper', ready: true },
    ],
  };
}

/** Skutek akcji: zamknięcie bezpiecznika zeruje awarie i czas do następnej próby. */
export function applyEnginesAction(
  view: VoiceEnginesView,
  action: EnginesAction,
): VoiceEnginesView {
  if (action.kind !== 'reset_breaker' || !view.stt.guard) return view;
  return {
    ...view,
    stt: {
      ...view.stt,
      guard: { ...view.stt.guard, breaker: 'closed', failures: 0, retry_in_s: null },
    },
  };
}
