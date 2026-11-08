# ui-terminal-impl

ui-terminal (F4): menedżer wbudowanych terminali ConPTY nad PseudoConsolePort — wejście tylko z gestu użytkownika, wyjście VT prosto do kanału UI (bez logów, magistrali i pamięci), jawne środowisko bez sekretów, zamknięcie zabija drzewo procesów.

Implementacja modułu `ui-terminal`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

Specyfikacja: [`docs/modules/ui-terminal/SPEC.md`](../../docs/modules/ui-terminal/SPEC.md).

```bash
cargo test -p ui-terminal-impl
```
