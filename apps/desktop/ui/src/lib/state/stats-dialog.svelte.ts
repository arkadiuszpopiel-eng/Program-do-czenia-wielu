// Okno „Statystyki rozmowy” (paleta `Ctrl+K`): otwarte albo nie. Osobny moduł — stan aplikacji
// jest u limitu długości pliku.

export const statsDialog = $state<{ open: boolean }>({ open: false });

export function openStats(): void {
  statsDialog.open = true;
}

export function closeStats(): void {
  statsDialog.open = false;
}
