// Okno „Tablica” (paleta `Ctrl+K`): otwarte albo nie. Osobny moduł — stan aplikacji jest u limitu
// długości pliku.

export const boardDialog = $state<{ open: boolean }>({ open: false });

export function openBoard(): void {
  boardDialog.open = true;
}

export function closeBoard(): void {
  boardDialog.open = false;
}
