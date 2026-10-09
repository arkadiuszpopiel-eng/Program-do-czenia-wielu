# Przegląd systemu kluczy API — 2026-10-09 (wersja 0.0.7)

Powód: zgłoszenie właściciela — dodanie klucza OpenAI kończyło się napisem „401 Klucz odrzucony” bez wyjaśnienia.
Po przebudowie system przeszedł trzy przeglądy w świeżym kontekście (ADR 0016): dwa w trakcie pracy i jeden na
końcu. Ten dokument opisuje ostatni (19 ustaleń) i mówi, co z każdym zrobiono. Żaden przegląd nie uruchamiał
prawdziwych zapytań do dostawców — zachowanie dostawców jest opisane z dokumentacji i pamięci.

## Ustalenia i decyzje

| Nr  | Waga          | Ustalenie                                                                                                                       | Co zrobiono                                                                                                                                                                           |
| --- | ------------- | ------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | wysoka        | OpenRouter oddaje listę modeli bez klucza — test „przechodził” dla dowolnego tekstu                                             | wynik „nie da się sprawdzić z góry” zamiast „klucz działa” (`probe.rs`, `PUBLIC_MODEL_LISTS`); pełne sprawdzenie przez `/key` — **otwarte** (wymaga nowej metody adaptera)            |
| 2   | wysoka        | limit zapytań (429) był traktowany jak dowód, że nowy klucz działa — wymiana mogła zastąpić dobry klucz błędnym                 | wymiana zapisuje kandydata tylko wtedy, gdy dostawca go rozpoznał albo nie da się go sprawdzić (`TestOutcome::accepts_new_key`); teksty już nie twierdzą, że „klucz jest w porządku”  |
| 3   | średnia       | zły klucz zgłaszany kodem 400 (Google, xAI) i nietypowe ciała błędów kończyły jako „błąd połączenia”                            | `lib-openai-compat` czyta tablicę, `error` jako tekst oraz `message`/`detail`; `probe.rs` rozpoznaje „API key not valid” i podobne                                                    |
| 4   | średnia       | dwie linie pliku `.env` sklejały się w jeden „klucz” i klucz jednego dostawcy mógł pójść do drugiego                            | sklejany jest tylko zawinięty wiersz; dwa wpisy zostają rozdzielone i nie są kluczem (Rust i TypeScript, wspólna tabela)                                                              |
| 5   | średnia       | nie dało się wymienić klucza, gdy nowego nie można sprawdzić (własny adres bez listy modeli)                                    | „nie da się sprawdzić” i „brak środków” zapisują nowy klucz; panel ma osobne komunikaty i ponowny test                                                                                |
| 6   | średnia       | konto bez klucza w magazynie pokazywało zielone „działa” z dawnego testu                                                        | stan „nieskonfigurowane” ma pierwszeństwo przed wynikiem ostatniego testu                                                                                                             |
| 7   | średnia/niska | krótki zamaskowany fragment (`****abcd`) przechodził do interfejsu i na dysk                                                    | maskowany jest każdy zamaskowany fragment, bez względu na długość                                                                                                                     |
| 8   | niska/średnia | po wymianie na klucz z ograniczeniem zapis modelu nie odświeżał panelu                                                          | panel bierze listę modeli z odpowiedzi polecenia                                                                                                                                      |
| 9   | niska/średnia | „nie da się sprawdzić” nie miało ani „Dalej”, ani pola modelu                                                                   | traktowane jak klucz z ograniczeniem: pole modelu i „Dalej”                                                                                                                           |
| 10  | niska         | odmowa „z innego powodu” omijała redakcję pełnego klucza; kod błędu bez limitu długości                                         | dodane do redakcji; kod: tylko drukowalne ASCII, do 64 znaków                                                                                                                         |
| 11  | niska         | po udanej wymianie nie było potwierdzenia zapisu                                                                                | komunikat pokazywany także przy wyniku „działa”                                                                                                                                       |
| 12  | niska         | `…KEY==` (koniec klucza base64) i `apikey:tajne` były okaleczane                                                                | nazwa zmiennej jest zdejmowana tylko przed czymś, co wygląda jak wartość                                                                                                              |
| 13  | niska         | Rust i TypeScript różniły się na znaku U+0085 i literach spoza ASCII w nazwie zmiennej                                          | wyrównane, przypadki we wspólnej tabeli                                                                                                                                               |
| 14  | niska         | fokus przeskakiwał na tytuł wyniku przy każdym odświeżeniu listy                                                                | fokus tylko przy zmianie wyniku                                                                                                                                                       |
| 15  | niska         | model wpisany ręcznie kasował listę od dostawcy; pusta lista kasowała model ręczny                                              | ręczny model tylko dla konta bez listy; pusta lista niczego nie kasuje                                                                                                                |
| 16  | niska         | pliki tekstów interfejsu przekroczyły 400 linii                                                                                 | teksty kreatora i kont w `pl-hub.ts` / `en-hub.ts`                                                                                                                                    |
| 17  | niska         | atrapa interfejsu odbiegała od rdzenia                                                                                          | wyrównane: reguła wymiany, włączanie po „bez testu”, porównanie adresów, reguła adresu. **Otwarte:** atrapa nie odtwarza braku klucza w magazynie ani odmowy dla konta bez klucza API |
| 18  | niska         | nieścisłe teksty (PL i EN)                                                                                                      | poprawione                                                                                                                                                                            |
| 19  | niska         | `http://localhost:80@inny.host` przechodziło regułę adresu; przekierowania; test konta zabronionego dostawcy; adres ze spacjami | host jest wyłuskiwany z adresu i `@` jest odrzucane; klient HTTP nie podąża za przekierowaniami; test i wymiana sprawdzają zgodność; adres jest przycinany                            |

## Co zostaje otwarte

- **OpenRouter:** uwierzytelnione sprawdzenie klucza (`GET /api/v1/key`). Dziś Alfa uczciwie mówi, że klucza nie da się
  sprawdzić z góry.
- **DeepSeek:** adres w katalogu to `https://api.deepseek.com/v1`; lista modeli pod `/v1/models` nie była sprawdzona
  prawdziwym zapytaniem. Jeśli jej nie ma, wynik będzie „nie da się sprawdzić z góry”, a nie błąd.
- **Prawdziwe klucze:** żaden test nie używa prawdziwego klucza (zasada bezpieczeństwa sesji). Ostateczne potwierdzenie
  to próba właściciela w działającej aplikacji.
- **Atrapa interfejsu** nie odtwarza stanu „brak klucza w magazynie” (ustalenie 17).
- Zadania „klucze dla firm” C2.12–C2.17 w [`docs/plan/03-roadmapa.md`](../plan/03-roadmapa.md).

## Sprawdzone i bez zastrzeżeń

Klucz nie trafia do zdarzeń, DTO, komunikatów błędów ani logów; nieudany zapis metadanych przy wymianie przywraca
poprzedni klucz; wynik testu jest odrzucany, gdy konto w międzyczasie zniknęło albo dostało inny klucz; wyłączone
konto zostaje wyłączone po teście i wymianie; zmiana dostawcy w kreatorze czyści klucz i adres; zestawy tekstów PL
i EN są identyczne.
