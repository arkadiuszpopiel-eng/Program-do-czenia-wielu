import { mount } from 'svelte';
import { installDefaultPolicy } from '@alfa/ui-kit';
import '@alfa/ui-kit/tokens.css';
import '@alfa/ui-kit/base.css';
import './app.css';
import App from './App.svelte';
import { createClient } from './lib/api';
import { errorText } from './lib/api/command-error';
import { AppState } from './lib/state/app.svelte';

// Trusted Types: polityka `default` tylko dla workera podświetlania (CSP wydania, PT-33).
installDefaultPolicy();

const target = document.getElementById('app');
if (!target) throw new Error('Brak elementu #app');

const client = await createClient();
const app = new AppState(client);
// Siatka bezpieczeństwa: odrzucenie bez obsługi (np. błąd komendy rdzenia) nie przepada po cichu.
// Widoki nadal obsługują błędy u siebie (kontekst, stan pól); anulowanie (AbortError) to nie błąd.
window.addEventListener('unhandledrejection', (event) => {
  const reason: unknown = event.reason;
  if (reason instanceof DOMException && reason.name === 'AbortError') return;
  // Znacznik dla audytu E2E (`e2e/core-errors.spec.ts`): widok nie obsłużył błędu u siebie.
  console.warn('[alfa] nieobsłużone odrzucenie:', errorText(reason));
  app.toasts.show({ kind: 'error', message: errorText(reason) });
});
mount(App, { target, props: { app } });
void app.start();
