// Okno „Wstaw szablon” (paleta `Ctrl+K`, Ustawienia → Szablony promptów): otwarte i wybrany
// szablon. Osobny moduł, bo stan aplikacji jest u limitu długości pliku.

export const templateDialog = $state<{ open: boolean; id: string | null }>({
  open: false,
  id: null,
});

/** Otwiera okno (opcjonalnie z wybranym szablonem). */
export function openTemplates(id: string | null = null): void {
  templateDialog.id = id;
  templateDialog.open = true;
}

export function closeTemplates(): void {
  templateDialog.open = false;
  templateDialog.id = null;
}
