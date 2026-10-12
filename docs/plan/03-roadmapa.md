# 03 — Roadmapa (dokument żywy)

Ostatnia aktualizacja: **2026-10-12** (wersja programu 0.0.43; raport z nocy: [`23-raport-nocny-2026-10-12.md`](23-raport-nocny-2026-10-12.md)). Obraz całości: [`16-plan-zbiorczy.md`](16-plan-zbiorczy.md). Statusy: patrz [`README.md`](README.md).
Stan kryteriów akceptacji żyje w [`../STATUS.md`](../STATUS.md); tu są zadania i etapy.

Zasada kolejności: **etap A ma pierwszeństwo**. Etap B można prowadzić równolegle z A, bo nie dodaje funkcji.
Etapy C–G startują po zamknięciu MVP (A.8) — z wyjątkiem zadań oznaczonych „można wcześniej”. Technicznie etap G
(Linux) wymaga tylko etapu B; o tym, czy ruszy przed D–F, decyduje właściciel.

```
A (MVP Windows) ══▶ P (funkcje Delty) ══▶ C (funkcje Windows)
   │                    └▶ U (wielu użytkowników) ─┐
   └▶ B (szwy) ─▶ D (host i protokół) ◀────────────┘ ─▶ E (panel WWW) ─▶ F (Android)
        └───────────────────────────────▶ G (Linux)               H (badawcze — bez terminu)
```

## Etap A — Domknięcie Windows v1 (MVP)

Szczegółowe zadania: [`../KONTYNUACJA.md`](../KONTYNUACJA.md) §6, etapy 0–3. Tutaj tylko stan zbiorczy.

| ID   | Zadanie                                                                                                                                                                                       | Stan     | Data       | Uwagi                                                                                                                                                                                    |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A.0  | Porządek w środowisku i repozytorium (KONTYNUACJA etap 0)                                                                                                                                     | w toku   | —          | zostało: `gh auth login` (właściciel), przegląd gałęzi Dependabota, pełne `gates.ps1 -Full`                                                                                              |
| A.1  | Skalowanie i układ interfejsu 1280–3840 px                                                                                                                                                    | zrobione | 2026-10-08 | wersja 0.0.2; testy e2e i test w prawdziwej aplikacji                                                                                                                                    |
| A.2  | Naprawy z przeglądów rdzenia, narzędzi i stanu interfejsu                                                                                                                                     | zrobione | 2026-10-09 | wersja 0.0.2; lista w `CHANGELOG.md`                                                                                                                                                     |
| A.3  | Style interfejsu „Szkło” i „Next” (jasny i ciemny)                                                                                                                                            | zrobione | 2026-10-09 | wersja 0.0.3; `e2e/skins.spec.ts`                                                                                                                                                        |
| A.3a | Własne listy wyboru i pozostałe kontrolki w stylu Alfy (bez kontrolek systemowych)                                                                                                            | zrobione | 2026-10-09 | wersja 0.0.4; błąd zgłoszony przez właściciela; testy listy otwartej w 6 kombinacjach + prawdziwa aplikacja                                                                              |
| A.3b | Okno „Co nowego” ze skrótem poprawek z dziennika zmian                                                                                                                                        | zrobione | 2026-10-09 | wersja 0.0.5; zgłoszenie właściciela; test pilnuje wpisu dla każdej wersji                                                                                                               |
| A.3c | Przegląd zmian 0.0.1–0.0.5 w świeżym kontekście i naprawa ustaleń                                                                                                                             | zrobione | 2026-10-09 | wersja 0.0.6; 3 przeglądy (interfejs, Rust, skrypty i testy), 0 krytycznych; raport: `docs/reviews/2026-10-09-przeglad-0.0.1-0.0.5.md`                                                   |
| A.3d | Dodawanie klucza API: diagnoza przyczyny, sprzątanie wklejonego klucza, wymiana i wyłączanie konta                                                                                            | zrobione | 2026-10-09 | wersja 0.0.7; zgłoszenie właściciela („401 Klucz odrzucony”); 3 przeglądy w świeżym kontekście; bez próby z prawdziwym kluczem; raport: `docs/reviews/2026-10-09-przeglad-kluczy-api.md` |
| A.3e | Naprawy po pierwszej próbie 0.0.7 na laptopie: wybór modelu rozmowy, strona „API i kolejki modeli”, redukcja szumów, koniec wypowiedzi w rozmowie głosowej, filtr zmyślonych wywołań narzędzi | zrobione | 2026-10-09 | wersja 0.0.8; pięć zgłoszeń właściciela; zegar potoku sprawdzony na mikrofonie laptopa                                                                                                   |
| A.3f | Porządkowanie interfejsu, krok 1: przejścia konto ↔ kolejki modeli, przełącznik szczegółów odpowiedzi                                                                                         | zrobione | 2026-10-09 | wersja 0.0.9; dalsze kroki wymagają wskazań właściciela (lista propozycji w raporcie sesji)                                                                                              |
| A.3g | Własne menu kontekstowe i podpowiedzi (bez elementów systemowych), kolejka modeli jako kafelki do przenoszenia, nowe nazwy stron ustawień modeli, osobna strona „Mosty CLI”                   | zrobione | 2026-10-09 | wersja 0.0.10; zgłoszenia właściciela                                                                                                                                                    |
| A.3h | Własna belka okna z paskiem LED i przyciskami okna, nawigacja Ustawień od nowa (ikony, zwijane grupy, szukanie z licznikami, klawiatura, wąskie okno)                                         | zrobione | 2026-10-09 | wersja 0.0.11; zgłoszenia właściciela; bez Snap Layouts po najechaniu                                                                                                                    |
| A.3i | Kilka kart graficznych: role kart, przypinanie modelu do karty głównej, lekki dostawca lokalny na karcie zintegrowanej (klasa „streszczenia”)                                                 | zrobione | 2026-10-09 | wersja 0.0.11 (kod i testy); praca na karcie zintegrowanej niesprawdzona na sprzęcie — wymaga silnika Vulkan (zgoda właściciela na pobranie)                                             |
| A.3j | Belka: obsada agentek, mikrofon i stan (profil, autonomia, koszt) od nowa, objaśnienie stanu                                                                                                  | zrobione | 2026-10-09 | wersja 0.0.12; zgłoszenie właściciela                                                                                                                                                    |
| A.3k | Znak Alfy: ikona programu i znak z nazwą na belce okna                                                                                                                                        | zrobione | 2026-10-09 | wersja 0.0.13; zgłoszenie właściciela                                                                                                                                                    |
| A.3l | Belka z torem paska LED, ekran ładowania, żywe awatary agentek z ustami w rytm mowy (zdarzenie `SpeechLevel`), strojenie mowy w „Głos” (styl, tempo, pauzy, różnice głosów)                   | zrobione | 2026-10-09 | wersja 0.0.14; brzmienie do oceny właściciela; inne głosy niż Piper „Gosia” — osobna decyzja                                                                                             |
| A.4  | Pierwsza rozmowa z modelem lokalnym na laptopie (KONTYNUACJA etap 1)                                                                                                                          | czeka    | —          | zgoda właściciela na pobranie pakietu ok. 6 GB                                                                                                                                           |
| A.5  | Głos na żywo: dyktowanie i czytanie                                                                                                                                                           | czeka    | —          | po A.4 (te same silniki w pakiecie)                                                                                                                                                      |
| A.6  | Pomiary MVP na sprzęcie, F0–F3 (KONTYNUACJA etap 2)                                                                                                                                           | czeka    | —          | po A.4                                                                                                                                                                                   |
| A.7  | Decyzje bezpieczeństwa: B-01…B-08, S-01…S-04, S-06, C-02, C-03, R-05, P-…                                                                                                                     | czeka    | —          | właściciel; pakiet gotowy: [`reviews/2026-10-10-pakiet-decyzji-wlasciciela.md`](../reviews/2026-10-10-pakiet-decyzji-wlasciciela.md) (44 decyzje z rekomendacjami)                       |
| A.8  | Wdrożenie decyzji i przegląd Jądra, wejście na `main` (KONTYNUACJA etap 3)                                                                                                                    | czeka    | —          | po A.7                                                                                                                                                                                   |

