# 10 — Zgodność prawna (UE / Polska)

**To nie jest porada prawna.** Zestawienie powstało 2026-10-09 głównie ze źródeł wtórnych (opracowania kancelarii
i firm doradczych); daty i zakres trzeba potwierdzić w tekstach aktów (EUR-Lex) i z prawnikiem przed pierwszym
wydaniem publicznym albo komercyjnym. Dopóki Alfa jest programem jednego właściciela na własny użytek, większość
obowiązków jej nie dotyczy — ale projektujemy tak, żeby wydanie nie wymagało przebudowy.

## Terminy

| Data           | Co                                                                                            | Akt                          | Dotyczy Alfy?                                      |
| -------------- | --------------------------------------------------------------------------------------------- | ---------------------------- | -------------------------------------------------- |
| 2025-02        | Zakazane praktyki; obowiązek „kompetencji AI”                                                 | AI Act                       | pośrednio — nie budujemy zakazanych funkcji        |
| 2025-06-28     | Wymogi dostępności dla wybranych produktów i usług                                            | Europejski Akt o Dostępności | niepewne — patrz niżej                             |
| 2025-08-02     | Obowiązki dostawców modeli ogólnego przeznaczenia                                             | AI Act                       | prawdopodobnie nie — Alfa nie jest dostawcą modelu |
| **2026-08-02** | Przejrzystość: użytkownik ma wiedzieć, że rozmawia z AI                                       | AI Act art. 50               | **tak** — przy wydaniu                             |
| **2026-09-11** | Zgłaszanie aktywnie wykorzystywanych podatności i poważnych incydentów (24 h / 72 h / 14 dni) | CRA art. 14                  | **tak** — przy wprowadzeniu na rynek               |
| **2026-12-02** | Maszynowe oznaczanie treści generowanych (dla systemów sprzed 2026-08-02 — po przesunięciu)   | AI Act art. 50 ust. 2        | **tak** — przy wydaniu                             |
| 2027-12-02     | Systemy wysokiego ryzyka (załącznik III) — po przesunięciu                                    | AI Act                       | nie — Alfa nie jest systemem wysokiego ryzyka      |
| **2027-12-11** | Pełne wymagania: bezpieczeństwo w projekcie, SBOM, okres wsparcia, oznakowanie CE             | CRA                          | **tak** — przy wprowadzeniu na rynek               |

## Zadania w planie

| Zadanie                                                                                                                  | Etap  | Stan                                      |
| ------------------------------------------------------------------------------------------------------------------------ | ----- | ----------------------------------------- |
| Informacja „rozmawiasz z AI” przy pierwszym uruchomieniu i stale w interfejsie                                           | C2.9  | zrobione (seria 1, punkt 3) |
| Etykieta „wygenerowane przez AI” w eksporcie rozmów (MD, HTML), plikach dźwiękowych i obrazach — czytelna i w metadanych | C2.9  | rozmowy: zrobione (seria 1, punkt 3); dźwięku ani obrazów program jeszcze nie zapisuje do plików |
| Karty modeli w programie: pochodzenie, licencja, przeznaczenie                                                           | C2.9  | zrobione (seria 1, punkt 4): katalog lokalny, `app-models/data/model-cards.json` |
| Karty licencji silników i głosów: licencja, autor, użycie komercyjne, wskazanie autora, sprawy otwarte                   | C5.7  | zrobione (seria 5, 0.0.64): `app-models/data/license-cards.json`; znak wodny mowy — decyzja właściciela |
| SBOM dla każdego wydania (Rust + npm)                                                                                    | C2.10 | zrobione (seria 2, punkt 1): `scripts/sbom.mjs`, etap `sbom` w bramce wersji |
| Polityka zgłaszania podatności, kontakt bezpieczeństwa, plik `SECURITY.md`                                               | C2.10 | w toku: `SECURITY.md` gotowy, kontakt czeka na właściciela |
| Procedura zgłoszeń z art. 14 CRA (kto, w jakim czasie, jakim kanałem)                                                    | C2.10 | planowane                                 |
| Deklarowany okres wsparcia i podpisane aktualizacje                                                                      | C2.10 | podpis minisign już jest; brak deklaracji |
| Brak telemetrii domyślnie; diagnostyka tylko po włączeniu                                                                | —     | obowiązuje od początku projektu           |
| Przekaźnik „ślepy na treść” + opis przetwarzania danych                                                                  | D     | czeka                                     |
| Dostępność: WCAG 2.2 AA / EN 301 549                                                                                     | C1.5  | testy axe w CI są; pełny audyt planowany  |

