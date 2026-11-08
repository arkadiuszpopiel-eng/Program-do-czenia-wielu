# ui-terminal-contract

ui-terminal (F4): kontrakt wbudowanego terminala ConPTY do logowania w CLI mostów — sterowany wyłącznie przez użytkownika (gest UI), strumień VT prosto do UI (bez magistrali, logów i pamięci), zamknięcie zabija drzewo procesów.

Kontrakt modułu `ui-terminal`: trait, typy, zdarzenia i współdzielone testy kontraktowe. Inne moduły zależą wyłącznie od tego crate'a.

Specyfikacja: [`docs/modules/ui-terminal/SPEC.md`](../../docs/modules/ui-terminal/SPEC.md).

```bash
cargo test -p ui-terminal-contract
```
