# tools-common-contract

Wspólny kontrakt narzędzi agentek (tools-*): manifest (JSON Schema, odwracalność, zdolności), trait Tool, protokół Brokera (decide → zatwierdzenie z limitem → verify), wyniki z oznaczeniem niezaufanej treści, redakcja sekretów, ścieżki.

Kontrakt modułu `tools-common`: trait, typy, zdarzenia i współdzielone testy kontraktowe. Inne moduły zależą wyłącznie od tego crate'a.

Specyfikacja: [`docs/modules/tools-common/SPEC.md`](../../docs/modules/tools-common/SPEC.md).

```bash
cargo test -p tools-common-contract
```
