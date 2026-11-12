import { builtinPerson } from '@alfa/ui-kit';
import type { AppState } from '../state/app.svelte';

/** Karta własnej persony prowadzi do Kreatora z jej wspólnym profilem współpracy. */
export function openPersonControls(
  app: Pick<AppState, 'selectedAgent' | 'openPanel' | 'openSettings'>,
  id: string,
): void {
  if (builtinPerson(id)) {
    app.selectedAgent = id;
    app.openPanel('agents');
  } else app.openSettings('builder');
}
