# 03 — Roadmapa (dokument żywy)

Ostatnia aktualizacja: **2026-10-09** (wersja programu 0.0.16). Statusy: patrz [`README.md`](README.md).
Stan kryteriów akceptacji żyje w [`../STATUS.md`](../STATUS.md); tu są zadania i etapy.

Zasada kolejności: **etap A ma pierwszeństwo**. Etap B można prowadzić równolegle z A, bo nie dodaje funkcji.
Etapy C–G startują po zamknięciu MVP (A.8) — z wyjątkiem zadań oznaczonych „można wcześniej”. Technicznie etap G
(Linux) wymaga tylko etapu B; o tym, czy ruszy przed D–F, decyduje właściciel.

```
A (MVP Windows) ══▶ C (funkcje Windows)
   │
   └▶ B (szwy) ─▶ D (host i protokół) ─▶ E (panel WWW) ─▶ F (Android)
        └───────────────────────────────▶ G (Linux)               H (badawcze — bez terminu)
```

## Etap A — Domknięcie Windows v1 (MVP)

Szczegółowe zadania: [`../KONTYNUACJA.md`](../KONTYNUACJA.md) §6, etapy 0–3. Tutaj tylko stan zbiorczy.

| ID | Zadanie | Stan | Data | Uwagi |
| --- | --- | --- | --- | --- |
| A.0 | Porządek w środowisku i repozytorium (KONTYNUACJA etap 0) | w toku | — | zostało: `gh auth login` (właściciel), przegląd gałęzi Dependabota, pełne `gates.ps1 -Full` |
| A.1 | Skalowanie i układ interfejsu 1280–3840 px | zrobione | 2026-10-08 | wersja 0.0.2; testy e2e i test w prawdziwej aplikacji |
| A.2 | Naprawy z przeglądów rdzenia, narzędzi i stanu interfejsu | zrobione | 2026-10-09 | wersja 0.0.2; lista w `CHANGELOG.md` |
| A.3 | Style interfejsu „Szkło” i „Next” (jasny i ciemny) | zrobione | 2026-10-09 | wersja 0.0.3; `e2e/skins.spec.ts` |
| A.3a | Własne listy wyboru i pozostałe kontrolki w stylu Alfy (bez kontrolek systemowych) | zrobione | 2026-10-09 | wersja 0.0.4; błąd zgłoszony przez właściciela; testy listy otwartej w 6 kombinacjach + prawdziwa aplikacja |
| A.3b | Okno „Co nowego” ze skrótem poprawek z dziennika zmian | zrobione | 2026-10-09 | wersja 0.0.5; zgłoszenie właściciela; test pilnuje wpisu dla każdej wersji |
| A.3c | Przegląd zmian 0.0.1–0.0.5 w świeżym kontekście i naprawa ustaleń | zrobione | 2026-10-09 | wersja 0.0.6; 3 przeglądy (interfejs, Rust, skrypty i testy), 0 krytycznych; raport: `docs/reviews/2026-10-09-przeglad-0.0.1-0.0.5.md` |
| A.3d | Dodawanie klucza API: diagnoza przyczyny, sprzątanie wklejonego klucza, wymiana i wyłączanie konta | zrobione | 2026-10-09 | wersja 0.0.7; zgłoszenie właściciela („401 Klucz odrzucony”); 3 przeglądy w świeżym kontekście; bez próby z prawdziwym kluczem; raport: `docs/reviews/2026-10-09-przeglad-kluczy-api.md` |
| A.3e | Naprawy po pierwszej próbie 0.0.7 na laptopie: wybór modelu rozmowy, strona „API i kolejki modeli”, redukcja szumów, koniec wypowiedzi w rozmowie głosowej, filtr zmyślonych wywołań narzędzi | zrobione | 2026-10-09 | wersja 0.0.8; pięć zgłoszeń właściciela; zegar potoku sprawdzony na mikrofonie laptopa |
| A.3f | Porządkowanie interfejsu, krok 1: przejścia konto ↔ kolejki modeli, przełącznik szczegółów odpowiedzi | zrobione | 2026-10-09 | wersja 0.0.9; dalsze kroki wymagają wskazań właściciela (lista propozycji w raporcie sesji) |
| A.3g | Własne menu kontekstowe i podpowiedzi (bez elementów systemowych), kolejka modeli jako kafelki do przenoszenia, nowe nazwy stron ustawień modeli, osobna strona „Mosty CLI” | zrobione | 2026-10-09 | wersja 0.0.10; zgłoszenia właściciela |
| A.3h | Własna belka okna z paskiem LED i przyciskami okna, nawigacja Ustawień od nowa (ikony, zwijane grupy, szukanie z licznikami, klawiatura, wąskie okno) | zrobione | 2026-10-09 | wersja 0.0.11; zgłoszenia właściciela; bez Snap Layouts po najechaniu |
| A.3i | Kilka kart graficznych: role kart, przypinanie modelu do karty głównej, lekki dostawca lokalny na karcie zintegrowanej (klasa „streszczenia”) | zrobione | 2026-10-09 | wersja 0.0.11 (kod i testy); praca na karcie zintegrowanej niesprawdzona na sprzęcie — wymaga silnika Vulkan (zgoda właściciela na pobranie) |
| A.3j | Belka: obsada agentek, mikrofon i stan (profil, autonomia, koszt) od nowa, objaśnienie stanu | zrobione | 2026-10-09 | wersja 0.0.12; zgłoszenie właściciela |
| A.3k | Znak Alfy: ikona programu i znak z nazwą na belce okna | zrobione | 2026-10-09 | wersja 0.0.13; zgłoszenie właściciela |
| A.3l | Belka z torem paska LED, ekran ładowania, żywe awatary agentek z ustami w rytm mowy (zdarzenie `SpeechLevel`), strojenie mowy w „Głos” (styl, tempo, pauzy, różnice głosów) | zrobione | 2026-10-09 | wersja 0.0.14; brzmienie do oceny właściciela; inne głosy niż Piper „Gosia” — osobna decyzja |
| A.4 | Pierwsza rozmowa z modelem lokalnym na laptopie (KONTYNUACJA etap 1) | czeka | — | zgoda właściciela na pobranie pakietu ok. 6 GB |
| A.5 | Głos na żywo: dyktowanie i czytanie | czeka | — | po A.4 (te same silniki w pakiecie) |
| A.6 | Pomiary MVP na sprzęcie, F0–F3 (KONTYNUACJA etap 2) | czeka | — | po A.4 |
| A.7 | Decyzje bezpieczeństwa: B-01…B-08, S-01…S-04, S-06, C-02, C-03, R-05, P-… | czeka | — | właściciel; pakiet gotowy: [`reviews/2026-10-10-pakiet-decyzji-wlasciciela.md`](../reviews/2026-10-10-pakiet-decyzji-wlasciciela.md) (44 decyzje z rekomendacjami) |
| A.8 | Wdrożenie decyzji i przegląd Jądra, wejście na `main` (KONTYNUACJA etap 3) | czeka | — | po A.7 |

