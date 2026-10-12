# Samodzielne testowanie — co pomijałem i jak to zamknąć

Zlecenie właściciela z 2026-10-12: „zrób tak, żebyś mógł sam więcej testować — na razie dużo rzeczy pomijasz”.
Ten dokument mówi, co dokładnie było pomijane, dlaczego, i co się zmienia. Stan wykonania jest w tabeli na końcu.

## 1. Co pomijałem — rozliczenie

| Co                                                                          | Skala                                                                                                                                                                           | Dlaczego pomijane                                                                    | Czy da się bez właściciela                                                                                                        |
| --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------- |
| Testy sprzętowe oznaczone „uruchamia maszyna użytkownika” (`#[ignore]`)     | ok. 40 testów w 20 pakietach: prawdziwe audio, Silero, słowo wywoławcze, model mówcy, embeddingi, schowek, Kosz, skróty, pulpit (Notatnik), rejestr, Edge, Menedżer poświadczeń | traktowałem je jak testy „dla kogoś innego”; ten laptop **jest** maszyną użytkownika | tak, prawie wszystkie                                                                                                             |
| Pełne testy i linter całego projektu przy każdej wersji                     | 945 programów testowych                                                                                                                                                         | czas i miejsce na dysku; puszczałem tylko pakiety, które zmieniałem                  | tak                                                                                                                               |
| Polecenia rdzenia w prawdziwej aplikacji                                    | 229 poleceń; test prawdziwej aplikacji ma 18 scenariuszy i nikt nie liczył, ile poleceń one ćwiczą                                                                              | brak pomiaru                                                                         | tak                                                                                                                               |
| Nowe funkcje w prawdziwej aplikacji                                         | każda nowa funkcja miała test w przeglądarce na atrapie, ale rzadko scenariusz na prawdziwym rdzeniu                                                                            | atrapa jest szybsza                                                                  | tak                                                                                                                               |
| Mowa przez prawdziwy głośnik i mikrofon, tłumienie echa, przerywanie głosem | cały tor akustyczny                                                                                                                                                             | „nie odtwarzam dźwięku” — słuszne w nocy, w dzień już nie                            | tak: komputer mówi nagraniem z głośnika, mikrofon słucha                                                                          |
| Jakość głosów                                                               | 10 barw Supertonic, Piper                                                                                                                                                       | „ocena słuchem należy do właściciela”                                                | częściowo: zrozumiałość (rozpoznanie własnej mowy), głośność, przesterowanie, przerwy — mierzalne; „czy ładny” — tylko właściciel |
| Funkcje, których wynik pisze model                                          | obowiązki, tłumacz reguł, streszczenia                                                                                                                                          | próba ręczna, raz                                                                    | tak: próba z automatyczną oceną „czy raport nie wychodzi poza dane”                                                               |
| Pobieranie przez program                                                    | Qwen3-ASR 2,5 GB, `llama-server`                                                                                                                                                | rozmiar                                                                              | tak — mieści się w stałej zgodzie do 3 GB                                                                                         |
| `cargo deny`                                                                | zależności i licencje                                                                                                                                                           | brak narzędzia lokalnie                                                              | tak, po doinstalowaniu narzędzia                                                                                                  |
| Kamera                                                                      | test sprzętowy                                                                                                                                                                  | kamera laptopa odłączona sprzętowo                                                   | **nie** — potrzebna Twoja ręka (zasłonka / klawisz)                                                                               |
| Karty AMD / Intel na żywo                                                   | odczyt sprzętu                                                                                                                                                                  | brak takiej karty                                                                    | **nie** — tylko test na zapisanych odpowiedziach narzędzi                                                                         |
| Testy „do decyzji człowieka” (Broker, P3-02, P3-04, W3-09, PT-25)           | 5 testów                                                                                                                                                                        | celowo czerwone do Twojej decyzji                                                    | **nie** — to decyzje, nie testy                                                                                                   |

Wniosek: z kilkunastu pozycji „nie sprawdziłem” z ostatnich raportów naprawdę wymagają Ciebie trzy grupy:
kamera, ocena „czy głos się podoba” i decyzje bezpieczeństwa. Resztę mogę i powinienem robić sam.

## 2. Co się zmienia

### 2.1 Trzy poziomy sprawdzenia — jeden skrypt, jeden raport

