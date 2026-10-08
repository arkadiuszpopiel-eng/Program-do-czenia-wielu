# tools-fs-fake

tools-fs: atrapa narzędzi plikowych — prawdziwe manifesty, walidacja argumentów, skryptowane wyniki bez we/wy (testy agent-runtime, UI).

Atrapa modułu `tools-fs` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/tools-fs/SPEC.md`](../../docs/modules/tools-fs/SPEC.md).

```bash
cargo test -p tools-fs-fake
```
