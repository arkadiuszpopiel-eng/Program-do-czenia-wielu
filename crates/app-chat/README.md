# app-chat

Silnik tury czatu w aplikacji (kategoria app-*): generacja odpowiedzi (strumień dostawcy z renderowaniem przyrostowym, przebieg agentki z narzędziami, delegacja do mostu CLI), zapis tury append-only, koszt, oś czasu, stan agentek; wydzielony z app-core.

Warstwa kompozycji aplikacji (`app-*`): składa moduły w całość dla `app-core` i powłoki; nie jest modułem z trójką `contract/impl/fake`.

Arena (P4, `arena.rs`): odpowiedź z modelem przypiętym w Routerze (`start_pinned_generation`, zapis `dostawca:model`) jako nowa gałąź pod wiadomością użytkownika i wybór gałęzi (`select_branch` — aktywny liść sesji).

Projekcja historii (`history.rs`): przed końcową wiadomością użytkownika dodaje dane
`app-store::execution_records` wybranej gałęzi. Pierwsza wiadomość bez wcześniejszej
tury agentki nie dostaje noty. Błędy i markery po narzędziach także są uwzględniane;
brak metadanych pozostaje stanem nieznanym. Odczyt odbywa się na istniejącym wątku
blokującym przygotowania historii, przed zwykłym wyborem trasy i kontrolą prywatności.
Nie zmienia tekstu wcześniejszych odpowiedzi, celu, listy narzędzi, zapisanej historii
ani odpowiedzi modelu; nie tworzy syntetycznych ToolUse/ToolResult. Przekazanie danych
nie jest dowodem, że rzeczywisty model poprawnie je zinterpretuje — #58 wymaga osobnej
oceny jakości i odbioru sprzętowego.

```bash
cargo test -p app-chat
```
