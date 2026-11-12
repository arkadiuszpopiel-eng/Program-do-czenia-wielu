# tools-browser-impl

tools-browser: izolowana przeglądarka agentek przez BrowserPort — każdy host przez Brokera net.egress(host) (filtr per żądanie), deny-lista domen dostawców, treść niezaufana, bez haseł, pobrania w kwarantannie.

Implementacja modułu `tools-browser`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

`browser_search` korzysta ze ścieżki `browser_open` i tych samych kontroli Brokera,
filtra hostów, kwarantanny i taintWeb. Nie przyznaje nowej zdolności ani dostępu do profilu
użytkownika. Wymaga jawnego silnika, nie kluczaAPI. Otwarta strona nie oznacza przeczytanych
wyników. Backend pozostaje oddzielnymEdge/Chrome; osadzenieWebView2 jest osobną pracą.

Specyfikacja: [`docs/modules/tools-browser/SPEC.md`](../../docs/modules/tools-browser/SPEC.md).

```bash
cargo test -p tools-browser-impl
```
