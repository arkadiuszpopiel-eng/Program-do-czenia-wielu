# tools-net-impl

tools-net: sieć agentek bez przeglądarki — net_fetch/net_download/net_search przez Brokera net.egress(host) przy każdym hoście i przekierowaniu na inny host, klient HTTPS z lib-netguard (bez proxy i przekierowań, resolver odrzucający adresy niepubliczne — DNS rebinding), limity rozmiaru i czasu, pobieranie strumieniowe do kwarantanny sesji (fs.write) z SHA-256 i MOTW, treść niezaufana.

Implementacja modułu `tools-net`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

Specyfikacja: [`docs/modules/tools-net/SPEC.md`](../../docs/modules/tools-net/SPEC.md).

```bash
cargo test -p tools-net-impl
```
