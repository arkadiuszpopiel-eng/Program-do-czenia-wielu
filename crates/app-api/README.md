# app-api

Kontrakt IPC aplikacji (kategoria app-*): DTO komend i zdarzeń (COMMANDS.md), błąd komend, identyfikatory DTO, paczki zdarzeń co klatkę, porty powłoki i modułów, protokół alfa://, powiadomienia natywne.

Warstwa kompozycji aplikacji (`app-*`): składa moduły w całość dla `app-core` i powłoki; nie jest modułem z trójką `contract/impl/fake`.

```bash
cargo test -p app-api
```
