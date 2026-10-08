# app-plugins

Wtyczki Wasm w aplikacji (kategoria app-*, F8-05): plugin-runtime-impl nad Brokerem, magazynem w katalogu danych Alfy i hostem operacji (pliki przez dziennik cofania, sieć przez klienta z egress-allowlistą — oba z własnym Broker::verify), narzędzia do rejestru agentek, komendy plugins_* i problemy dla Diagnosty.

Warstwa kompozycji aplikacji (`app-*`): składa moduły w całość dla `app-core` i powłoki; nie jest modułem z trójką `contract/impl/fake`.

```bash
cargo test -p app-plugins
```
