# tools-browser-fake

tools-browser: atrapa narzędzi — prawdziwe manifesty i walidacja argumentów z kontraktu, wyniki skryptowane w pamięci (odczyty oznaczone jako niezaufane), zapis wywołań; bez Brokera, przeglądarki i sieci.

Atrapa modułu `tools-browser` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/tools-browser/SPEC.md`](../../docs/modules/tools-browser/SPEC.md).

```bash
cargo test -p tools-browser-fake
```
