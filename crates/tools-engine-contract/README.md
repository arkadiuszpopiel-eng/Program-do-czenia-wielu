# tools-engine-contract

tools-engine: kontrakt połączenia z silnikiem gier NextGO.Gray — port silnika, protokół JSON-RPC (adres nasłuchu, żądania, podpowiedzi błędów), manifesty i argumenty 15 narzędzi.

Kontrakt modułu `tools-engine`: trait, typy i współdzielone testy kontraktowe. Inne moduły zależą wyłącznie od tego crate'a.

Specyfikacja: [`docs/modules/tools-engine/SPEC.md`](../../docs/modules/tools-engine/SPEC.md).

```bash
cargo test -p tools-engine-contract
```
