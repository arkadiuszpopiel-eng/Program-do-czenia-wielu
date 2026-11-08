# tools-window-fake

tools-window: atrapa narzędzi — prawdziwe manifesty i walidacja argumentów z kontraktu, wyniki skryptowane w pamięci (odczyty oznaczone jako niezaufane), zapis wywołań; bez Brokera i bez we/wy.

Atrapa modułu `tools-window` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/tools-window/SPEC.md`](../../docs/modules/tools-window/SPEC.md).

```bash
cargo test -p tools-window-fake
```