**Wyjście:** scenariusz MVP 6/6 na laptopie, każde kryterium F0–F3 z wynikiem z maszyny, `main` zawiera
zweryfikowany kod. To jest „Alfa 0.1”.

## Etap B — Szwy modularności

Bez nowych funkcji dla użytkownika; każde zadanie kończy się tym, że wszystkie dotychczasowe testy są zielone.

| ID  | Zadanie                                                                        | Stan         | Data | Uwagi                                                                                      |
| --- | ------------------------------------------------------------------------------ | ------------ | ---- | ------------------------------------------------------------------------------------------ |
| B.1 | Generator typów TS z Rust (ADR 0013) + kontrola w CI                           | w toku       | 2026-10-11 | etap 1 w 0.0.21: `app-api/src/schema.rs` (schemars, widok serializacji) → `packages/schemas/ipc.v1.json` → `scripts/gen-ipc-types.mjs` → `ui/src/lib/api/generated/ipc.ts` + `compat.ts` (501 sprawdzeń typów ręcznych, typowane `call` w `tauri-client.ts`); bramka i CI. Bez nowej zależności (zamiast `tauri-specta`/`ts-rs` — `schemars`, który już jest; do odnotowania w ADR 0013). **Zostaje etap 2:** usunąć ręczne `types*.ts` (19 typów interfejs zawęża z napisu do listy wartości — decyzja: wyliczenia w Rust albo zawężanie po stronie okna) |
| B.2 | Numery sekwencji zdarzeń i wznowienie „od `seq`”                               | zrobione     | 2026-10-11 | wersja 0.0.19; `app-api/src/event_log.rs` (numer przy wysłaniu, ramka `EventFrame` z `prev`/`seq`/`epoch`, bufor 1024 ramek / 8192 zdarzeń), polecenie `events_resume`, `ui/src/lib/api/event-sequencer.ts`; pole `offset` w `TextDelta` zamyka ustalenie o deltach w czasie wczytywania rozmowy; rozmiar bufora to wartość robocza — próg ustali właściciel po pomiarach (ADR 0019) |
| B.3 | Błędy strukturalne na łączu (`code`, `message`, `details`)                     | zrobione     | 2026-10-11 | wersja 0.0.19; `app-api/src/details.rs` (filtr sekretów), powłoka odsyła obiekt `AppError` zamiast napisu, `CommandError.details` w interfejsie; szczegóły wypełniają na razie błędy sesji, kont, ustawień, pamięci, kamer i modułów niepodłączonych — kolejne miejsca przy okazji zmian |
| B.4 | Fabryka platform `platform-select` i mapa zdolności hosta                      | czeka        | —    | ADR 0018                                                                                   |
| B.5 | Oznaczenie poleceń `local_only` i odpowiedniki dla klientów zdalnych           | do zrobienia | —    | okna dialogowe, podgląd plików, ścieżki                                                    |
| B.6 | Katalog silników i modeli z wymiarem system / architektura                     | do zrobienia | —    | `app-models/src/data.rs`                                                                   |
| B.7 | Kontrakty Brokera niezależne od systemu (ścieżki, powłoki, tożsamość)          | czeka        | —    | ADR 0018 + przegląd właściciela; po A.7                                                    |
| B.8 | Zmienne środowiskowe i ścieżki w binarkach Jądra przez `AppPaths`              | czeka        | —    | obszar Jądra — przegląd właściciela                                                        |
| B.9 | Drobne: `app-bridges/src/app.rs:227` (powłoka dla systemów innych niż Windows) | do zrobienia | —    |                                                                                            |

**Wyjście:** `cargo test --workspace` na Windows i Ubuntu zielone; korzeń kompozycji nie zawiera nazw
`platform_windows_*`; typy TS generowane.

## Etap C — Funkcje Windows po MVP

### C1 — z planu v5 (KONTYNUACJA etapy 4–8)

| ID   | Zadanie                                                   | Stan      | Data | Uwagi                           |
| ---- | --------------------------------------------------------- | --------- | ---- | ------------------------------- |
| C1.1 | Jakość głosu (WER PL, przerwania, cztery głosy)           | czeka     | —    | korpus nagrań od właściciela; cztery głosy — grupa C5 |
| C1.1a | Głos agentek z chmury jako opcja (adapter mowy przez konto API, zapas lokalny, nigdy w sesji prywatnej) | czeka | — | po grupie C5 — właściciel wybrał 2026-10-11 głos lokalny (koszt chmury); ADR 0026 |
| C1.2 | Mosty CLI, klucze i dostawcy chmurowi na żywo             | czeka     | —    | logowanie i klucze — właściciel |
| C1.3 | Sterowanie komputerem (UIA, ekran, wejście)               | czeka     | —    | po decyzji P2-01                |
| C1.4 | Pamięć i transfer: recall@5, migracje, round-trip `.alfa` | czeka     | —    | recall możliwy zaraz po A.4     |
| C1.5 | Samonaprawa, wtyczki, pentest, audyt WCAG                 | planowane | —    |                                 |

### C2 — nowe (opis: [`04-nowe-funkcje.md`](04-nowe-funkcje.md))

