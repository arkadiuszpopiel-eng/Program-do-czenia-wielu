# tools-office-fake

tools-office: atrapa narzędzi — prawdziwe manifesty i walidacja argumentów z kontraktu, wyniki skryptowane w pamięci (odczyty oznaczone jako niezaufane), zapis wywołań; bez Brokera, Office i we/wy.

Atrapa modułu `tools-office` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/tools-office/SPEC.md`](../../docs/modules/tools-office/SPEC.md).

```bash
cargo test -p tools-office-fake
```
