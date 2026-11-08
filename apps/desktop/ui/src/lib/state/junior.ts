// Styl „Junior” (etap K, plan 41): po wybraniu stylu Alfa proponuje — jednym przyciskiem, bez
// zmiany po cichu — wolniejszą mowę, dźwięki sygnałowe i uproszczony ekran. Każde z tych ustawień
// zostaje zwykłym ustawieniem Głosu, Powiadomień i Wyglądu (można je potem zmienić osobno).
import type { AppState } from './app.svelte';

/** Tempo mowy dla dziecka (%, `voice.tts_speed`: 60–160, naturalne 100). */
export const JUNIOR_SPEED = 85;

/** Czy głos, dźwięki i ekran są już ustawione jak dla dziecka. */
export function juniorVoiceReady(app: AppState): boolean {
  return (
    app.num('voice.tts_speed', 100) <= JUNIOR_SPEED &&
    app.bool('notify.earcons', true) &&
    app.bool('ui.simple', false)
  );
}

/** Toast z propozycją po przełączeniu na styl „Junior” (tylko gdy jest co zmienić). */
export function offerJunior(app: AppState): void {
  if (juniorVoiceReady(app)) return;
  const { t } = app.i18n;
  app.toasts.show({
    key: 'junior-offer',
    kind: 'info',
    message: t('junior.offer', { speed: JUNIOR_SPEED }),
    actionLabel: t('junior.offerAction'),
    onAction: () => void applyJuniorVoice(app),
    timeoutMs: 15_000,
  });
}

export async function applyJuniorVoice(app: AppState): Promise<void> {
  const speed =
    app.num('voice.tts_speed', 100) <= JUNIOR_SPEED ||
    (await app.setSetting('voice.tts_speed', JUNIOR_SPEED));
  const sounds = app.bool('notify.earcons', true) || (await app.setSetting('notify.earcons', true));
  const simple = app.bool('ui.simple', false) || (await app.setSetting('ui.simple', true));
  if (speed && sounds && simple) {
    app.toasts.show({
      key: 'junior-offer',
      kind: 'success',
      message: app.i18n.t('junior.applied', { speed: JUNIOR_SPEED }),
    });
  }
}
