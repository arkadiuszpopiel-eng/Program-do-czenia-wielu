# 00 — Stan wykonania

Stan na **2026-10-09**, wersja programu **0.0.18**, gałąź `ccr-af4b63c6-3fyzaj` (commity lokalne, niewypchnięte).
Źródło prawdy o kryteriach: [`../STATUS.md`](../STATUS.md). Liczby o kodzie: [`../CATALOG.md`](../CATALOG.md).
Opis zmian po ludzku: [`../../CHANGELOG.md`](../../CHANGELOG.md).

## Jak czytać ten dokument

| Oznaczenie | Znaczenie                                                                                           |
| ---------- | --------------------------------------------------------------------------------------------------- |
| ✅ CI      | przechodzi w testach automatycznych, na atrapach — **nie** dowodzi działania na prawdziwej maszynie |
| ✅ sprzęt  | zmierzone albo sprawdzone w działającym programie na laptopie właściciela                           |
| 🟡         | zbudowane, czeka na pomiar na sprzęcie, korpus nagrań albo klucz                                    |
| ⛔         | brak implementacji albo brak zestawu testowego                                                      |

## Skrót

| Miara                               | Wartość                                                                                | Źródło                        |
| ----------------------------------- | -------------------------------------------------------------------------------------- | ----------------------------- |
| Kod Rust                            | 241 crate'ów, 107 modułów, ok. 373 tys. linii, 2514 testów                             | CATALOG                       |
| Interfejs (Svelte 5 + TS)           | ok. 46 tys. linii; vitest 257/257; E2E 138 zielonych, 1 czerwony (znany, próg czasowy) | przebieg 2026-10-09           |
| Prawdziwa aplikacja na laptopie     | uruchamia się; testy dymne `e2e-app` 11/11 na profilu tymczasowym (wersja 0.0.2)       | przebieg 2026-10-09           |
| Kryteria akceptacji F0–F9           | 108: 38 ✅ CI · 57 🟡 · 11 ⛔ · 2 do decyzji                                           | STATUS (macierz z 2026-10-06) |
| Kryteria potwierdzone na sprzęcie   | **0 z 108**                                                                            | STATUS                        |
| Model lokalny odpowiada na laptopie | **nie sprawdzone** — brak silnika; czeka na zgodę na pobranie ok. 6 GB                 | —                             |
| Głos na żywo                        | **nie sprawdzone** — jw.                                                               | —                             |

Najważniejsze zdanie tego dokumentu: **kod wszystkich fal F0–F9 istnieje i przechodzi testy automatyczne, ale
żadna fala nie jest zamknięta na prawdziwej maszynie.** Dlatego etap A roadmapy ma pierwszeństwo przed nowymi
funkcjami.

## Wersje

| Wersja | Data       | Co zawiera                                                                                                                                                                                                                                            | Jak sprawdzone                                                                                                                                                                                                                                          |
| ------ | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0.0.1  | 2026-10-07 | pierwszy stan uruchamialny: kod fal F0–F9                                                                                                                                                                                                             | CI; pierwszy test na laptopie znalazł błędy (brak silnika, skalowanie)                                                                                                                                                                                  |
| 0.0.2  | 2026-10-09 | stabilizacja: skalowanie interfejsu, naprawy z przeglądów rdzenia, narzędzi i stanu interfejsu, porządek w repozytorium                                                                                                                               | testy jednostkowe i regresji dla każdej naprawy; E2E; `e2e-app` 11/11 na prawdziwej aplikacji                                                                                                                                                           |
| 0.0.3  | 2026-10-09 | style „Szkło” i „Next” (jasny i ciemny); plan v6 w MD i HTML                                                                                                                                                                                          | vitest (`skin.test.ts`), E2E z kontrolą dostępności w czterech kombinacjach (`skins.spec.ts`), zrzuty wszystkich widoków                                                                                                                                |
| 0.0.4  | 2026-10-09 | własne listy wyboru i pozostałe kontrolki w stylu Alfy (zgłoszenie właściciela: nieczytelna lista systemowa w stylu „Szkło”)                                                                                                                          | E2E: lista otwarta w 6 kombinacjach stylu i motywu; test w prawdziwej aplikacji; zrzuty stanu otwartego                                                                                                                                                 |
| 0.0.5  | 2026-10-09 | okno „Co nowego” pokazuje skrót poprawek z dziennika zmian                                                                                                                                                                                            | testy `app-updates` (skrót wpisu; wpis istnieje dla uruchamianej wersji)                                                                                                                                                                                |
| 0.0.6  | 2026-10-09 | naprawy po przeglądzie zmian 0.0.1–0.0.5: dekodowanie wyjścia powłoki, kolejka offline, fokus w rozmowie, lista wyboru, „Co nowego”, spójność numeru wersji, świeżość HTML                                                                            | testy regresji dla każdej poprawki; pełny zestaw testów interfejsu; testy crate'ów; test prawdziwej aplikacji                                                                                                                                           |
| 0.0.7  | 2026-10-09 | dodawanie klucza API od nowa: diagnoza przyczyny odmowy, sprzątanie wklejonego klucza, wymiana klucza bez utraty działającego, wyłączanie konta, adresy API czterech dostawców w katalogu                                                             | Rust: 6 testów całej ścieżki z lokalnym serwerem dostawcy, klasyfikacja odpowiedzi, wspólna tabela kluczy; E2E: 24 testy kreatora (6 kombinacji stylu i motywu, EN); zrzuty wszystkich stanów. Nie sprawdzono z prawdziwym kluczem — robi to właściciel |
| 0.0.8  | 2026-10-09 | wybór modelu rozmowy z listy dostawcy, strona „API i kolejki modeli” (do 10 modeli na klasę zadań), redukcja szumów mikrofonu w trzech poziomach, wykrywanie końca wypowiedzi w rozmowie głosowej, filtr zmyślonych wywołań narzędzi modelu lokalnego | Rust: ścieżka z lokalnym serwerem dostawcy, reguły kolejek, szum wentylatora, zegar potoku na prawdziwym mikrofonie; E2E: 15 testów strony API i mikrofonu; zrzuty. Rozmowa głosowa od końca do końca na laptopie — do potwierdzenia przez właściciela  |

