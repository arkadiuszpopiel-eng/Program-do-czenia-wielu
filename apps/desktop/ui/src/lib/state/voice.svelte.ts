// Stan trybu głosowego w oknie głównym: dostępność (bez modeli — powód), mikrofon i pigułka
// (kto mówi, poziom, transkrypt częściowy); F5 — głos rozszerzony (słowa wywoławcze, weryfikacja
// głosu, dyktowanie, czytanie) z rdzenia (`voice_features`, zdarzenie `VoiceFeaturesChanged`).
import type { MicState } from '@alfa/ui-kit';
import type { AlfaClient } from '../api/client';
import type { VoiceStatus } from '../api/types';
import type { VoicePillState } from '../api/types-system';
import type { VoiceFeatures } from '../api/types-voice';

export class VoiceUiState {
  status = $state<VoiceStatus | null>(null);
  pill = $state<VoicePillState | null>(null);
  mic = $state<MicState>('off');
  level = $state(0);
  /** Głos rozszerzony F5 (`null` — jeszcze nie wczytany). */
  features = $state<VoiceFeatures | null>(null);

  get available(): boolean {
    return this.status?.state !== 'unavailable';
  }

  /** Trwa czytanie na głos (Esc je zatrzymuje). */
  get reading(): boolean {
    const r = this.features?.read.state;
    return r === 'speaking' || r === 'paused';
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

  applyFeatures(features: VoiceFeatures): void {
    this.features = features;
  }

  /**
   * Esc: stop czytania z kolejką (rdzeń czyści też czytany tekst z pamięci). Błąd trafia do
   * `onError` (toast) — Esc, który po cichu nie zatrzymał czytania, wyglądałby na działający.
   */
  stopReading(client: AlfaClient, onError: (error: unknown) => void): void {
    void client.voiceFeatures
      .read({ kind: 'control', control: 'stop' })
      .then((f) => this.applyFeatures(f), onError);
  }
}
