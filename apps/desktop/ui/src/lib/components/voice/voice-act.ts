// Akcje głosu rozszerzonego F5 z UI: wywołanie komendy, nowy widok do stanu okna, błąd → toast
// (np. odmowa w polu hasła, „nieskalibrowane — potwierdź ryzyko").
import type {
  DictationAction,
  ReadAction,
  SpeakerAction,
  VoiceFeatures,
  WakeAction,
} from '../../api/types-voice';
import { errorText } from '../../api/command-error';
import type { AppState } from '../../state/app.svelte';
import { load } from '../../state/attempt';

export interface VoiceActions {
  wake(action: WakeAction): Promise<boolean>;
  speaker(action: SpeakerAction): Promise<boolean>;
  dictation(action: DictationAction): Promise<boolean>;
  read(action: ReadAction): Promise<boolean>;
}

export function voiceActions(app: AppState): VoiceActions {
  const api = app.client.voiceFeatures;
  async function run(call: Promise<VoiceFeatures>): Promise<boolean> {
    try {
      app.voice.applyFeatures(await call);
      return true;
    } catch (error) {
      app.toasts.show({ kind: 'warning', message: errorText(error) });
      return false;
    }
  }
  return {
    wake: (action) => run(api.wake(action)),
    speaker: (action) => run(api.speaker(action)),
    dictation: (action) => run(api.dictation(action)),
    read: (action) => run(api.read(action)),
  };
}

/**
 * Wczytuje stan głosu F5 do stanu okna. Zwraca komunikat błędu (widok pokazuje go z „Ponów"
 * zamiast „Ładowanie…" na zawsze) albo `null` po sukcesie.
 */
export async function loadVoiceFeatures(app: AppState): Promise<string | null> {
  const result = await load(() => app.client.voiceFeatures.features());
  if (result.status === 'failed') return result.error;
  if (result.status === 'ready') app.voice.applyFeatures(result.value);
  return null;
}
