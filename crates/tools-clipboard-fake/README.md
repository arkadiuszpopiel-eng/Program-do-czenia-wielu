# tools-clipboard-fake

tools-clipboard: atrapa narzędzi schowka — prawdziwe manifesty, walidacja argumentów, schowek w pamięci z cofaniem zapisu (bez Brokera).

Atrapa modułu `tools-clipboard` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/tools-clipboard/SPEC.md`](../../docs/modules/tools-clipboard/SPEC.md).

```bash
cargo test -p tools-clipboard-fake
```