## Sprawdzone na laptopie (i7-13620H, RTX 4050 6 GB, Windows 11)

Tylko to, co naprawdę uruchomiono w zbudowanym programie:

| Co                                                        | Wynik                                      | Dowód                                       |
| --------------------------------------------------------- | ------------------------------------------ | ------------------------------------------- |
| Start aplikacji, moduły, wykrycie sprzętu (`laptop-cuda`) | działa                                     | logi z 2026-10-07                           |
| Okno główne, ustawienia, panele, kreator, paleta          | działa                                     | `apps/desktop/ui/e2e-app` 11/11             |
| Skala interfejsu powyżej 100 % (pole pisania w oknie)     | naprawione i sprawdzone                    | test w prawdziwej aplikacji (`Ctrl+=`)      |
| Ostrzeżenie o zajętym skrócie globalnym                   | widoczne                                   | STATUS, dopisek 2026-10-08                  |
| Komunikat „model bez silnika” z przyciskiem               | kod i testy; w aplikacji **nie obejrzany** | —                                           |
| Kodowanie polskich znaków z PowerShell i `cmd`            | sprawdzone na prawdziwych procesach        | `platform-windows-impl/tests/shell_utf8.rs` |
| „O programie”: wersja, data i commit kompilacji           | działa (0.0.2)                             | zrzut z 2026-10-09                          |

Próba generalna na serwerze CI z Windows (procesor 4 rdzenie, bez karty) zmierzyła: Bielik 4.5B — 6,8 tok/s,
Bielik 1.5B — 21 tok/s, Piper — współczynnik czasu rzeczywistego 0,34. To pomiary na innej maszynie, nie na
laptopie właściciela.

## Fale planu v5

| Fala                          | Zakres                                                                        |  ✅ CI |     🟡 |     ⛔ | Do decyzji | Co blokuje zamknięcie                                          |
| ----------------------------- | ----------------------------------------------------------------------------- | -----: | -----: | -----: | ---------: | -------------------------------------------------------------- |
| F0 Fundament                  | repo, CI, kontrakty rdzenia, ADR-y, spike'i                                   |      4 |     13 |      0 |          2 | pomiary na sprzęcie; akceptacja makiet i ADR-ów                |
| F1 Czat                       | dostawcy, model lokalny, sesje, router, ustawienia, zasobnik, Szybkie pytanie |      7 |      6 |      0 |          0 | model lokalny na żywo; budżety RAM i czasu                     |
| F2 Głos i agentki             | potok głosu, przerywanie, cztery persony                                      |      2 |     12 |      0 |          0 | korpus nagrań, odsłuchy, sprzęt                                |
| F3 Jądro bezpieczeństwa       | Broker, okno zgód, watchdog, „Cofnij”, kill-switch, narzędzia                 |      6 |      5 |      2 |          0 | zestaw ataków, scenariusz MVP, Jądro poza procesem na maszynie |
| F4 Mosty i MCP                | mosty CLI, terminal, MCP                                                      |      3 |      5 |      0 |          0 | logowanie do CLI (właściciel)                                  |
| F5 Agentki i głos rozszerzony | agentki równoległe, harmonogram, słowa wywoławcze, dyktowanie                 |      5 |      7 |      0 |          0 | korpus i sprzęt                                                |
| F6 Sterowanie komputerem      | UIA, ekran, wejście, przeglądarka, Office                                     |      1 |      3 |      4 |          0 | decyzja P2-01, pomocnik `uiAccess`, zestaw zadań               |
| F7 Pamięć i transfer          | cztery warstwy pamięci, inspektor, kopie                                      |      5 |      2 |      1 |          0 | recall na prawdziwym embedderze, migracje                      |
| F8 Samonaprawa                | diagnosta, ulepszacz, wtyczki Wasm                                            |      5 |      0 |      1 |          0 | zamrożony zestaw złośliwych wtyczek                            |
| F9 Dopieszczenie              | dostępność, wydajność, pentest                                                |      0 |      4 |      3 |          0 | pentest, audyt                                                 |
| **Razem**                     |                                                                               | **38** | **57** | **11** |      **2** |                                                                |

