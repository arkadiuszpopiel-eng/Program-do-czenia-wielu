// Okno „Tablica” (paleta `Ctrl+K`): otwarte albo nie. Osobny moduł — stan aplikacji jest u limitu
// długości pliku.

export const boardDialog = $state<{ open: boolean }>({ open: false });

export function openBoard(): void {
  boardDialog.open = true;
}

export function closeBoard(): void {
  boardDialog.open = false;
}

export const projectsDialog = $state<{
  open: boolean;
  sessionId: string | null;
  projectName: string | null;
}>({ open: false, sessionId: null, projectName: null });

export function openProjects(opts?: {
  sessionId?: string | null;
  projectName?: string | null;
}): void {
  projectsDialog.sessionId = opts?.sessionId ?? null;
  projectsDialog.projectName = opts?.projectName ?? null;
  projectsDialog.open = true;
}

export function closeProjects(): void {
  projectsDialog.open = false;
  projectsDialog.sessionId = null;
  projectsDialog.projectName = null;
}

/** Okno „Pipeline'y” (biblioteka łańcuchów modeli). */
export const pipelinesDialog = $state<{ open: boolean; name: string | null }>({
  open: false,
  name: null,
});

export function openPipelines(name: string | null = null): void {
  pipelinesDialog.name = name;
  pipelinesDialog.open = true;
}

export function closePipelines(): void {
  pipelinesDialog.open = false;
  pipelinesDialog.name = null;
}
