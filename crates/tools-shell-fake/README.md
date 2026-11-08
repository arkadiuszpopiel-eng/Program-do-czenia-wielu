# tools-shell-fake

tools-shell: atrapa narzędzia powłoki — prawdziwe manifesty, walidacja argumentów, skryptowane wyniki poleceń bez uruchamiania procesów.

Atrapa modułu `tools-shell` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/tools-shell/SPEC.md`](../../docs/modules/tools-shell/SPEC.md).

```bash
cargo test -p tools-shell-fake
```
