# tools-screen-fake

tools-screen: atrapa narzędzi — prawdziwe manifesty i walidacja argumentów z kontraktu, wyniki skryptowane w pamięci (odczyty oznaczone jako niezaufane), zapis wywołań; bez Brokera i bez we/wy.

Atrapa modułu `tools-screen` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/tools-screen/SPEC.md`](../../docs/modules/tools-screen/SPEC.md).

```bash
cargo test -p tools-screen-fake
```
