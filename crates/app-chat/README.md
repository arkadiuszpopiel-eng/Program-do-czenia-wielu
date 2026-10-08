# app-chat

Silnik tury czatu w aplikacji (kategoria app-*): generacja odpowiedzi (strumień dostawcy z renderowaniem przyrostowym, przebieg agentki z narzędziami, delegacja do mostu CLI), zapis tury append-only, koszt, oś czasu, stan agentek; wydzielony z app-core.

Warstwa kompozycji aplikacji (`app-*`): składa moduły w całość dla `app-core` i powłoki; nie jest modułem z trójką `contract/impl/fake`.

```bash
cargo test -p app-chat
```
