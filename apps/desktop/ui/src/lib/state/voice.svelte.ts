// Stan trybu głosowego w oknie głównym: dostępność (bez modeli — powód), mikrofon i pigułka
// (kto mówi, poziom, transkrypt częściowy).
import type { MicState } from '@alfa/ui-kit';
import type { VoiceStatus } from '../api/types';
import type { VoicePillState } from '../api/types-system';

export class VoiceUiState {
  status = $state<VoiceStatus | null>(null);
  pill = $state<VoicePillState | null>(null);
  mic = $state<MicState>('off');
  level = $state(0);

  get available(): boolean {
    return this.status?.state !== 'unavailable';
  }

  applyPill(state: VoicePillState): void {
    this.pill = state;
    this.mic = state.mic;
    this.level = state.level;
  }

  applyStatus(status: VoiceStatus): void {
    this.status = status;
    if (status.state !== 'active') this.mic = status.muted ? 'muted' : 'off';
  }
}
