# tools-uia-contract

tools-uia: kontrakt narzędzi UI Automation agentek — drzewo okna, wyszukiwanie, tekst tylko do odczytu (treść niezaufana), akcje przez wzorce (Invoke, SetValue, Toggle, ExpandCollapse, SelectionItem, Scroll).

Kontrakt modułu `tools-uia`: trait, typy, zdarzenia i współdzielone testy kontraktowe. Inne moduły zależą wyłącznie od tego crate'a.

Specyfikacja: [`docs/modules/tools-uia/SPEC.md`](../../docs/modules/tools-uia/SPEC.md).

```bash
cargo test -p tools-uia-contract
```
