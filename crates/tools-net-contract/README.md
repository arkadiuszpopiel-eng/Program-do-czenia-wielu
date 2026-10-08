# tools-net-contract

tools-net: kontrakt narzędzi sieciowych agentek — net_fetch (GET/HEAD tylko https, bez hostów i adresów niepublicznych, każdy host i przekierowanie na inny host przez Brokera net.egress(host), limity rozmiaru i czasu, treść niezaufana), net_download (kwarantanna sesji, SHA-256, MOTW), net_search (port bez dostawcy); porty HttpPort i SearchPort.

Kontrakt modułu `tools-net`: trait, typy, zdarzenia i współdzielone testy kontraktowe. Inne moduły zależą wyłącznie od tego crate'a.

Specyfikacja: [`docs/modules/tools-net/SPEC.md`](../../docs/modules/tools-net/SPEC.md).

```bash
cargo test -p tools-net-contract
```
