# tools-engine-fake

tools-engine: atrapa silnika — świat w pamięci z rewizjami, idempotentnymi transakcjami, konfliktem rewizji i cofaniem, bez uruchamiania procesów.

Atrapa modułu `tools-engine` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/tools-engine/SPEC.md`](../../docs/modules/tools-engine/SPEC.md).

```bash
cargo test -p tools-engine-fake
```
