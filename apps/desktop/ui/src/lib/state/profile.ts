// Profil modeli widoczny w oknie (pasek pisania, okienko stanu, załączniki): wybór zapisany w
// ustawieniach; bez kluczy API działa tylko profil lokalny, więc wtedy pokazujemy „Lokalny”.
import type { ModelProfile } from '../api/types';
import type { AppState } from './app.svelte';

export function currentProfile(app: AppState): ModelProfile {
  if (app.system?.keys_configured === false) return 'local';
  const fallback = app.system?.profile ?? app.sessions.active?.profile ?? 'hybrid';
  return app.str('models.default_profile', fallback) as ModelProfile;
}