**Wyjście:** scenariusz MVP 6/6 na laptopie, każde kryterium F0–F3 z wynikiem z maszyny, `main` zawiera
zweryfikowany kod. To jest „Alfa 0.1”.

## Etap B — Szwy modularności

Bez nowych funkcji dla użytkownika; każde zadanie kończy się tym, że wszystkie dotychczasowe testy są zielone.

| ID | Zadanie | Stan | Data | Uwagi |
| --- | --- | --- | --- | --- |
| B.1 | Generator typów TS z Rust (ADR 0013) + kontrola w CI | do zrobienia | — | usuwa ręczne `types*.ts`; warunek dla D |
| B.2 | Numery sekwencji zdarzeń i wznowienie „od `seq`” | do zrobienia | — | `EventHub`, `pump.rs`; zamyka też otwarte ustalenie o deltach w czasie wczytywania rozmowy |
| B.3 | Błędy strukturalne na łączu (`code`, `message`, `details`) | do zrobienia | — | |
| B.4 | Fabryka platform `platform-select` i mapa zdolności hosta | czeka | — | ADR 0018 |
| B.5 | Oznaczenie poleceń `local_only` i odpowiedniki dla klientów zdalnych | do zrobienia | — | okna dialogowe, podgląd plików, ścieżki |
| B.6 | Katalog silników i modeli z wymiarem system / architektura | do zrobienia | — | `app-models/src/data.rs` |
| B.7 | Kontrakty Brokera niezależne od systemu (ścieżki, powłoki, tożsamość) | czeka | — | ADR 0018 + przegląd właściciela; po A.7 |
| B.8 | Zmienne środowiskowe i ścieżki w binarkach Jądra przez `AppPaths` | czeka | — | obszar Jądra — przegląd właściciela |
| B.9 | Drobne: `app-bridges/src/app.rs:227` (powłoka dla systemów innych niż Windows) | do zrobienia | — | |

