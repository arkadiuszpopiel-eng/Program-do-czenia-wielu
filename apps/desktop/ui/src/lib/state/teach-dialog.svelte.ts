// Okno „Naucz Alfę” (paleta `Ctrl+K`, panel Pamięć): otwarte albo nie. Osobny moduł — stan
// aplikacji jest u limitu długości pliku.

export const teachDialog = $state<{ open: boolean }>({ open: false });

export function openTeach(): void {
  teachDialog.open = true;
}

export function closeTeach(): void {
  teachDialog.open = false;
}
