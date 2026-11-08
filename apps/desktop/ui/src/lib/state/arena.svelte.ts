// Okno „Porównaj modele (Arena)” (paleta `Ctrl+K`, menu wiadomości): otwarte albo nie i której
// wiadomości dotyczy (`null` — ostatnia wiadomość użytkownika bieżącej ścieżki). Osobny moduł —
// stan aplikacji jest u limitu długości pliku.

export const arenaDialog = $state<{ open: boolean; turnId: string | null }>({
  open: false,
  turnId: null,
});

export function openArena(turnId: string | null = null): void {
  arenaDialog.turnId = turnId;
  arenaDialog.open = true;
}

export function closeArena(): void {
  arenaDialog.open = false;
}
