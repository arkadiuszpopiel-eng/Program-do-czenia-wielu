# ui-terminal-fake

ui-terminal: atrapa usługi terminali dla testów UI/korzenia kompozycji — echo wejścia, skryptowane wyjście, zamknięcie z powiadomieniem, zapis profili (bez procesów).

Atrapa modułu `ui-terminal` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/ui-terminal/SPEC.md`](../../docs/modules/ui-terminal/SPEC.md).

```bash
cargo test -p ui-terminal-fake
```