## Co wynika z nowych platform

| Zmiana w planie v6             | Skutek prawny                                                              | Co robimy                                                                                       |
| ------------------------------ | -------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| Przekaźnik w Internecie        | kto go prowadzi, ten przetwarza dane (adresy IP, metadane połączeń) — RODO | przekaźnik nie widzi treści; minimalne logi; możliwość postawienia własnego; opis przetwarzania |
| Powiadomienia push             | dane przechodzą przez usługę producenta systemu                            | powiadomienie bez treści — tylko sygnał „pobierz”                                               |
| Klucz API na telefonie         | odpowiedzialność za bezpieczeństwo klucza                                  | magazyn sprzętowy, zgoda, kasowanie zdalne (ADR 0023)                                           |
| Aplikacja w sklepie            | regulaminy sklepów, deklaracje o danych                                    | decyzja właściciela (F.8)                                                                       |
| Nagrywanie głosu osób trzecich | zgoda rozmówców to obowiązek użytkownika                                   | wyraźny wskaźnik nagrywania; opis w przewodniku                                                 |

## Kamery i biometria (grupa C3)

| Temat                                                                                            | Co z niego wynika dla Alfy                                                                                     |
| ------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------- |
| Wzorzec twarzy = dane biometryczne (RODO, art. 9)                                                | wyraźna zgoda każdej zapisywanej osoby, dane tylko lokalnie i zaszyfrowane, łatwe usunięcie (projekt ADR 0025) |
| Użytek domowy a inne osoby w kadrze                                                              | kamery domyślnie wyłączone, brak nagrywania, brak wzorców osób niezapisanych                                   |
| AI Act: zakaz rozpoznawania emocji w pracy i szkole, zakaz baz twarzy z niecelowanego pobierania | Alfa nie ocenia emocji i nie buduje wzorców bez zapisu — do potwierdzenia przez prawnika                       |
| Obraz wysyłany do dostawcy w chmurze                                                             | osobna zgoda per kamera, nigdy w sesji prywatnej, widoczny odbiorca                                            |

## Pytania do prawnika

1. Czy Alfa — asystent na komputer, rozprowadzany bezpłatnie albo odpłatnie — podlega Europejskiemu Aktowi
   o Dostępności? Dyrektywa wymienia konkretne kategorie; ogólny asystent prawdopodobnie do nich nie należy,
   chyba że jest sprzedawany konsumentom przez sklep internetowy.
2. Czy rozprowadzanie cudzych modeli o otwartych wagach bez zmian czyni z Alfy „dostawcę” w rozumieniu AI Act?
   A po dostrojeniu modelu?
3. Jak art. 50 ust. 2 stosuje się do tekstu i mowy generowanych lokalnie, na komputerze użytkownika?
4. Licencja GPL-3.0 silnika mowy uruchamianego jako osobny proces — skutki dla licencji Alfy (ADR 0024).
5. Licencje modeli (część wariantów PLLuM jest niekomercyjna) i głosów (każdy głos Pipera ma własną).
6. Status prowadzącego przekaźnik: administrator czy podmiot przetwarzający; czy potrzebna ocena skutków.

- Kamery (C3): czy rozpoznawanie zapisanych domowników i gości mieści się w wyjątku użytku domowego; jakie
  obowiązki dochodzą, gdy program jest używany w firmie; czy rozpoznawanie rozmówcy podlega przepisom AI Act
  o identyfikacji biometrycznej.

## Zasada na co dzień

Niezależnie od rozstrzygnięć: budujemy tak, jakby obowiązki nas dotyczyły tam, gdzie kosztuje to mało
(oznaczenia, SBOM, dostępność, brak telemetrii), i nie obiecujemy zgodności, której nikt nie potwierdził.
