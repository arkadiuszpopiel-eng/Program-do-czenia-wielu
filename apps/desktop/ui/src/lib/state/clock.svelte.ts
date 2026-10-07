// Wspólny „teraz" dla względnych znaczników czasu — jeden zegar na okno, tyka co 30 s,
// tylko gdy ktoś go czyta (brak pracy w bezczynności, gdy nic nie jest wyświetlane).
import { createSubscriber } from 'svelte/reactivity';

const subscribe = createSubscriber((update) => {
  const handle = setInterval(update, 30_000);
  return () => clearInterval(handle);
});

export function now(): number {
  subscribe();
  return Date.now();
}
