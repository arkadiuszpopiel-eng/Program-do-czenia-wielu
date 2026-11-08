# app-modules

Adaptery portów aplikacji na modułach -impl (kategoria app-*): transfer .alfa z dialogami powłoki, Broker w procesie z dziennikiem cofania, głos (audio + TTS), silniki TTS z sidecarów, katalog i sonda dostawców, Router jako „mózg”, sejf kluczy.

Warstwa kompozycji aplikacji (`app-*`): składa moduły w całość dla `app-core` i powłoki; nie jest modułem z trójką `contract/impl/fake`.

```bash
cargo test -p app-modules
```
