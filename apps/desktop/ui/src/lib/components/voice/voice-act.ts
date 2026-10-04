// Akcje głosu rozszerzonego F5 z UI: wywołanie komendy, nowy widok do stanu okna, błąd → toast
// (np. odmowa w polu hasła, „nieskalibrowane — potwierdź ryzyko").
import type {
  DictationAction,
  ReadAction,
  SpeakerAction,
  VoiceFeatures,
  WakeAction,
} from '../../api/types-voice';
import type { AppState } from '../../state/app.svelte';

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
      app.toasts.show({
        kind: 'warning',
        message: error instanceof Error ? error.message : String(error),
      });
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
