# tools-input-fake

tools-input: atrapa narzędzi — prawdziwe manifesty i walidacja argumentów z kontraktu, wyniki skryptowane w pamięci (odczyty oznaczone jako niezaufane), zapis wywołań; bez Brokera i bez we/wy.

Atrapa modułu `tools-input` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/tools-input/SPEC.md`](../../docs/modules/tools-input/SPEC.md).

```bash
cargo test -p tools-input-fake
```
