# tools-window-contract

tools-window: kontrakt narzędzi okien agentek (lista, fokus, położenie, stan) i wspólna bramka GUI (gui.control(aplikacja), strażnik okien Alfy/Brokera, weryfikacja po akcji, mapowanie błędów GUI) dla tools-uia/input/screen.

Kontrakt modułu `tools-window`: trait, typy, zdarzenia i współdzielone testy kontraktowe. Inne moduły zależą wyłącznie od tego crate'a.

Specyfikacja: [`docs/modules/tools-window/SPEC.md`](../../docs/modules/tools-window/SPEC.md).

```bash
cargo test -p tools-window-contract
```
