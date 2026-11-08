// Widok panelu Sesje (lista albo karty, P4) zapamiętany na tym komputerze. Osobno od logiki kart,
// żeby pakiet startowy okna nie zawierał kodu widoku ładowanego leniwie.
export type SessionsView = 'list' | 'cards';

const VIEW_KEY = 'alfa.sessions.view';
/** Widok z poprzedniego uruchomienia (pamięć tego komputera); brak albo błąd — lista. */
export function readSessionsView(storage: Pick<Storage, 'getItem'> | null): SessionsView {
  try {
    return storage?.getItem(VIEW_KEY) === 'cards' ? 'cards' : 'list';
  } catch {
    return 'list';
  }
}

export function writeSessionsView(storage: Pick<Storage, 'setItem'> | null, view: SessionsView) {
  try {
    storage?.setItem(VIEW_KEY, view);
  } catch {
    // Pamięć przeglądarki niedostępna — widok działa do zamknięcia okna.
  }
}