`node scripts/verify.mjs <poziom>` uruchamia etapy po kolei, pilnuje miejsca na dysku i zapisuje wynik do
`target/verify/<wersja>/raport.md` (oraz `raport.json`). Raport sam wypisuje, czego **nie** uruchomiono i z jakiego
powodu — lista „nie sprawdziłem” przestaje być pisana z pamięci.

| Poziom   | Kiedy                                                                                           | Etapy                                                                                                                                      |
| -------- | ----------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `szybki` | po każdej zmianie                                                                               | formatowanie, linter i testy zmienionych pakietów, testy interfejsu, kontrola typów                                                        |
| `wersja` | przed każdą wersją                                                                              | linter i testy **całego** projektu, testy w przeglądarce, budowa, test prawdziwej aplikacji, pokrycie poleceń rdzenia, kontrola zależności |
| `sprzet` | przed każdą budową dla właściciela (co 5 ulepszeń) i po zmianach w głosie, modelach, platformie | wszystkie testy sprzętowe, tor akustyczny, zrozumiałość głosów, próby funkcji modelowych, pobieranie przez program                         |

### 2.2 Testy sprzętowe — uruchamiane naprawdę

`scripts/verify-hw.sh` ustawia zmienne (`ALFA_SILERO_VAD`, `ALFA_KWS_MODEL`, `ALFA_SPEAKER_MODEL`,
`ALFA_EMBED_MODEL`, `ALFA_QWEN_*`, `ALFA_SMART_TURN_*`, `ALFA_LIVE_*`) na modele i silniki z `%LOCALAPPDATA%\Alfa`
(tylko do odczytu) i uruchamia testy `--ignored` pakiet po pakiecie. Podział:

- **bez śladu na pulpicie** (zawsze): audio, modele głosu, embeddingi, rejestr, procesy i usługi, wydajność Markdown;
- **dotykają pulpitu** (schowek, Kosz, skróty, Notatnik, Edge, Menedżer poświadczeń): uruchamiane, gdy właściciel
  nie pracuje przy komputerze albo na polecenie; test schowka przywraca jego zawartość, pozostałe sprzątają po sobie;
- **wyłączone z biegu**: testy „do decyzji człowieka” i testy wymagające ręcznej akcji (blokada ekranu, gra na
  pełnym ekranie, kamera) — raport wymienia je z powodem.

### 2.3 Pokrycie poleceń rdzenia w prawdziwej aplikacji

- Okno zapisuje nazwy poleceń, które wywołało; test prawdziwej aplikacji na końcu zrzuca tę listę, a
  `scripts/ipc-coverage.mjs` porównuje ją ze spisem poleceń i wypisuje nieprzećwiczone.
- Nowy scenariusz „przegląd poleceń”: każde polecenie odczytu bez argumentów jest wywoływane na prawdziwym rdzeniu
  i musi odpowiedzieć bez błędu w rozsądnym czasie.
- Reguła: nowa funkcja = scenariusz na prawdziwym rdzeniu, nie tylko na atrapie. Pokrycie nie może spaść między wersjami.

### 2.4 Tor akustyczny — komputer rozmawia sam ze sobą

`scripts/voice-loop.mjs` na uruchomionej aplikacji (profil tymczasowy): włącza mikrofon, odtwarza z głośnika nagrane
zdanie syntetycznym głosem i sprawdza, co program rozpoznał i czy odpowiedział. Trzy próby:

1. zdanie z głośnika → rozpoznany tekst zgodny z oczekiwanym (próg błędu słów);
2. agentka mówi → jej własny głos **nie** wraca jako wypowiedź użytkownika (tłumienie echa);
3. zdanie z głośnika w trakcie mowy agentki → mowa się zatrzymuje (przerywanie głosem).

Tylko w dzień (reguła „bez dźwięku w nocy” zostaje), na bieżącej głośności systemu — skrypt nie zmienia ustawień
dźwięku; gdy mikrofon nie słyszy głośnika, mówi to wprost zamiast zaliczać.

### 2.5 Zrozumiałość głosów

Każda barwa mówi pięć zdań kontrolnych do pliku (bez głośnika), rozpoznawanie zamienia je z powrotem na tekst.
Miary: odsetek błędnych słów, głośność, przesterowanie, długość ciszy na początku i końcu. To nie zastępuje ucha,
ale wyłapuje głos niezrozumiały, za cichy albo ucięty — bez właściciela.

