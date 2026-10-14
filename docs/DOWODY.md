# Rejestr dowodów

Plik generowany (`node scripts/evidence.mjs`) z raportów `scripts/verify.mjs`. Każdy wiersz to jeden bieg:
kiedy, na jakim sprzęcie, z którego commita i z jakim wynikiem. Logi etapów leżą lokalnie w
`target/verify/<wersja>/` (poza repozytorium). Zielony etap dowodzi tylko tego, co ten etap sprawdza —
testy na atrapach nie są pomiarem na sprzęcie; kryteria akceptacji i ich progi są w [`STATUS.md`](STATUS.md).

## Poziom „wersja”

| Wersja | Zakończony | Commit | Sprzęt | Wynik | Etapy zielone / czerwone / nieuruchomione |
| --- | --- | --- | --- | --- | --- |
| 0.0.51 | 2026-10-14 18:10 | — | brak w raporcie (starszy format raportu) | CZERWONY | 17 / 1 / 0 |
| 0.0.49 | 2026-10-12 15:45 | — | brak w raporcie (starszy format raportu) | ZIELONY | 10 / 0 / 0 |
| 0.0.48 | 2026-10-12 12:31 | — | brak w raporcie (starszy format raportu) | ZIELONY | 10 / 0 / 0 |
| 0.0.47 | 2026-10-12 11:00 | — | brak w raporcie (starszy format raportu) | ZIELONY | 10 / 0 / 0 |
| 0.0.46 | 2026-10-12 09:51 | — | brak w raporcie (starszy format raportu) | CZERWONY | 7 / 2 / 1 |

### Ostatni bieg: 0.0.51 (2026-10-14 18:10)

| Etap | Wynik | Liczby / powód | Polecenie | Log |
| --- | --- | --- | --- | --- |
| Spójność numeru wersji (pliki, manifesty, Cargo.lock) | zielony |  | — | — |
| Formatowanie Rust (workspace i powłoka) | zielony |  | — | — |
| Graf zależności modułów | zielony |  | — | — |
| Linter całego projektu | zielony |  | — | — |
| Linter powłoki Tauri (zwykła i testowa) | zielony |  | — | — |
| Testy całego projektu | zielony | 956 programów testowych, 2732 zielonych, 0 czerwonych, 34 pominiętych | — | — |
| Testy powłoki Tauri (zwykła i testowa) | zielony |  | — | — |
| Schematy JSON zgodne z Rustem | zielony |  | — | — |
| Tokeny wyglądu i kontrast | zielony |  | — | — |
| Lint interfejsu i skryptów (eslint, Svelte 5, CSS) | zielony |  | — | — |
| Typy łącza zgodne ze schematem | zielony |  | — | — |
| Kontrola typów interfejsu | zielony |  | — | — |
| Testy interfejsu | zielony | 356 passed (356) | — | — |
| Budowa, budżet rozmiaru i testy w przeglądarce | zielony | 357 passed (4.6m) | — | — |
| Budowa i test prawdziwej aplikacji (instancja testowa), pokrycie łącza | zielony | 28 passed (1.4m); pokrycie łącza 122 z 233 (52.4 %) | — | — |
| Plan w HTML: aktualność i odnośniki | zielony |  | — | — |
| Zależności i licencje (cargo deny) | CZERWONY |  | — | — |
| Podatności pakietów JS (pnpm audit) | zielony |  | — | — |

## Poziom „sprzet”

| Wersja | Zakończony | Commit | Sprzęt | Wynik | Etapy zielone / czerwone / nieuruchomione |
| --- | --- | --- | --- | --- | --- |
| 0.0.48 | 2026-10-12 12:40 | — | brak w raporcie (starszy format raportu) | ZIELONY | 5 / 0 / 0 |
| 0.0.47 | 2026-10-12 11:16 | — | brak w raporcie (starszy format raportu) | ZIELONY | 5 / 0 / 0 |
| 0.0.46 | 2026-10-12 10:05 | — | brak w raporcie (starszy format raportu) | CZERWONY | 3 / 2 / 0 |

### Ostatni bieg: 0.0.48 (2026-10-12 12:40)

| Etap | Wynik | Liczby / powód | Polecenie | Log |
| --- | --- | --- | --- | --- |
| Nagrania kontrolne 16 kHz | zielony |  | — | — |
| Testy sprzętowe | zielony | vad: zielony (1 zal., 0 obl., 50 s) · turn: zielony (1 zal., 0 obl., 8 s) · stt: zielony (1 zal., 0 obl., 18 s) · rozmowa: zielony (2 zal., 0 obl., 100 s) · mikrofon: zielony (1 zal., 0 obl., 84 s) · mowca: zielony (1 zal., 0 obl., 20 s) · embed: zielony (1 zal., 0 obl., 16 s) · slowo: POMINIĘTY (brak plików) · system: zielony (2 zal., 0 obl., 15 s) · rejestr: zielony (1 zal., 0 obl., 12 s) · markdown: zielony (1 zal., 0 obl., 16 s) · pobranie-motw: zielony (1 zal., 0 obl., 0 s) | — | — |
| Instancja testowa z kopią modelu | zielony |  | — | — |
| Próby obowiązków na lokalnym modelu z oceną | zielony | morning: zielony (62 s) · evening: zielony (46 s) · tidy: zielony (30 s) · costs: zielony (16 s) | — | — |
| Zamknięcie instancji testowej | zielony |  | — | — |

## Poziom „szybki”

| Wersja | Zakończony | Commit | Sprzęt | Wynik | Etapy zielone / czerwone / nieuruchomione |
| --- | --- | --- | --- | --- | --- |
| 0.0.45 | 2026-10-12 08:49 | — | brak w raporcie (starszy format raportu) | ZIELONY | 3 / 0 / 0 |

### Ostatni bieg: 0.0.45 (2026-10-12 08:49)

| Etap | Wynik | Liczby / powód | Polecenie | Log |
| --- | --- | --- | --- | --- |
| Formatowanie Rust | zielony |  | — | — |
| Kontrola typów interfejsu | zielony |  | — | — |
| Testy interfejsu | zielony | 336 passed (336) | — | — |