**Wyjście:** `cargo test --workspace` na Windows i Ubuntu zielone; korzeń kompozycji nie zawiera nazw
`platform_windows_*`; typy TS generowane.

## Etap C — Funkcje Windows po MVP

### C1 — z planu v5 (KONTYNUACJA etapy 4–8)

| ID | Zadanie | Stan | Data | Uwagi |
| --- | --- | --- | --- | --- |
| C1.1 | Jakość głosu (WER PL, przerwania, cztery głosy) | czeka | — | korpus nagrań od właściciela |
| C1.2 | Mosty CLI, klucze i dostawcy chmurowi na żywo | czeka | — | logowanie i klucze — właściciel |
| C1.3 | Sterowanie komputerem (UIA, ekran, wejście) | czeka | — | po decyzji P2-01 |
| C1.4 | Pamięć i transfer: recall@5, migracje, round-trip `.alfa` | czeka | — | recall możliwy zaraz po A.4 |
| C1.5 | Samonaprawa, wtyczki, pentest, audyt WCAG | planowane | — | |

### C2 — nowe (opis: [`04-nowe-funkcje.md`](04-nowe-funkcje.md))

| ID | Zadanie | Stan | Data | Uwagi |
| --- | --- | --- | --- | --- |
| C2.1 | Zestaw ewaluacyjny po polsku w CI (poprawność wywołań narzędzi, wierność RAG, ataki, WER, czas, VRAM) | planowane | — | można wcześniej — pomaga w A.6 |
| C2.2 | Modele na 6 GB: Bielik-Minitron-7B v3, Qwen3.5-4B; tryb wielu modeli w `llama-server` | planowane | — | po A.4; pomiar przed wpisaniem do pakietów |
| C2.3 | Wymuszanie poprawnego JSON wywołań narzędzi schematem (gramatyka) | planowane | — | |
| C2.4 | Bielik-Guard jako dodatkowy klasyfikator w Brokerze | czeka | — | obszar Jądra — decyzja właściciela |
| C2.5 | MCP: obsługa specyfikacji 2026-07-28 obok dotychczasowej | planowane | — | SDK `rmcp`; każda akcja nadal przez Brokera |
| C2.6 | Umiejętności w formacie Agent Skills (`SKILL.md`) z przypięciem sumy i przeglądem | planowane | — | |
| C2.7 | Zadania zaplanowane z budżetem uprawnień („gdy komputer włączony i bezczynny”) | planowane | — | moduły `scheduler`, `triggers` już są |
| C2.8 | Kanwa artefaktów (Markdown, HTML, SVG w izolowanej ramce) | planowane | — | ADR 0009 już to dopuszcza |
| C2.9 | Oznaczanie treści wygenerowanych przez AI (art. 50 AI Act) | planowane | — | termin prawny: 2026-12-02 — patrz [`10`](10-zgodnosc-prawna.md) |
| C2.10 | SBOM, polityka zgłaszania podatności, kontakt bezpieczeństwa (CRA) | planowane | — | przed pierwszym wydaniem publicznym |
| C2.11 | Dokumenty: wyciąganie tekstu z PDF i DOCX w Rust; skany — opcjonalny dodatek | planowane | — | |
| C2.11a | OpenRouter: uwierzytelnione sprawdzenie klucza (`GET /api/v1/key`) zamiast „nie da się sprawdzić z góry” | planowane | — | ustalenie 1 przeglądu kluczy; wymaga nowej metody adaptera |
| C2.12 | Klucze dla firm: nagłówki organizacji i projektu (OpenAI), Azure OpenAI (adres zasobu, wdrożenie, wersja API) | planowane | — | opis: [`04`](04-nowe-funkcje.md#klucze-i-konta-dla-firm) |
| C2.13 | Sieć firmowa: serwer pośredniczący i własny urząd certyfikacji dla połączeń z dostawcami | czeka | — | dotyka `lib-netguard` i polityk sieci — decyzja właściciela |
| C2.14 | Okresowe sprawdzanie kluczy w tle, wiek klucza i przypomnienie o wymianie | planowane | — | moduł `scheduler` już jest; bez zużycia tokenów (lista modeli) |
| C2.15 | Konto zapasowe: przełączenie na inne konto tego samego dostawcy, gdy pierwsze nie ma środków albo jest ograniczane | planowane | — | reguły routera; widoczne w osi czasu |
| C2.16 | Dziennik operacji na kluczach (dodanie, wymiana, wyłączenie, usunięcie, wynik testu) bez treści klucza | czeka | — | audyt jest w Jądrze — decyzja właściciela |
| C2.17 | Import kluczy ze zmiennych środowiskowych: przycisk w interfejsie i polecenie rdzenia | zrobione | 2026-10-10 | wersja 0.0.16; polecenie `accounts_import_env` i raport w „Konta i klucze API”; nieuruchamiane na prawdziwych zmiennych |

**Wyjście:** każda funkcja ma kryterium w `ACCEPTANCE.md` (progi ustala właściciel) i wynik ze sprzętu.

### C3 — kamery, rozpoznawanie osób i opis obrazu (opis: [`12-kamery-i-rozpoznawanie-osob.md`](12-kamery-i-rozpoznawanie-osob.md))

Zlecenie właściciela z 2026-10-09. Kolejność: najpierw kamery i opis obrazu (bez biometrii), potem osoby.

| ID | Zadanie | Stan | Data | Uwagi |
| --- | --- | --- | --- | --- |
| C3.1 | Port kamery w `platform-contract` (`CameraPort`) i atrapa: lista urządzeń, tryby, strumień, podłączanie na żywo | zrobione | 2026-10-10 | wersja 0.0.16; `platform-contract/src/camera.rs`, atrapa w `platform-fake` |
| C3.2 | Kamery USB na Windows (Media Foundation) | w toku | — | kod w `platform-windows-impl/src/camera/` (wersja 0.0.16); **niesprawdzone na prawdziwej kamerze** — kamera laptopa odłączona sprzętowo |
| C3.3 | Hub kamer: rejestr, kilka strumieni naraz, usypianie, klatka na żądanie, podgląd, przełącznik główny i kill-switch | zrobione | 2026-10-10 | wersja 0.0.16; moduł `camera-hub` (contract, fake, impl), testy na atrapie portu; zdarzenia magistrali i kamery sieciowe — dalej |
| C3.4 | Strona Ustawień „Kamery” i wskaźnik kamery w belce oraz pigułce głosowej | zrobione | 2026-10-10 | wersja 0.0.16; strona „Kamery”, wskaźnik w belce (pigułka głosowa — dalej); 3 style × 2 motywy |
| C3.5 | Kamery sieciowe: RTSP, wyszukiwanie ONVIF w sieci lokalnej, hasła w Credential Manager, dekoder w osobnym procesie | planowane | — | dotyka `lib-netguard` (lista adresów) — uzgodnić zakres z właścicielem |
| C3.6 | Model widzący lokalnie: wybór modelu, pomiar pamięci karty razem z modelem rozmowy na 6 GB | czeka | — | zgoda właściciela na pobranie modelu; technologia do sprawdzenia ([`11`](11-technologie-2026-10.md)) |
| C3.7 | Narzędzie `camera.look` i opis obrazu („opisz mi to, co widzisz”): droga lokalna i chmurowa za zgodą, miniatura w rozmowie | w toku | — | wersja 0.0.16: zdjęcie z kamery jako załącznik wiadomości (ręcznie, przyciskiem); narzędzie `camera.look` dla agentek wymaga zdolności Brokera — decyzja właściciela |
| C3.8 | Wykrywanie twarzy i wzorzec twarzy (ONNX Runtime na procesorze); wybór modeli po sprawdzeniu licencji | czeka | — | podpis ADR 0025; zgoda na pobranie modeli |
| C3.9 | Rejestr osób i kreator „Zapisz osobę” (zgoda, imię, trzy ujęcia); wzorce zaszyfrowane, klucz w Credential Manager | czeka | — | podpis ADR 0025; moduły `person-recognition`, `person-memory` |
| C3.10 | Rozpoznanie rozmówcy: kto jest przed kamerą, próg pewności, gość bez pamięci, narzędzie `camera.who`, powitanie po imieniu | czeka | — | po C3.8–C3.9 |
| C3.11 | Pamięć per osoba: profil, fakty ze źródłem, streszczenie każdej rozmowy; trwała po restarcie | czeka | — | podpis ADR 0025; opiera się na istniejącym module `memory` |
| C3.12 | Przypisanie tur do osoby („kto mówił”) i pamięć rozmówcy w kontekście; strona Ustawień „Osoby” | czeka | — | historia pozostaje tylko do dopisywania |
| C3.13 | Łączenie twarzy z głosem (`voice-speaker`): zgodność podnosi pewność, rozbieżność = pytanie | czeka | — | po C3.10 |
| C3.14 | Zestaw ewaluacyjny: sceny do opisu, osoby do rozróżnienia, napisy-ataki w kadrze, zdjęcie zamiast twarzy | planowane | — | zestaw ataków do `evals/` — zatwierdza właściciel |
| C3.15 | Pomiary na sprzęcie i przegląd bezpieczeństwa w świeżym kontekście; kryteria `ACC-V6-camera-…`, `-vision-…`, `-person-…` | planowane | — | progi ustala właściciel |

**Wyjście z C3:** „opisz mi to, co widzisz” działa na laptopie właściciela z co najmniej dwiema kamerami naraz;
Alfa rozpoznaje zapisane osoby po restarcie i pamięta, o czym z każdą rozmawiała; żadne kryterium prywatności
z ADR 0025 nie jest złamane (testy w prawdziwej aplikacji).

## Etap D — Host bez okna i protokół zdalny

| ID | Zadanie | Stan | Data | Uwagi |
| --- | --- | --- | --- | --- |
| D.0 | Podpis właściciela pod ADR 0019–0023 | czeka | — | bez tego nie powstaje kod |
| D.1 | `remote-gateway`: sesje klientów, ACL, ramki, wznowienie | czeka | — | po B.1–B.3, B.5 |
| D.2 | `device-pairing`: QR, klucze urządzeń, lista, odwołanie | czeka | — | |
| D.3 | `remote-relay`: przekaźnik szyfrogramu + obraz kontenera + instrukcja | czeka | — | |
| D.4 | Kanał E2E (wzorzec Noise) i test „przekaźnik nie widzi treści” | czeka | — | |
| D.5 | Dowód zgody z urządzenia sparowanego w Brokerze | czeka | — | ADR 0022; przegląd właściciela |
| D.6 | `AlfaClient` po kanale zdalnym (TypeScript) | czeka | — | trzecia implementacja obok Tauri i atrapy |
| D.7 | `host-headless`: binarka `alfa-host` | czeka | — | drugi krok — najpierw brama w procesie aplikacji |
| D.8 | Aktualizacja modelu zagrożeń i test „żadnego nasłuchu” dla nowych modułów | czeka | — | |
| D.9 | Zestaw ataków na dostęp zdalny (pisze sesja-recenzentka) | czeka | — | |

**Wyjście:** drugi komputer łączy się z hostem przez przekaźnik, prowadzi rozmowę, zatwierdza działanie; zrywane
połączenie wznawia się bez utraty zdarzeń; test dowodzi, że przekaźnik widzi tylko szyfrogram.

## Etap E — Panel WWW

| ID | Zadanie | Stan | Data | Uwagi |
| --- | --- | --- | --- | --- |
| E.1 | Układ interfejsu na wąski ekran (od 360 px) | planowane | — | można wcześniej — korzysta też okno Tauri |
| E.2 | `web-panel`: budowa PWA z tego samego kodu Svelte | planowane | — | |
| E.3 | Logowanie kluczem dostępu (passkey) i parowanie przeglądarki | planowane | — | |
| E.4 | Zgody w panelu z potwierdzeniem kluczem dostępu | planowane | — | po D.5 |
| E.5 | Powiadomienia push bez treści („obudź i pobierz”) | planowane | — | |
| E.6 | Tryb bez komputera w przeglądarce | czeka | — | ADR 0023 — kwestia przechowywania klucza API |
| E.7 | Hosting panelu i polityka bezpieczeństwa treści (CSP) | planowane | — | |

**Wyjście:** z przeglądarki w telefonie i na obcym komputerze da się zrobić wszystko, co pozwala mapa zdolności
hosta; audyt dostępności (axe) zielony na 360 px i 1280 px.

## Etap F — Android 15+

| ID | Zadanie | Stan | Data | Uwagi |
| --- | --- | --- | --- | --- |
| F.1 | Projekt Tauri mobile, budowa APK w CI | planowane | — | Tauri 2.x; `crate-type` już przygotowany |
| F.2 | `platform-android`: magazyn kluczy, biometria, powiadomienia | planowane | — | wtyczki Kotlin tam, gdzie brak gotowych |
| F.3 | Parowanie aparatem (QR) | planowane | — | |
| F.4 | Pilot 100 %: rozmowa, agentki, pliki hosta, zgody, STOP | planowane | — | po D i E.1 |
| F.5 | Głos w telefonie: mikrofon i głośnik telefonu, przetwarzanie na hoście | planowane | — | usługa pierwszoplanowa |
| F.6 | `offline-client`: rozmowa przez API bez hosta + scalenie po powrocie | czeka | — | ADR 0023 |
| F.7 | Testy na urządzeniu i w emulatorze, kryteria akceptacji | planowane | — | telefon testowy — właściciel |
| F.8 | Publikacja (APK do pobrania; sklep — decyzja właściciela) | planowane | — | |

**Wyjście:** telefon sparowany w mniej niż minutę; pełna obsługa z telefonu; po wyłączeniu komputera rozmowa
działa przez model chmurowy i po powrocie pojawia się w historii na komputerze.

## Etap G — Linux

| ID | Zadanie | Stan | Data | Uwagi |
| --- | --- | --- | --- | --- |
| G.1 | `platform-linux` — podstawa: pliki z koszem, procesy, schowek, sekrety (Secret Service) | planowane | — | po B.4 |
| G.2 | Powłoka: analizator POSIX w kontrakcie i Jądrze | czeka | — | po B.7; przegląd właściciela |
| G.3 | Jądro poza procesem: usługa użytkownika, gniazdo Unix z kontrolą UID | czeka | — | przegląd właściciela |
| G.4 | Okno zgód na Linuksie i dowód fizycznego wejścia | badawcze | — | Wayland nie daje prostego odpowiednika — patrz [`06`](06-linux.md) |
| G.5 | Audio (PipeWire) i głos | planowane | — | |
| G.6 | Skróty globalne, zasobnik, okna (portale XDG) | planowane | — | |
| G.7 | Silniki: llama.cpp i whisper.cpp dla Linuksa (CUDA, Vulkan, CPU) | planowane | — | po B.6 |
| G.8 | Pakiety: deb i rpm, potem Flatpak; test WebKitGTK na karcie NVIDIA w CI | planowane | — | |
| G.9 | Sterowanie komputerem (AT-SPI, portale) | badawcze | — | |

**Wyjście:** scenariusz MVP 6/6 na Ubuntu LTS z kartą NVIDIA; zdolności niedostępne są ukryte, nie zepsute.

## Etap H — Badawcze (bez terminu, bez zobowiązania)

| ID | Temat | Stan | Uwagi |
| --- | --- | --- | --- |
| H.1 | Mały model na telefonie (LiteRT-LM / llama.cpp, Gemma 4 E2B) | badawcze | decyzja właściciela z 2026-10-09: model lokalny tylko na komputerze — temat odłożony |
| H.2 | MCP Apps (interfejsy narzędzi w izolowanej ramce) | badawcze | po C2.5 i C2.8 |
| H.3 | Rozmowa głosowa w pełnym dupleksie po polsku | badawcze | dziś brak modelu lokalnego na 6 GB |
| H.4 | Akceleracja NPU (Windows ML / Foundry Local) | badawcze | drugi format modeli obok GGUF |
| H.5 | Alfa jako serwer MCP / agent ACP dla edytorów | badawcze | |
| H.6 | Protokół A2A | badawcze | dziś mało przydatny w programie dla jednej osoby |
| H.7 | Synchronizacja pełna (CRDT: Loro / Automerge) | badawcze | tylko jeśli tryb pilota okaże się niewystarczający |

## Decyzje właściciela, które blokują zadania

| Decyzja | Blokuje |
| --- | --- |
| Zgoda na pobranie pakietu modeli (ok. 6 GB) | A.4, A.5, A.6, C1.4, C2.2 |
| `gh auth login` i wypchnięcie commitów | A.0, A.8 |
| Ustalenia bezpieczeństwa (B-…, S-…, C-…, R-…, P-…) | A.7, A.8, B.7 |
| Podpis pod ADR 0017–0018 | B.4, B.7, G |
| Podpis pod ADR 0019–0023 | D, E, F |
| ADR 0024 — licencja silnika mowy | C1.1 (wybór głosu), wydanie publiczne |
| ADR 0025 — kamery i biometria twarzy | C3.8–C3.13 |
| Czy obraz z kamery może iść do modelu w chmurze | C3.7 (droga chmurowa) |
| Zgoda na pobranie modelu widzącego i modeli twarzy | C3.6, C3.8 |
| Progi nowych kryteriów w `ACCEPTANCE.md` | wyjście z C, D, E, F, G |

## Dziennik roadmapy

| Data | Zmiana |
| --- | --- |
| 2026-10-10 | Wersja 0.0.16 (noc bez właściciela): kamery USB — port, hub, strona „Kamery”, wskaźnik, zdjęcie jako załącznik (C3.1–C3.4, część C3.7); import kluczy ze zmiennych (C2.17); pakiet decyzji A.7; podsumowanie i dalsze kroki w dokumencie [`13`](13-podsumowanie-i-dalsze-kroki.md). |
| 2026-10-09 | Zlecenie właściciela: grupa C3 — kamery (USB, IP, Wi-Fi, kilka naraz), rozpoznawanie zapisanych osób z pamięcią długoterminową, opis obrazu na polecenie. Dokument [`12`](12-kamery-i-rozpoznawanie-osob.md), cztery szkice SPEC, projekt ADR 0025. Tylko plan — kod nie powstał. |
| 2026-10-09 | Wersja 0.0.8: naprawy po próbie na laptopie (A.3e) — rozmowa przez API, kolejki modeli, głos. Właściciel pobrał silniki głosu i model lokalny: pierwsza rozmowa lokalna i nasłuch działają (A.4/A.5 w toku). |
| 2026-10-09 | Wersja 0.0.7: dodawanie klucza API od nowa (A.3d); zadania C2.12–C2.17 (klucze dla firm) dopisane do planu. |
| 2026-10-09 | Wersja 0.0.6: naprawy po przeglądzie zmian 0.0.1–0.0.5 (A.3c); numer wersji zmienia skrypt. |
| 2026-10-09 | Wersja 0.0.4: listy wyboru i inne kontrolki rysowane przez Alfę (A.3a). |
| 2026-10-09 | Powstanie planu v6. Etapy A.1–A.3 wpisane jako zrobione (wersje 0.0.2 i 0.0.3). Cztery decyzje właściciela o dostępie zdalnym. |
