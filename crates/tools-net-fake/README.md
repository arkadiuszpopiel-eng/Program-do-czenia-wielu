# tools-net-fake

tools-net: atrapy — narzędzia (prawdziwe manifesty i walidacja adresów z kontraktu, wyniki skryptowane, odczyty niezaufane) oraz wirtualna sieć dla HttpPort (trasy, przekierowania, treść nieskończona i wisząca, hosty z adresem niepublicznym, dziennik żądań) i wyszukiwarka skryptowana.

Atrapa modułu `tools-net` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/tools-net/SPEC.md`](../../docs/modules/tools-net/SPEC.md).

```bash
cargo test -p tools-net-fake
```
