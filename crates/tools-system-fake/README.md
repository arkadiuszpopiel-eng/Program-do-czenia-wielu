# tools-system-fake

tools-system: atrapa narzędzi systemowych — prawdziwe manifesty, walidacja argumentów i odmowy polityki z kontraktu, wyniki skryptowane w pamięci (odczyty oznaczone jako niezaufane), zapis wywołań; bez Brokera i bez systemu.

Atrapa modułu `tools-system` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/tools-system/SPEC.md`](../../docs/modules/tools-system/SPEC.md).

```bash
cargo test -p tools-system-fake
```
