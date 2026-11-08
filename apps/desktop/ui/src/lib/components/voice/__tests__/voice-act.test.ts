// Akcje i wczytywanie głosu F5: błąd rdzenia daje komunikat (toast albo „Ponów"), nigdy ciszę.
import { describe, expect, it, vi } from 'vitest';
import { setupApp } from '../../../state/__tests__/helpers';
import { loadVoiceFeatures, voiceActions } from '../voice-act';

const coreError = { code: 'unavailable', message: 'Rdzeń głosu nie odpowiada' };

describe('voice-act', () => {
  it('loadVoiceFeatures: sukces zapisuje stan i zwraca null', async () => {
    const { app } = setupApp();
    expect(await loadVoiceFeatures(app)).toBeNull();
    expect(app.voice.features).not.toBeNull();
  });

  it('loadVoiceFeatures: błąd rdzenia → komunikat do „Ponów", bez toastu', async () => {
    const { app, client } = setupApp();
    vi.spyOn(client.voiceFeatures, 'features').mockRejectedValue(coreError);
    expect(await loadVoiceFeatures(app)).toBe(coreError.message);
    expect(app.voice.features).toBeNull();
    expect(app.toasts.items).toEqual([]);
  });

  it('voiceActions: odmowa → false i toast z komunikatem rdzenia', async () => {
    const { app, client } = setupApp();
    vi.spyOn(client.voiceFeatures, 'wake').mockRejectedValue(coreError);
    const ok = await voiceActions(app).wake({ kind: 'set_dnd', on: true });
    expect(ok).toBe(false);
    expect(app.toasts.items.map((t) => t.message)).toEqual([coreError.message]);
  });
});