| ID     | Zadanie                                                                                                            | Stan      | Data       | Uwagi                                                                                                                   |
| ------ | ------------------------------------------------------------------------------------------------------------------ | --------- | ---------- | ----------------------------------------------------------------------------------------------------------------------- |
| C2.1   | Zestaw ewaluacyjny po polsku w CI (poprawność wywołań narzędzi, wierność RAG, ataki, WER, czas, VRAM)              | w toku    | 2026-10-11 | 0.0.22: szkic `evals/V6/pl-core` (36 przypadków: narzędzia, wierność wobec źródła, wstrzyknięcia; status `proposed`, test integralności w `evals-impl/tests/repo_suites.rs`). **Zostaje:** akceptacja i progi właściciela, wariant runnera uruchamiający model z narzędziami, pomiary sprzętowe (WER, czas, VRAM) |
| C2.2   | Modele na 6 GB: Bielik-Minitron-7B v3, Qwen3.5-4B; tryb wielu modeli w `llama-server`                              | planowane | —          | po A.4; pomiar przed wpisaniem do pakietów                                                                              |
| C2.3   | Wymuszanie poprawnego JSON wywołań narzędzi schematem (gramatyka)                                                  | planowane | —          |                                                                                                                         |
| C2.4   | Bielik-Guard jako dodatkowy klasyfikator w Brokerze                                                                | czeka     | —          | obszar Jądra — decyzja właściciela                                                                                      |
| C2.5   | MCP: obsługa specyfikacji 2026-07-28 obok dotychczasowej                                                           | planowane | —          | SDK `rmcp`; każda akcja nadal przez Brokera                                                                             |
| C2.6   | Umiejętności w formacie Agent Skills (`SKILL.md`) z przypięciem sumy i przeglądem                                  | planowane | —          |                                                                                                                         |
| C2.7   | Zadania zaplanowane z budżetem uprawnień („gdy komputer włączony i bezczynny”)                                     | planowane | —          | moduły `scheduler`, `triggers` już są                                                                                   |
| C2.8   | Kanwa artefaktów (Markdown, HTML, SVG w izolowanej ramce)                                                          | planowane | —          | ADR 0009 już to dopuszcza                                                                                               |
| C2.9   | Oznaczanie treści wygenerowanych przez AI (art. 50 AI Act)                                                         | planowane | —          | termin prawny: 2026-12-02 — patrz [`10`](10-zgodnosc-prawna.md)                                                         |
| C2.10  | SBOM, polityka zgłaszania podatności, kontakt bezpieczeństwa (CRA)                                                 | planowane | —          | przed pierwszym wydaniem publicznym                                                                                     |
| C2.11  | Dokumenty: wyciąganie tekstu z PDF i DOCX w Rust; skany — opcjonalny dodatek                                       | planowane | —          |                                                                                                                         |
| C2.11a | OpenRouter: uwierzytelnione sprawdzenie klucza (`GET /api/v1/key`) zamiast „nie da się sprawdzić z góry”           | planowane | —          | ustalenie 1 przeglądu kluczy; wymaga nowej metody adaptera                                                              |
| C2.12  | Klucze dla firm: nagłówki organizacji i projektu (OpenAI), Azure OpenAI (adres zasobu, wdrożenie, wersja API)      | planowane | —          | opis: [`04`](04-nowe-funkcje.md#klucze-i-konta-dla-firm)                                                                |
| C2.13  | Sieć firmowa: serwer pośredniczący i własny urząd certyfikacji dla połączeń z dostawcami                           | czeka     | —          | dotyka `lib-netguard` i polityk sieci — decyzja właściciela                                                             |
| C2.14  | Okresowe sprawdzanie kluczy w tle, wiek klucza i przypomnienie o wymianie                                          | zrobione  | 2026-10-11 | wersja 0.0.20; `app-core/src/parts/key_watch.rs` (pętla co 10 min, odstęp z `accounts.check_interval_hours`), `Account.key_saved_at` w hubie, przypomnienie liczy interfejs (`accounts.key_max_age_days`); test listą modeli — bez tokenów; niesprawdzone u prawdziwego dostawcy |
| C2.15  | Konto zapasowe: przełączenie na inne konto tego samego dostawcy, gdy pierwsze nie ma środków albo jest ograniczane | zrobione  | 2026-10-11 | wersja 0.0.20; `app-core/src/parts/backup_account.rs`, `app-modules/src/route/accounts.rs` (odstawienie konta po limicie), `RouterCore::reset_health`; komunikat w oknie i wpis w logu — **w osi czasu widać dostawcę, nie konto** (do dopisania przy C2.16); tura z błędem nie jest ponawiana sama |
| C2.16  | Dziennik operacji na kluczach (dodanie, wymiana, wyłączenie, usunięcie, wynik testu) bez treści klucza             | czeka     | —          | audyt jest w Jądrze — decyzja właściciela                                                                               |
| C2.17  | Import kluczy ze zmiennych środowiskowych: przycisk w interfejsie i polecenie rdzenia                              | zrobione  | 2026-10-10 | wersja 0.0.16; polecenie `accounts_import_env` i raport w „Konta i klucze API”; nieuruchamiane na prawdziwych zmiennych |

**Wyjście:** każda funkcja ma kryterium w `ACCEPTANCE.md` (progi ustala właściciel) i wynik ze sprzętu.

### C3 — kamery, rozpoznawanie osób i opis obrazu (opis: [`12-kamery-i-rozpoznawanie-osob.md`](12-kamery-i-rozpoznawanie-osob.md))

Zlecenie właściciela z 2026-10-09. Kolejność: najpierw kamery i opis obrazu (bez biometrii), potem osoby.

| ID    | Zadanie                                                                                                                    | Stan      | Data       | Uwagi                                                                                                                                                                |
| ----- | -------------------------------------------------------------------------------------------------------------------------- | --------- | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C3.1  | Port kamery w `platform-contract` (`CameraPort`) i atrapa: lista urządzeń, tryby, strumień, podłączanie na żywo            | zrobione  | 2026-10-10 | wersja 0.0.16; `platform-contract/src/camera.rs`, atrapa w `platform-fake`                                                                                           |
| C3.2  | Kamery USB na Windows (Media Foundation)                                                                                   | w toku    | —          | kod w `platform-windows-impl/src/camera/` (wersja 0.0.16); **niesprawdzone na prawdziwej kamerze** — kamera laptopa odłączona sprzętowo                              |
| C3.3  | Hub kamer: rejestr, kilka strumieni naraz, usypianie, klatka na żądanie, podgląd, przełącznik główny i kill-switch         | zrobione  | 2026-10-10 | wersja 0.0.16; moduł `camera-hub` (contract, fake, impl), testy na atrapie portu; zdarzenia magistrali i kamery sieciowe — dalej                                     |
| C3.4  | Strona Ustawień „Kamery” i wskaźnik kamery w belce oraz pigułce głosowej                                                   | zrobione  | 2026-10-10 | wersja 0.0.16; strona „Kamery”, wskaźnik w belce i w pigułce głosowej (0.0.18); 3 style × 2 motywy                                                                       |
| C3.5  | Kamery sieciowe: RTSP, wyszukiwanie ONVIF w sieci lokalnej, hasła w Credential Manager, dekoder w osobnym procesie         | do zrobienia | —          | zgoda właściciela 2026-10-11 na bibliotekę dekodującą obraz i pracę w sieci lokalnej; zmiany w `lib-netguard` nadal z przeglądem właściciela                                                                                               |
| C3.6  | Model widzący lokalnie: wybór modelu, pomiar pamięci karty razem z modelem rozmowy na 6 GB                                 | czeka     | —          | zgoda właściciela na pobranie modelu; technologia do sprawdzenia ([`11`](11-technologie-2026-10.md))                                                                 |
| C3.7  | Narzędzie `camera.look` i opis obrazu („opisz mi to, co widzisz”): droga lokalna i chmurowa za zgodą, miniatura w rozmowie | w toku    | —          | 0.0.16: zdjęcie z kamery jako załącznik (przycisk); 0.0.17: prośba „opisz mi to, co widzisz” w tekście sama dołącza zdjęcie (`logic/camera-look.ts`). Dalej: to samo w rozmowie głosowej i narzędzie `camera.look` dla agentek (zdolność Brokera — decyzja właściciela) |
| C3.8  | Wykrywanie twarzy i wzorzec twarzy (ONNX Runtime na procesorze); wybór modeli po sprawdzeniu licencji                      | czeka     | —          | podpis ADR 0025; zgoda na pobranie modeli                                                                                                                            |
| C3.9  | Rejestr osób i kreator „Zapisz osobę” (zgoda, imię, trzy ujęcia); wzorce zaszyfrowane, klucz w Credential Manager          | czeka     | —          | podpis ADR 0025; moduły `person-recognition`, `person-memory`                                                                                                        |
| C3.10 | Rozpoznanie rozmówcy: kto jest przed kamerą, próg pewności, gość bez pamięci, narzędzie `camera.who`, powitanie po imieniu | czeka     | —          | po C3.8–C3.9                                                                                                                                                         |
| C3.11 | Pamięć per osoba: profil, fakty ze źródłem, streszczenie każdej rozmowy; trwała po restarcie                               | czeka     | —          | podpis ADR 0025; opiera się na istniejącym module `memory`                                                                                                           |
| C3.12 | Przypisanie tur do osoby („kto mówił”) i pamięć rozmówcy w kontekście; strona Ustawień „Osoby”                             | czeka     | —          | historia pozostaje tylko do dopisywania                                                                                                                              |
| C3.13 | Łączenie twarzy z głosem (`voice-speaker`): zgodność podnosi pewność, rozbieżność = pytanie                                | czeka     | —          | po C3.10                                                                                                                                                             |
| C3.14 | Zestaw ewaluacyjny: sceny do opisu, osoby do rozróżnienia, napisy-ataki w kadrze, zdjęcie zamiast twarzy                   | planowane | —          | zestaw ataków do `evals/` — zatwierdza właściciel                                                                                                                    |
| C3.15 | Pomiary na sprzęcie i przegląd bezpieczeństwa w świeżym kontekście; kryteria `ACC-V6-camera-…`, `-vision-…`, `-person-…`   | planowane | —          | progi ustala właściciel                                                                                                                                              |

**Wyjście z C3:** „opisz mi to, co widzisz” działa na laptopie właściciela z co najmniej dwiema kamerami naraz;
Alfa rozpoznaje zapisane osoby po restarcie i pamięta, o czym z każdą rozmawiała; żadne kryterium prywatności
z ADR 0025 nie jest złamane (testy w prawdziwej aplikacji).

### C4 — strony Ustawień oznaczone „wkrótce” (spis: [`14-funkcje-wkrotce.md`](14-funkcje-wkrotce.md))

Zgłoszenie właściciela z 2026-10-11: funkcje ze znakiem „wkrótce” mają być zapisane w planie. Test
`app-core/tests/soon_inventory.rs` pilnuje, żeby każda taka strona miała tu zadanie.

| ID   | Zadanie                                                                                                   | Stan      | Data | Uwagi                                                                                          |
| ---- | --------------------------------------------------------------------------------------------------------- | --------- | ---- | ---------------------------------------------------------------------------------------------- |
| C4.1 | Strona „Router i reguły”: klasy zadań i zapas w oknie, tagi prywatności i jurysdykcji                     | planowane | —    | Router już działa; brakuje widoku i edycji reguł (dziś tylko plik)                             |
| C4.2 | Strona „Logi i prywatność”: retencja i redakcja logów, widok „co poszło do chmury”                        | czeka     | —    | widok czyta audyt i tagi prywatności — obszar Jądra; zakres uzgodnić z właścicielem            |
| C4.3 | Strona „Moduły”: lista modułów ze stanem, włącz / wyłącz, budżety RAM i CPU                               | planowane | —    | bez Brokera, watchdoga, audytu i kill-switcha (reguła twarda); część danych jest w „Zdrowie systemu” |
| C4.4 | Strona „Zaawansowane”: edytor surowy TOML ze sprawdzaniem schematu, flagi eksperymentalne                 | planowane | —    | klucze polityki Jądra tylko do odczytu (rdzeń je odrzuca)                                      |

**Wyjście z C4:** w nawigacji Ustawień nie ma znaku „wkrótce”; każda strona przechodzi przegląd w trzech stylach
i dwóch motywach.

### C5 — głos lokalny wysokiej jakości (opis: [`15-glos-lokalny.md`](15-glos-lokalny.md))

Decyzja właściciela z 2026-10-11: lepszy głos ma być lokalny, nie z chmury. **Grupa wchłonięta przez zadanie P1**
(głos w konfiguracji Delty: Supertonic 3 z 6 krokami i wyprzedzeniem o dwa zdania); zadania niżej zostają jako ślad.

| ID   | Zadanie                                                                                                              | Stan         | Data | Uwagi                                                                              |
| ---- | -------------------------------------------------------------------------------------------------------------------- | ------------ | ---- | ---------------------------------------------------------------------------------- |
| C5.1 | Licencje i źródła u źródła: pliki LICENSE silników i wag, stan znaku wodnego, pochodzenie wpisu „Pocket TTS PL”      | do zrobienia | —    | wynik w `docs/vendor/`; bez pobierania modeli                                      |
| C5.2 | Stanowisko pomiarowe głosu i odsłuch: 20 zdań po polsku, pierwszy dźwięk, szybkość, pamięć karty                     | w toku       | —    | zgoda na pobranie 2026-10-11; Supertonic 3, Piper i Chatterbox (RTX 4050) zmierzone — dokument 15, §5a; **czeka na odsłuch właściciela** (`alfa-glos-proba/odsluch.html`) |
| C5.3 | Silnik Chatterbox Multilingual jako proces poboczny bez Pythona (`chatterbox.cpp`) za `TtsBackend`                   | czeka        | —    | pakiet producenta za wolny (4,8 s na zdanie); wymaga odsłuchu, potem pomiaru `chatterbox.cpp` (osobna zgoda na budowę ze źródeł) |
| C5.4 | Głos szybki na procesorze: próba Supertonic 3 (`tract` albo proces poboczny); do tego czasu Piper                    | czeka        | —    | zgoda na pobranie; licencja wag OpenRAIL-M do przeczytania; repozytorium archiwalne |
| C5.5 | Automatyczny wybór poziomu głosu (wolna karta, bateria, budżet opóźnienia) i ustawienie w zakładce „Głos”            | planowane    | —    | po C5.3                                                                            |
| C5.6 | Cztery głosy wzorcowe agentek z krótkich próbek                                                                      | czeka        | —    | decyzja właściciela: skąd próbki; nigdy głos prawdziwej osoby bez jej zgody        |
| C5.7 | Karta licencji każdego silnika i głosu w programie; oznaczenie mowy syntetycznej (znak wodny)                        | planowane    | —    | łączy się z C2.9 (art. 50) i C2.10 (SBOM); decyzja o znaku wodnym                  |

**Wyjście z C5:** właściciel w odsłuchu 20 zdań ocenia głos jako wyraźnie lepszy od Pipera; cztery agentki mają
różne głosy; pierwszy dźwięk mieści się w budżecie z `docs/VOICE.md` na laptopie bazowym; rozmowa nie urywa się,
gdy karta jest zajęta (przełączenie na poziom szybki).

## Etap P — Przeniesienie funkcji z Delty AI

Opis: [`17-przeniesienie-z-delty.md`](17-przeniesienie-z-delty.md). Decyzja właściciela z 2026-10-11: Alfa jest programem docelowym; z Delty AI 0.8.35 przenosimy system rozmowy
i mowy oraz wszystkie brakujące funkcje. P1 i P2 mają pierwszeństwo — zamykają A.5 i A.4.

| ID  | Zadanie                                                                                                                              | Stan         | Data | Uwagi                                                                                   |
| --- | ------------------------------------------------------------------------------------------------------------------------------------ | ------------ | ---- | --------------------------------------------------------------------------------------- |
| P0  | Dokumenty: specyfikacja głosu Delty ze stałymi i macierz wszystkich funkcji Delty (ok. 400 pozycji) z decyzją dla każdej              | zrobione     | 2026-10-11 | dokumenty 16, 17, 19 (specyfikacja głosu), 20 i 21 (macierz); do przeglądu właściciela; uzupełniane, gdy przy pracy wyjdzie pominięta funkcja |
| P1  | Rozmowa i mowa: Supertonic 3, 17 głosów, Qwen3-ASR, zabezpieczenia rozpoznawania, bezpiecznik, Smart Turn, ekran „Rozmowa na żywo”    | w toku       | —    | 0.0.30: silnik Supertonic 3 jako sidecar Rust (`lib-supertonic`), cztery barwy agentek, pozycja katalogu z sumami; pomiar 0,97 s na zdanie. 0.0.31: wybór barwy dla każdej agentki w Ustawieniach (10 barw); wyprzedzenie syntezy Alfa już ma (kolejka 16 zdań). Pomiar Qwen3-ASR na laptopie: 0,44 s na zdanie (dokument 19, §2) — ale **wymaga nowszego `llama-server`** niż wydanie 6710 w katalogu Alfy. 0.0.32: silnik `llama.cpp` b11559 w katalogu z przypiętymi sumami. 0.0.33: Qwen3-ASR ze strażnikiem, bezpiecznikiem i Whisperem jako zapasem (`voice-stt-impl`: `qwen.rs`, `guard.rs`, `breaker.rs`, `fallback.rs`). 0.0.34: Smart Turn v3.2 jako sidecar `alfa-smart-turn` (`lib-smart-turn`, klient w `voice-turn-impl/src/smart_turn.rs`), czasy rozmowy 250 / 350 / 2000 ms, model i biblioteka ONNX Runtime w katalogu pobrań. 0.0.35: karta „Silniki rozmowy” ze stanem strażnika i zamknięciem bezpiecznika (`voice_engines`), cztery ustawienia rozmowy (`voice.talk_*`), wybór karta / procesor wg wolnej pamięci karty, zwalnianie silnika po bezczynności, nowe silniki w pakietach, pisownia imion z rozpoznawania mowy. 0.0.37: liczniki Smart Turn. 0.0.38: wybór silnika rozpoznawania wg zajętości karty, mały Whisper na procesor. 0.0.39: próba na prawdziwym mikrofonie (bez mowy człowieka). 0.0.41: „Sprawdź silniki” (próba rozpoznawania, końca wypowiedzi i głosu bez mikrofonu). 0.0.42: język rozpoznawania mowy w ustawieniach (domyślnie polski). 0.0.43: zwarta karta silników w panelu „Głos”. **Zostaje:** próba z żywą mową i tłumieniem echa (właściciel), kula stanu i miernik w nagłówku rozmowy, głosy Pipera z parametrami, głos Edge jako opcja (połączenie z siecią — przegląd właściciela) |
| P2  | Modele lokalne: Sklep modeli, rozkład na karty, dopasowanie do pamięci karty, poprawki szablonów, warianty silnika, panel System      | w toku       | —    | zamyka A.4; łączy się z C2.2. 0.0.35: nowe silniki głosu w pakietach, wybór karta / procesor wg wolnej pamięci karty. 0.0.36: monitor kart graficznych na żywo (`device_gpu_live`, karty NVIDIA). 0.0.39: ocena „mieści się na karcie” przy modelach rozmowy. **Zostaje:** Sklep modeli, rozkład na karty, pięć stopni awaryjnego startu, poprawki szablonów, warianty silnika z wycofaniem, panel System, monitor dla kart AMD / Intel |
| P3  | Nawigacja i powłoka: menu kołowe, doki, okna odczepiane, pulpit kafelkowy, Wstecz / Dalej, tabela skrótów                             | w toku       | —    | obok obecnej nawigacji Alfy 0.0.37: „Wstecz / Dalej” (Alt+←, Alt+→, boczne przyciski myszy, 50 miejsc). 0.0.42: wyszukiwarka w ściągawce skrótów. **Zostaje:** menu kołowe, doki, okna odczepiane, pulpit kafelkowy, ekran startowy |
| P4  | Rozmowa: karty z profilem, Tablica, Tryb skupienia, Arena, załączniki z czytnikami, archiwum i eksporty, szablony                     | planowane    | —    | łączy się z C2.8                                                                        |
| P5  | Sieć i wiedza: wyszukiwanie, głębokie badanie, wiadomości, „Naucz”, polecenia bez modelu, tłumaczenie, instrukcja                     | planowane    | —    | treść z sieci oznaczana jako niezaufana; łączy się z C2.6, C2.11                        |
| P6  | Osoby: baza osób, rozpoznawanie twarzy i głosu, pamięć per osoba                                                                      | planowane    | —    | ADR 0025 przyjęty 2026-10-11; realizuje C3.8–C3.13; łączy się z etapem U                |
| P7  | Komputer: sterowanie do poziomu Delty przez Brokera, menedżer procesów, spis urządzeń, ekran uprawnień                                | czeka        | —    | realizuje C1.3; zmiany zdolności Brokera — przegląd właściciela                         |
| P8  | Twórczość: Studio obrazu, dostawcy obrazów, tryb hybrydowy, Galeria, Studio opowieści, Audio, menedżer kamer                          | planowane    | —    | modele obrazu: pobrania powyżej 3 GB za osobną zgodą                                    |
| P9  | Awatar: 2D / 2.5D / 3D / VRM, emocje, ruch ust (12 kształtów), osobne okno                                                            | planowane    | —    |                                                                                         |
| P10 | Odtwarzanie multimediów i sterowanie głosem (bez biblioteki)                                                                          | planowane    | —    | pobieranie z serwisów — osobna decyzja właściciela                                      |
| P11 | Przepływy, role i przepływ wieloagentowy, edytor persony, laboratorium treningu, pulpit testowy, personalizacja                       | planowane    | —    | łączy się z C1.5                                                                        |
| P12 | Agentki programistki: pisanie, uruchamianie i budowanie oprogramowania w katalogu projektu, rozmowa między agentkami w trakcie pracy, dobór modelu do trudności zadania | planowane    | —    | zlecenie właściciela 2026-10-11; nadrzędny przycisk „Pełne uprawnienia” jak w Claude Code i Codex (szybki włącznik poziomu L4, włącza tylko człowiek, stale widoczny); zmiana w Brokerze — przegląd właściciela |
| P13 | Dyski sieciowe i chmurowe: podłączanie (SMB / NAS, WebDAV, SFTP, S3, OneDrive, Google Drive, Dropbox), przeglądanie i praca agentek na plikach, kopie zapasowe z harmonogramem, wersjami i przywracaniem | planowane    | —    | zlecenie właściciela 2026-10-11; pełne uprawnienia do tworzenia kopii; hasła i tokeny w Menedżerze poświadczeń; dotyka `lib-netguard` — przegląd właściciela |
| P14 | Obowiązki codzienne agentek: przypisanie agentki do stałego zadania (poczta, kalendarz, przegląd wiadomości, przypomnienia, porządki w plikach, raport dnia) z harmonogramem, budżetem i raportem | w toku      | —    | zlecenie właściciela 2026-10-11; rozwija C2.7 i moduły `scheduler`, `triggers`; łączniki poczty i kalendarza — nowe. 0.0.40: cztery gotowe obowiązki (przegląd dnia, podsumowanie, porządki, koszty) wypełniające formularz wyzwalacza; poczta i kalendarz czekają na łączniki. 0.0.44: wzory ukryte po próbie na lokalnym modelu (zmyślony raport). 0.0.45: rdzeń dokleja do zadania w tle arkusz faktów (zadania, wyzwalacze, koszty, katalog roboczy), próba na lokalnym modelu dała raporty zgodne z danymi — wzory WRÓCIŁY |

**Wyjście z P:** każda pozycja macierzy funkcji Delty ma stan „jest / przeniesione / odrzucone z powodem”, a to,
co przeniesione, ma test i wynik z laptopa właściciela.

## Etap U — Wielu użytkowników

Opis: [`18-wielu-uzytkownikow.md`](18-wielu-uzytkownikow.md). Zlecenie właściciela z 2026-10-11: konta i logowanie na wszystkich celach. Projekt decyzji: ADR 0027.

| ID  | Zadanie                                                                                                   | Stan      | Data | Uwagi                                          |
| --- | --------------------------------------------------------------------------------------------------------- | --------- | ---- | ---------------------------------------------- |
| U.0 | Podpis właściciela pod ADR 0027                                                                           | czeka     | —    | zmienia model uprawnień — obszar Jądra         |
| U.1 | Moduł kont: rejestr, role, logowanie lokalne (Windows Hello, klucz dostępu, PIN), blokada                 | czeka     | —    | po U.0                                         |
| U.2 | Rozdzielenie danych na konta i migracja dotychczasowych danych na konto właściciela                       | czeka     | —    | po U.1                                         |
| U.3 | Klucze API, limity kosztów i poziom autonomii na konto                                                    | czeka     | —    | po U.1                                         |
| U.4 | Broker: konto w prośbie o zgodę, ograniczenia roli, dziennik z kontem                                     | czeka     | —    | przegląd właściciela                           |
| U.5 | Okno: ekran logowania, przełącznik kont w belce, strona Ustawień „Użytkownicy”                            | czeka     | —    | po U.1                                         |
| U.6 | Role Dziecko i Gość: filtr treści, brak pamięci gościa                                                    | czeka     | —    | po U.2                                         |
| U.7 | Połączenie z bazą osób: rozpoznanie głosu i twarzy wybiera konto                                          | czeka     | —    | po U.2 i P6                                    |
| U.8 | Konta w dostępie zdalnym: parowanie urządzenia z kontem, logowanie w panelu WWW i w telefonie             | czeka     | —    | po U.1, D.2, E.3, F.2                          |
| U.9 | Testy ataków (przejęcie sesji, podniesienie roli, dostęp do cudzych danych) i przegląd w świeżym kontekście | planowane | —    |                                                |

**Wyjście z U:** dwie osoby korzystają z jednej Alfy na komputerze i z dwóch telefonów, nie widząc swoich danych;
dziecko nie uruchomi działania w systemie; właściciel odbiera dostęp jednym kliknięciem.

## Etap D — Host bez okna i protokół zdalny

| ID  | Zadanie                                                                   | Stan  | Data | Uwagi                                            |
| --- | ------------------------------------------------------------------------- | ----- | ---- | ------------------------------------------------ |
| D.0 | Podpis właściciela pod ADR 0019–0023                                      | czeka | —    | bez tego nie powstaje kod                        |
| D.1 | `remote-gateway`: sesje klientów, ACL, ramki, wznowienie                  | czeka | —    | po B.1–B.3, B.5                                  |
| D.2 | `device-pairing`: QR, klucze urządzeń, lista, odwołanie                   | czeka | —    |                                                  |
| D.3 | `remote-relay`: przekaźnik szyfrogramu + obraz kontenera + instrukcja     | czeka | —    |                                                  |
| D.4 | Kanał E2E (wzorzec Noise) i test „przekaźnik nie widzi treści”            | czeka | —    |                                                  |
| D.5 | Dowód zgody z urządzenia sparowanego w Brokerze                           | czeka | —    | ADR 0022; przegląd właściciela                   |
| D.6 | `AlfaClient` po kanale zdalnym (TypeScript)                               | czeka | —    | trzecia implementacja obok Tauri i atrapy        |
| D.7 | `host-headless`: binarka `alfa-host`                                      | czeka | —    | drugi krok — najpierw brama w procesie aplikacji |
| D.8 | Aktualizacja modelu zagrożeń i test „żadnego nasłuchu” dla nowych modułów | czeka | —    |                                                  |
| D.9 | Zestaw ataków na dostęp zdalny (pisze sesja-recenzentka)                  | czeka | —    |                                                  |

**Wyjście:** drugi komputer łączy się z hostem przez przekaźnik, prowadzi rozmowę, zatwierdza działanie; zrywane
połączenie wznawia się bez utraty zdarzeń; test dowodzi, że przekaźnik widzi tylko szyfrogram.

## Etap E — Panel WWW

| ID  | Zadanie                                                      | Stan      | Data | Uwagi                                        |
| --- | ------------------------------------------------------------ | --------- | ---- | -------------------------------------------- |
| E.1 | Układ interfejsu na wąski ekran (od 360 px)                  | planowane | —    | można wcześniej — korzysta też okno Tauri    |
| E.2 | `web-panel`: budowa PWA z tego samego kodu Svelte            | planowane | —    |                                              |
| E.3 | Logowanie kluczem dostępu (passkey) i parowanie przeglądarki | planowane | —    |                                              |
| E.4 | Zgody w panelu z potwierdzeniem kluczem dostępu              | planowane | —    | po D.5                                       |
| E.5 | Powiadomienia push bez treści („obudź i pobierz”)            | planowane | —    |                                              |
| E.6 | Tryb bez komputera w przeglądarce                            | czeka     | —    | ADR 0023 — kwestia przechowywania klucza API |
| E.7 | Hosting panelu i polityka bezpieczeństwa treści (CSP)        | planowane | —    |                                              |

**Wyjście:** z przeglądarki w telefonie i na obcym komputerze da się zrobić wszystko, co pozwala mapa zdolności
hosta; audyt dostępności (axe) zielony na 360 px i 1280 px.

## Etap F — Android 15+

| ID  | Zadanie                                                                | Stan      | Data | Uwagi                                    |
| --- | ---------------------------------------------------------------------- | --------- | ---- | ---------------------------------------- |
| F.1 | Projekt Tauri mobile, budowa APK w CI                                  | planowane | —    | Tauri 2.x; `crate-type` już przygotowany |
| F.2 | `platform-android`: magazyn kluczy, biometria, powiadomienia           | planowane | —    | wtyczki Kotlin tam, gdzie brak gotowych  |
| F.3 | Parowanie aparatem (QR)                                                | planowane | —    |                                          |
| F.4 | Pilot 100 %: rozmowa, agentki, pliki hosta, zgody, STOP                | planowane | —    | po D i E.1                               |
| F.5 | Głos w telefonie: mikrofon i głośnik telefonu, przetwarzanie na hoście | planowane | —    | usługa pierwszoplanowa                   |
| F.6 | `offline-client`: rozmowa przez API bez hosta + scalenie po powrocie   | czeka     | —    | ADR 0023                                 |
| F.7 | Testy na urządzeniu i w emulatorze, kryteria akceptacji                | planowane | —    | telefon testowy — właściciel             |
| F.8 | Publikacja (APK do pobrania; sklep — decyzja właściciela)              | planowane | —    |                                          |

**Wyjście:** telefon sparowany w mniej niż minutę; pełna obsługa z telefonu; po wyłączeniu komputera rozmowa
działa przez model chmurowy i po powrocie pojawia się w historii na komputerze.

## Etap G — Linux

| ID  | Zadanie                                                                                 | Stan      | Data | Uwagi                                                              |
| --- | --------------------------------------------------------------------------------------- | --------- | ---- | ------------------------------------------------------------------ |
| G.1 | `platform-linux` — podstawa: pliki z koszem, procesy, schowek, sekrety (Secret Service) | planowane | —    | po B.4                                                             |
| G.2 | Powłoka: analizator POSIX w kontrakcie i Jądrze                                         | czeka     | —    | po B.7; przegląd właściciela                                       |
| G.3 | Jądro poza procesem: usługa użytkownika, gniazdo Unix z kontrolą UID                    | czeka     | —    | przegląd właściciela                                               |
| G.4 | Okno zgód na Linuksie i dowód fizycznego wejścia                                        | badawcze  | —    | Wayland nie daje prostego odpowiednika — patrz [`06`](06-linux.md) |
| G.5 | Audio (PipeWire) i głos                                                                 | planowane | —    |                                                                    |
| G.6 | Skróty globalne, zasobnik, okna (portale XDG)                                           | planowane | —    |                                                                    |
| G.7 | Silniki: llama.cpp i whisper.cpp dla Linuksa (CUDA, Vulkan, CPU)                        | planowane | —    | po B.6                                                             |
| G.8 | Pakiety: deb i rpm, potem Flatpak; test WebKitGTK na karcie NVIDIA w CI                 | planowane | —    |                                                                    |
| G.9 | Sterowanie komputerem (AT-SPI, portale)                                                 | badawcze  | —    |                                                                    |

**Wyjście:** scenariusz MVP 6/6 na Ubuntu LTS z kartą NVIDIA; zdolności niedostępne są ukryte, nie zepsute.

## Etap H — Badawcze (bez terminu, bez zobowiązania)

| ID  | Temat                                                        | Stan     | Uwagi                                                                                |
| --- | ------------------------------------------------------------ | -------- | ------------------------------------------------------------------------------------ |
| H.1 | Mały model na telefonie (LiteRT-LM / llama.cpp, Gemma 4 E2B) | badawcze | decyzja właściciela z 2026-10-09: model lokalny tylko na komputerze — temat odłożony |
| H.2 | MCP Apps (interfejsy narzędzi w izolowanej ramce)            | badawcze | po C2.5 i C2.8                                                                       |
| H.3 | Rozmowa głosowa w pełnym dupleksie po polsku                 | badawcze | dziś brak modelu lokalnego na 6 GB                                                   |
| H.4 | Akceleracja NPU (Windows ML / Foundry Local)                 | badawcze | drugi format modeli obok GGUF                                                        |
| H.5 | Alfa jako serwer MCP / agent ACP dla edytorów                | badawcze |                                                                                      |
| H.6 | Protokół A2A                                                 | badawcze | dziś mało przydatny w programie dla jednej osoby                                     |
| H.7 | Synchronizacja pełna (CRDT: Loro / Automerge)                | badawcze | tylko jeśli tryb pilota okaże się niewystarczający                                   |

## Decyzje właściciela, które blokują zadania

| Decyzja                                            | Blokuje                               |
| -------------------------------------------------- | ------------------------------------- |
| Zgoda na pobranie pakietu modeli (ok. 6 GB)        | A.4, A.5, A.6, C1.4, C2.2             |
| `gh auth login` i wypchnięcie commitów             | A.0, A.8                              |
| Ustalenia bezpieczeństwa (B-…, S-…, C-…, R-…, P-…) | A.7, A.8, B.7                         |
| Podpis pod ADR 0017–0018                           | B.4, B.7, G                           |
| Podpis pod ADR 0019–0023                           | D, E, F                               |
| ADR 0024 — licencja silnika mowy                   | C1.1 (wybór głosu), C5.3 (dołączenie do wydania), wydanie publiczne |
| ADR 0026 — głos z chmury jako opcja | C1.1a |
| ADR 0027 — konta, logowanie i role | etap U |
| Pobieranie multimediów z serwisów internetowych | część P10 |
| Pobrania powyżej 3 GB (modele obrazu, duże modele rozmowy) — poniżej tej granicy zgoda stała z 2026-10-11 | części P2 i P8 |
| Znak wodny w mowie syntetycznej; skąd głosy wzorcowe agentek | C5.6, C5.7 |
| Czy obraz z kamery może iść do modelu w chmurze    | C3.7 (droga chmurowa)                 |
| Zgoda na pobranie modelu widzącego i modeli twarzy | C3.6, C3.8                            |
| Zakres widoku „co poszło do chmury” (czyta audyt i tagi prywatności) | C4.2 |
| Progi liczbowe zestawu `evals/V6/pl-core` (zestaw zaakceptowany do pracy 2026-10-11) | C2.1 (bramka w CI) |
| Progi nowych kryteriów w `ACCEPTANCE.md`           | wyjście z C, D, E, F, G               |

## Dziennik roadmapy

| Data       | Zmiana                                                                                                                                                                                                                                                                              |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-10-12 | Wersja 0.0.45: Historia zmian — dziesięć ostatnich wersji i zakładka „Starsze”, ta sama lista w oknie „Co nowego”; P14 — arkusz faktów dla zadań w tle i powrót gotowych obowiązków (próba na lokalnym modelu); P1 — kula stanu rozmowy w panelu „Głos”; P2 — wyszukiwarka, kolejność i miejsce na dysku w katalogu modeli. |
| 2026-10-12 | Wersja 0.0.44: P14 — gotowe obowiązki ukryte po próbie na lokalnym modelu (raport z wymyślonymi danymi); pobieranie nowych pozycji samym programem sprawdzone na świeżym profilu. |
| 2026-10-12 | Wersja 0.0.43: P1 — karta silników w bocznym panelu „Głos”; test płynności strumienia doprecyzowany — testy w przeglądarce 324 z 324. Raport nocy: dokument [`23`](23-raport-nocny-2026-10-12.md). |
| 2026-10-12 | Wersja 0.0.42: P1 — język rozpoznawania mowy w ustawieniach; P3 — wyszukiwarka w ściągawce skrótów. Pełne testy projektu zielone (945 programów testowych). |
| 2026-10-12 | Wersja 0.0.41: P1 — „Sprawdź silniki”: próba każdego silnika rozmowy z czasem i przyczyną błędu. |
| 2026-10-12 | Wersja 0.0.40: P14 — gotowe obowiązki codzienne agentek (cztery szablony z harmonogramem i przypisaną agentką). |
| 2026-10-12 | Wersja 0.0.39: P2 — ocena „mieści się na karcie” przy modelach rozmowy; P1 — próba potoku rozmowy na prawdziwym mikrofonie (po cichu). |
| 2026-10-12 | Wersja 0.0.38: P1 — wybór silnika rozpoznawania wg zajętości karty (pomiary: Qwen3-ASR 0,26 s na karcie, 2,4 s na procesorze; Whisper turbo 54 s na procesorze), mały model Whisper jako zapas na procesor. |
| 2026-10-12 | Wersja 0.0.37: P3 — „Wstecz / Dalej” po odwiedzonych miejscach; P1 — liczniki modelu Smart Turn na karcie „Silniki rozmowy”. |
| 2026-10-12 | Wersja 0.0.36: P2 — karty graficzne na żywo na stronie „Urządzenia” (pamięć, obciążenie, temperatura, moc, programy; silniki Alfy oznaczone). |
| 2026-10-12 | Wersja 0.0.35: P1 — karta „Silniki rozmowy” (stan strażnika, bezpiecznik, telemetria), ustawienia rozmowy, wybór karta / procesor dla Qwen3-ASR wg wolnej pamięci, nowe silniki głosu w pakietach. |
| 2026-10-11 | Wersja 0.0.34: P1 — koniec wypowiedzi oceniany modelem Smart Turn v3.2 (osobny proces z ONNX Runtime; zgodność z wzorcem wydawcy; 25–45 ms na ocenę); model i biblioteka w katalogu pobrań. |
| 2026-10-11 | Wersja 0.0.33: P1 — rozpoznawanie mowy Qwen3-ASR ze strażnikiem jakości, bezpiecznikiem i silnikiem zapasowym; model w katalogu pobrań. |
| 2026-10-11 | Wersja 0.0.32: silnik `llama.cpp` b11559 w katalogu pobrań (wymagany przez Qwen3-ASR), sumy SHA-256 przypięte; pomiar rozpoznawania 0,43 s na zdanie. Decyzja właściciela: aplikację budować co pięć dużych ulepszeń albo na polecenie. |
| 2026-10-11 | Wersja 0.0.31: P1 — wybór barwy głosu dla każdej agentki (ustawienia `voice.tts_voice_*`). |
| 2026-10-11 | Wersja 0.0.30: plan zatwierdzony przez właściciela; start P1 — głos Supertonic 3 (sidecar `alfa-supertonic`, cztery barwy agentek, model w katalogu pobrań). |
| 2026-10-11 | Wersja 0.0.29: P0 zrobione (specyfikacja głosu — dokument [`19`](19-specyfikacja-glosu-delty.md), macierz — [`20`](20-macierz-delty-czesc-1.md), [`21`](21-macierz-delty-czesc-2.md)); nowe grupy P12 (agentki programistki z przyciskiem „Pełne uprawnienia”), P13 (dyski sieciowe i chmurowe, kopie), P14 (obowiązki codzienne); nowości z rynku R1–R13 (dokument [`22`](22-nowosci-2026-10.md)); macierz porównana ze zrzutami menu Delty. |
| 2026-10-11 | Wersja 0.0.28: plan zbiorczy (dokument [`16`](16-plan-zbiorczy.md)), nowy etap P — przeniesienie funkcji z Delty AI (dokument [`17`](17-przeniesienie-z-delty.md)) i nowy etap U — wielu użytkowników (dokument [`18`](18-wielu-uzytkownikow.md), ADR 0027). Decyzje właściciela: Alfa programem docelowym, ADR 0025 przyjęty, stała zgoda na pobrania do 3 GB. |
| 2026-10-11 | Wersja 0.0.27: poprawki z przeglądu paneli bocznych i nakładek w skali 1:1 — kolejność kart zgodna ze skrótami `Alt+1…7`, pole wiadomości w Zadaniach, opis polecenia `/pamięć`. |
| 2026-10-11 | Wersja 0.0.26: pierwszy pomiar głosów na laptopie bazowym (C5.2 w toku) — Chatterbox z pakietu producenta i Supertonic 3 są za wolne do rozmowy, decyzja po odsłuchu; poprawki z przeglądu stron Ustawień w pełnej skali i nowa reguła bramki (niezdefiniowane zmienne stylów). |
| 2026-10-11 | Wersja 0.0.25: dodatek o głosie lokalnym (dokument [`15`](15-glos-lokalny.md)) i nowa grupa C5. Decyzja właściciela: lepszy głos lokalnie (Chatterbox Multilingual + głos szybki na procesorze), chmura (C1.1a) na koniec kolejki. |
| 2026-10-11 | Wersja 0.0.24: zautomatyzowany przegląd okien, paneli i skrótów (`e2e/review-sweep.spec.ts`, raport w `docs/reviews/2026-10-11-przeglad-okien-i-skrotow.md`); naprawione szukanie w sesjach na oknie szerokości laptopa. |
| 2026-10-11 | Wersja 0.0.23: historia zmian w Aktualizacjach (wagi krytyczna / poważna / zwykła), spis funkcji „wkrótce” (dokument [`14`](14-funkcje-wkrotce.md)) i nowa grupa C4. Zgoda właściciela na: kamery sieciowe (C3.5), `schemars` w generatorze typów (B.1), dalszą pracę nad zestawem `evals/V6/pl-core` (C2.1). |
| 2026-10-11 | Wersja 0.0.22: C2.1 — szkic zestawu ewaluacyjnego po polsku (`evals/V6/pl-core`), do akceptacji właściciela. C3.5 (kamery sieciowe) wstrzymane: wymaga nowej zależności do dekodowania obrazu — decyzja właściciela. |
| 2026-10-11 | Wersja 0.0.21: B.1 etap 1 — schemat łącza i typy TS generowane z Rusta, kontrola zgodności w kompilacji, bramce i CI. |
| 2026-10-11 | Wersja 0.0.20: C2.14 (sprawdzanie kluczy w tle, wiek klucza) i C2.15 (konto zapasowe tego samego dostawcy). |
| 2026-10-11 | Wersja 0.0.19: B.2 (numery zdarzeń, wznowienie, pozycja w deltach tekstu) i B.3 (błędy z kodem i szczegółami docierają do okna także w prawdziwej aplikacji). 226 poleceń. |
| 2026-10-10 | Wersja 0.0.16 (noc bez właściciela): kamery USB — port, hub, strona „Kamery”, wskaźnik, zdjęcie jako załącznik (C3.1–C3.4, część C3.7); import kluczy ze zmiennych (C2.17); pakiet decyzji A.7; podsumowanie i dalsze kroki w dokumencie [`13`](13-podsumowanie-i-dalsze-kroki.md). |
| 2026-10-09 | Zlecenie właściciela: grupa C3 — kamery (USB, IP, Wi-Fi, kilka naraz), rozpoznawanie zapisanych osób z pamięcią długoterminową, opis obrazu na polecenie. Dokument [`12`](12-kamery-i-rozpoznawanie-osob.md), cztery szkice SPEC, projekt ADR 0025. Tylko plan — kod nie powstał.   |
| 2026-10-09 | Wersja 0.0.8: naprawy po próbie na laptopie (A.3e) — rozmowa przez API, kolejki modeli, głos. Właściciel pobrał silniki głosu i model lokalny: pierwsza rozmowa lokalna i nasłuch działają (A.4/A.5 w toku).                                                                        |
| 2026-10-09 | Wersja 0.0.7: dodawanie klucza API od nowa (A.3d); zadania C2.12–C2.17 (klucze dla firm) dopisane do planu.                                                                                                                                                                         |
| 2026-10-09 | Wersja 0.0.6: naprawy po przeglądzie zmian 0.0.1–0.0.5 (A.3c); numer wersji zmienia skrypt.                                                                                                                                                                                         |
| 2026-10-09 | Wersja 0.0.4: listy wyboru i inne kontrolki rysowane przez Alfę (A.3a).                                                                                                                                                                                                             |
| 2026-10-09 | Powstanie planu v6. Etapy A.1–A.3 wpisane jako zrobione (wersje 0.0.2 i 0.0.3). Cztery decyzje właściciela o dostępie zdalnym.                                                                                                                                                      |
