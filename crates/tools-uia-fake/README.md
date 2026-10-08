# tools-uia-fake

tools-uia: atrapa narzędzi — prawdziwe manifesty i walidacja argumentów z kontraktu, wyniki skryptowane w pamięci (odczyty oznaczone jako niezaufane), zapis wywołań; bez Brokera i bez we/wy.

Atrapa modułu `tools-uia` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/tools-uia/SPEC.md`](../../docs/modules/tools-uia/SPEC.md).

```bash
cargo test -p tools-uia-fake
```
