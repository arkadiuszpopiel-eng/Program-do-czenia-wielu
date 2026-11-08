# plugin-runtime-fake

Atrapa wtyczek Wasm (F8-05): ten sam cykl życia z kontraktu, zachowania Rust zamiast Wasm per hash modułu, wirtualny zegar, nagrane zdarzenia i wywołania.

Atrapa modułu `plugin-runtime` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/plugin-runtime/SPEC.md`](../../docs/modules/plugin-runtime/SPEC.md).

```bash
cargo test -p plugin-runtime-fake
```
