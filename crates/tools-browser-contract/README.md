# tools-browser-contract

tools-browser: kontrakt narzędzi przeglądarki agentek — izolowany profil, otwarcie adresu po zgodzie Brokera net.egress(host), migawka dostępności i tekst (treść niezaufana), kliknięcie i wpisywanie (bez haseł), zrzut, pobrania w kwarantannie.

Kontrakt modułu `tools-browser`: trait, typy, zdarzenia i współdzielone testy kontraktowe. Inne moduły zależą wyłącznie od tego crate'a.

`browser_search` otwiera stronę wybranego jawnie silnika (DuckDuckGo/Google/Bing), bezAPI.
Fraza pozostaje jedną zakodowaną wartością q; obowiązuje istniejący limitURL. Wynik
`navigation_only` nie jest rankingiem lub potwierdzeniem przeczytania źródeł — do tego
potrzebny `browser_read` i ocena treści. [Zakres decyzji](../../docs/reviews/2026-11-12-owner-decisions.md).

Specyfikacja: [`docs/modules/tools-browser/SPEC.md`](../../docs/modules/tools-browser/SPEC.md).

```bash
cargo test -p tools-browser-contract
```