### 2.6 Funkcje, których wynik pisze model

`scripts/trial.sh` (profil tymczasowy z kopią modelu) wchodzi do repozytorium razem ze scenariuszami obowiązków.
Ocena automatyczna: każda liczba, data i nazwa w raporcie agentki musi występować w arkuszu faktów; raport
z czymkolwiek spoza danych = próba czerwona.

## 3. Reguły pracy (obowiązują od 0.0.46)

1. Wersja nie jest „gotowa” bez poziomu `wersja`; budowa dla właściciela — bez poziomu `sprzet`.
2. „Nie sprawdziłem” wolno napisać tylko z jednym z trzech powodów: brak sprzętu, decyzja właściciela, ocena
   zmysłami właściciela. Każdy inny powód oznacza brakujący test, który trzeba dopisać.
3. Raport dla właściciela bierze liczby z `raport.json`, nie z pamięci.
4. Błąd znaleziony przez próbę na żywo dostaje test, który by go złapał (jak nazwy wyzwalaczy po restarcie w 0.0.45).

### 2.7 Instancja testowa obok uruchomionej Alfy

Dotąd każdy test prawdziwej aplikacji wymagał zamknięcia okna właściciela (jeden port, jedna instancja, ten sam plik
programu). Od 0.0.46:

- powłoka testowa przyjmuje własny port (`ALFA_E2E_CDP_PORT`) i tryb instancji testowej (`ALFA_E2E_INSTANCE=1`) —
  wyłącznie w budowie z cechą `e2e`; budowa produkcyjna nadal pilnuje jednej instancji i nie otwiera portu;
- Alfa właściciela działa z osobnej kopii programu (`Alfa-uruchomiona/`, kładzie ją `scripts/deploy-owner.sh`),
  więc budowa i testy jej nie dotykają;
- `scripts/verify-app.sh` buduje, uruchamia test prawdziwej aplikacji na profilu tymczasowym i liczy pokrycie
  poleceń; `scripts/trial/profile.sh` uruchamia instancję testową do prób (z kopią modelu albo bez).

## 4. Stan wykonania

| Element | Stan |
| --- | --- |
| Rozliczenie pominięć (ten dokument) | zrobione 2026-10-12 |
| Pełne testy i linter całego projektu dla 0.0.45 | zrobione: linter bez uwag; 946 programów testowych, 2676 testów zielonych, 0 czerwonych, 33 pominięte (sprzętowe) |
| `scripts/verify-hw.mjs` i pierwszy bieg testów sprzętowych | zrobione: 12 etapów „cichych” — 11 zielonych, 1 pominięty (brak wytrenowanego modelu słowa wywoławczego). Pierwszy bieg znalazł błąd w kwarantannie pobrań (katalog w skróconej postaci `SPOPIE~1` brany za dowiązanie) — naprawiony z testem |
| Instancja testowa obok Alfy właściciela | zrobione: test prawdziwej aplikacji przechodzi (19 z 19) przy uruchomionej Alfie właściciela |
| Pokrycie poleceń rdzenia + scenariusz „przegląd poleceń” | zrobione: 38 poleceń odczytu sprawdzanych ze schematem łącza; pierwszy pomiar 62 z 228 (27,2 %); po scenariuszach cykli życia **87 z 228 (38,2 %)** |
| Ocena raportów z obowiązków (dane + forma) i skrypt prób | zrobione: pierwszy bieg wykrył odgrywanie ról we wszystkich czterech raportach (przepuszczone w 0.0.45); po poprawce polecenia 3 z 4 czyste, czwarty odrzucony błędnie przez oceniającego — poprawione |
| Testy sprzętowe dotykające pulpitu (schowek, Kosz, skróty, Notatnik, Edge, poświadczenia) | do zrobienia — gdy właściciel nie pracuje przy komputerze |
| `scripts/verify.mjs` z jednym raportem | zrobione (poziomy szybki / wersja / sprzet) |
| Tor akustyczny | do zrobienia |
| Zrozumiałość głosów | do zrobienia |
| Pobranie Qwen3-ASR i `llama-server` przez program | do zrobienia |
| `cargo deny` lokalnie | do zrobienia |
| Pokrycie poleceń rdzenia ≥ 60 % | do zrobienia (scenariusze: konta, zadania, wyzwalacze, pamięć, pliki, sesje) |
