// Profile w rzeczywistej aplikacji; bez modelu, mikrofonu i zmiany poziomu autonomii.
import { expect, shot, test } from './alfa';
import { call } from './ipc-call';
import { jarvisFlow } from './jarvis-flow';

test('Jarvis: Kreator, cztery persony i Głos zachowują uprawnienia oraz mikrofon', async ({
  alfa,
}) => {
  const { main } = alfa;
  const permissions = await call<{ global: string }>(main, 'permissions_get', { sessionId: null });
  const voice = await call<{ state: string }>(main, 'voice_status');
  const originals = new Map<string, string>();
  let primaryError: unknown;
  try {
    await test.step('profile Jarvisa i nawigacja Głosu', () =>
      jarvisFlow(
        main,
        (key, value) => originals.set(key, value),
        () => shot(alfa, '52b-jarvis-kreator'),
      ));
    expect(
      (await call<{ global: string }>(main, 'permissions_get', { sessionId: null })).global,
    ).toBe(permissions.global);
    expect((await call<{ state: string }>(main, 'voice_status')).state).toBe(voice.state);
  } catch (error) {
    primaryError = error;
  }
  // Sprzątanie wszystkich profili kończy się przed zgłoszeniem pierwotnej porażki.
  let cleanupError: unknown;
  for (const [key, value] of originals) {
    try {
      await call(main, 'settings_set', { key, value });
    } catch (error) {
      cleanupError ??= error;
    }
  }
  if (primaryError) {
    if (cleanupError) console.warn('[jarvis] Przywracanie profili również nieudane');
    throw primaryError;
  }
  if (cleanupError) throw cleanupError;
});