## Wykonane i przetestowane od macierzy z 2026-10-06

Pozycje poniżej są w kodzie z testami regresji (✅ CI); te z dopiskiem „sprzęt” sprawdzono też w prawdziwej
aplikacji. Szczegóły: STATUS, dopisek 2026-10-08/09.

| Obszar         | Co                                                                                                                      | Testy                                                                             |
| -------------- | ----------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Interfejs      | skalowanie i układ 1280–3840 px; dopasowanie skali do okna; siedem grup ustawień; prawy panel obok rozmowy na laptopie  | E2E `responsive.spec.ts`; **sprzęt**                                              |
| Interfejs      | rozmowa otwiera się na ostatniej wiadomości (trzymanie dołu listy)                                                      | E2E; test jednostkowy listy wirtualnej                                            |
| Interfejs      | dziewięć napraw logiki stanu (usunięta sesja, szkic, błąd jednego zdarzenia, kolejność pamięci podręcznej…)             | `state/__tests__/review.test.ts`                                                  |
| Interfejs      | style Klasyczny / Szkło / Next w wariancie jasnym i ciemnym                                                             | `skin.test.ts`, `skins.spec.ts` (axe)                                             |
| Rozmowa        | model bez silnika → czytelny błąd i przycisk; edycja z załącznikami; kolejka offline a kosz                             | Rust + E2E                                                                        |
| Głos           | „Przeczytaj na głos” (dostęp do schowka); komunikaty o braku silnika; STOP kończy dyktowanie                            | `app-core/tests/review_voice.rs`                                                  |
| Narzędzia      | odczyt zakresowy pliku; limit zapisu; nazwy urządzeń Windows; kodowanie wyjścia powłok; kodowanie stron; przekierowania | testy crate'ów `tools-*`, `platform-windows-impl`; część na prawdziwych procesach |
| Cofanie        | usuwanie utworzonych katalogów po restarcie; powtarzalne „Cofnij” schowka                                               | `undo-journal`, `tools-clipboard`                                                 |
| Modele i kopie | zwalnianie pamięci po nieudanym starcie; limit czasu ładowania; kopia nie powstaje bez szyfrowania                      | `app-core/tests/review.rs`                                                        |
| Porządek       | katalog modułów, README w każdym crate'cie, żaden plik Rust ponad 400 linii, bramki jednym poleceniem                   | `scripts/catalog.mjs`, `scripts/gates.ps1`                                        |
| Wersjonowanie  | dziennik zmian; „O programie” pokazuje datę i commit                                                                    | **sprzęt**                                                                        |

## Czego nie ma w kodzie (z planu v5)

Narzędzie Voice Lab w interfejsie, transkrypcja spotkań, adapter chmurowy rozmowy głos–głos, pomocnik
`uiAccess`, kamera i mikrofon w narzędziach mediów, dostawca wyszukiwania w sieci, własny pasek tytułu z Mica,
generator typów TS (ADR 0013), eksport rozmowy do PDF.

## Z planu v6

Żadna funkcja planu v6 nie jest jeszcze zbudowana. Gotowe są: dokumenty tego katalogu, projekty ADR 0017–0024,
szkice SPEC dziewięciu modułów, przegląd technologii. Stan zadań: [`03-roadmapa.md`](03-roadmapa.md).

## Znane problemy

| Problem                                                                         | Waga    | Stan                               |
| ------------------------------------------------------------------------------- | ------- | ---------------------------------- |
| Model lokalny i głos niesprawdzone na żywo                                      | wysoka  | czeka na zgodę na pobranie pakietu |
| Jądro poza procesem niesprawdzone na żadnej maszynie                            | wysoka  | etap A.6                           |
| Analizator poleceń powłoki nie widzi poleceń w cudzysłowie (S-01, S-03)         | wysoka  | decyzja właściciela                |
| Zmiany Jądra bez przeglądu człowieka                                            | wysoka  | etap A.8                           |
| Build wydaniowy ma `panic = "abort"` — strażnik paniki generacji tam nie działa | średnia | decyzja właściciela                |
| Test E2E „strumień 100 tok/s” na granicy progu; test czasowy palety niestabilny | niska   | decyzja o progu                    |
| Ponad 70 commitów lokalnych niewypchniętych                                     | —       | `gh auth login` (właściciel)       |
