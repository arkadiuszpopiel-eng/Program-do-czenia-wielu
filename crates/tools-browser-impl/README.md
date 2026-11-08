# tools-browser-impl

tools-browser: izolowana przeglądarka agentek przez BrowserPort — każdy host przez Brokera net.egress(host) (filtr per żądanie), deny-lista domen dostawców, treść niezaufana, bez haseł, pobrania w kwarantannie.

Implementacja modułu `tools-browser`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

Specyfikacja: [`docs/modules/tools-browser/SPEC.md`](../../docs/modules/tools-browser/SPEC.md).

```bash
cargo test -p tools-browser-impl
```
