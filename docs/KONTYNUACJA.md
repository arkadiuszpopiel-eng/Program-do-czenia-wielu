# Alfa — ocena zdolności do kontynuacji i plan dalszych prac

Stan na 2026-10-07 (wieczór), laptop RTX 4050; decyzje właściciela i postęp Etapu 0 dopisane 2026-10-08.
Autor: Claude (Opus 5.5), sesja lokalna na Windows.
**Aktualizacja 2026-10-09:** etapy 0–8 z tego dokumentu są etapem A i częścią C1 planu v6 —
[`plan/03-roadmapa.md`](plan/03-roadmapa.md); stan wykonania: [`plan/00-stan-wykonania.md`](plan/00-stan-wykonania.md).
Liczby w rozdz. 2 to stan z 2026-10-07; aktualne podaje [`CATALOG.md`](CATALOG.md) (241 crate'ów, 2514 testów, 16 ADR-ów
i 8 projektów).
Źródła: repozytorium `alfa` (gałąź `ccr-af4b63c6-3fyzaj`, commit `75fee0d`), `docs/STATUS.md`, przeglądy w
`docs/reviews/`, dzisiejsze logi Alfy z `%LOCALAPPDATA%\Alfa\logs` oraz bramki uruchomione w tej sesji.

## 1. Werdykt

**Tak — jestem w stanie dalej rozwijać ten program i prowadzić prace nad nim.** Kod jest czytelnie podzielony,
ma kontrakty, atrapy i testy, a dokumentacja (`AGENTS.md`, `STATUS.md`, SPEC-i, ADR-y) pozwala wejść w dowolny
moduł bez znajomości historii. W tej sesji zbudowałem i przetestowałem fragment workspace'u na tej maszynie.

Trzy zastrzeżenia, od których zależy, czy to „tak" będzie coś warte:

1. **Wąskim gardłem nie jest już pisanie kodu, tylko sprawdzanie go na prawdziwym sprzęcie.** Z 108 kryteriów
   akceptacji 38 jest zielonych — wszystkie na atrapach i w CI. 57 czeka na pomiar na sprzęcie, korpusie albo
   kluczu, 11 nie ma zestawu lub implementacji, 2 czekają na Twoją akceptację. Pierwszy prawdziwy test (dziś)
   od razu znalazł błędy, których CI nie widziało.
2. **Części pracy nie zrobię z definicji**: nagrań, odsłuchów, logowania do CLI, zgód UAC, fizycznych kliknięć
   w oknie Brokera i — co najważniejsze — zatwierdzania zmian Jądra bezpieczeństwa. To ostatnie jest celowe:
   reguły projektu zabraniają, żeby AI zatwierdzała własne zmiany w zabezpieczeniach.
3. **„Prowadzenie" oznacza ciągłość przez pliki, nie przez pamięć.** Nie działam między sesjami. Stan projektu
   musi żyć w `docs/STATUS.md` i `memory/`, a każdą sesję zaczynasz Ty.

## 2. Co sprawdziłem w tej sesji

### Rozmiar i dojrzałość

| Miara                          | Wartość                                                                                             |
| ------------------------------ | --------------------------------------------------------------------------------------------------- |
| Kod Rust                       | ok. 373 tys. linii w 2065 plikach, 242 crate'y (ok. 100 modułów)                                    |
| Kod interfejsu (Svelte 5 + TS) | ok. 46 tys. linii                                                                                   |
| Testy Rust (`#[test]`)         | 2537                                                                                                |
| Historia                       | 128 commitów w 8 dni (30.09 – 7.10), wszystkie autorstwa „Claude"                                   |
| Dokumentacja                   | plan v5 (120 KB), 15 ADR-ów, 77 SPEC-ów modułów, 3 przeglądy bezpieczeństwa, przewodnik użytkownika |
| Limity z `AGENTS.md`           | 3 pliki Rust przekraczają 400 linii (406–436); trafienia `TODO` to tylko wartości zastępcze w danych testowych |

Macierz akceptacji z `docs/STATUS.md` (stan z 6.10):

| Fala                     | ✅ CI/atrapy | 🟡 czeka na sprzęt/korpus/klucz | ⛔ brak | ⚖️ decyzja |
| ------------------------ | ------------ | ------------------------------- | ------- | ---------- |
| F0 Fundament             | 4            | 13                              | 0       | 2          |
| F1 Czat                  | 7            | 6                               | 0       | 0          |
| F2 Głos                  | 2            | 12                              | 0       | 0          |
| F3 Bezpieczeństwo (MVP)  | 6            | 5                               | 2       | 0          |
| F4 Mosty i MCP           | 3            | 5                               | 0       | 0          |
| F5 Agentki + głos rozsz. | 5            | 7                               | 0       | 0          |
| F6 Computer use          | 1            | 3                               | 4       | 0          |
| F7 Pamięć                | 5            | 2                               | 1       | 0          |
| F8 Samonaprawa           | 5            | 0                               | 1       | 0          |
| F9 Dopieszczenie         | 0            | 4                               | 3       | 0          |
| **Razem**                | **38**       | **57**                          | **11**  | **2**      |

### Bramki uruchomione tutaj

| Bramka                                        | Kopia `E:\…\alfa` (ta sesja)              | Kopia `D:\alfa`  |
| --------------------------------------------- | ----------------------------------------- | ---------------- |
| `cargo test` — 7 crate'ów rdzenia¹            | **122 testy, 0 porażek** (7 min od zera)  | nie uruchamiałem |
| vitest (interfejs)                            | nie startuje (`ERR_MODULE_NOT_FOUND`)     | **219/219**      |
| svelte-check                                  | 291 błędów (zepsute `node_modules`)       | **0 błędów**     |
| `cargo fmt --all -- --check`                  | błąd systemu 206 (za długa linia poleceń) | ten sam błąd     |
| `cargo deny`, `cargo clippy`, pełny workspace | nie uruchomione²                          | nie uruchomione  |
| Stan CI na GitHubie                           | niesprawdzony — `gh` nie jest zalogowany  |                  |

¹ `router-impl`, `safety-broker-impl`, `undo-journal-impl`, `tools-fs-impl`, `risk-classifier-contract`,
`sessions-impl`, `memory-impl`. Potwierdziłem po nazwach, że przeszły testy, na które powołuje się STATUS
(m.in. `zero_successful_attacks`, `kill_switch_p95_under_200ms`, `chain_of_10k_events_verifies_and_survives_reopen`,
`random_sequences_are_fully_undoable`, `tainted_egress_never_proceeds_even_on_l4`,
`fifty_trials_5xx_and_timeout_switch_within_2s_without_loss`). To próbka, nie cały workspace.

² `cargo-deny` nie jest zainstalowany. Pełne `cargo test --workspace` potrzebuje według komentarza w `Cargo.toml`
ponad 25 GB na wyniki kompilacji, a na dyskach D: i E: jest po ok. 30 GB wolnego (94 % zajęte).

### Co pokazał dzisiejszy test na laptopie (z logów Alfy)

- Aplikacja startuje, moduły wstają w poprawnej kolejności, wykrywa sprzęt (`laptop-cuda`, profil głosu D).
- **Model lokalny ani razu nie odpowiedział.** Pobrany jest Bielik 1.5B, Whisper turbo i embedder e5-small, ale
  katalog `sidecars` jest pusty — brakuje `llama-server.exe`. Router pięć razy wybrał model lokalny, katalog
  `model_calls` jest pusty. W logu co 15 minut wraca ostrzeżenie „start na GPU nieudany — fallback na CPU",
  choć przyczyną jest brak pliku, a nie karta.
- Skrót `Ctrl+Alt+Space` (Szybkie pytanie) jest zajęty przez inny program — rejestracja pada przy każdym starcie.
- Czytanie na głos odmówiło (`unsupported`) — brak silnika TTS.
- Tryb deweloperski: Broker działa w procesie aplikacji, watchdog nie działa. To oczekiwane, ale znaczy, że
  izolacja Brokera nie była jeszcze sprawdzona na żadnej maszynie.
- Trzy ostatnie commity dnia to poprawki po tym teście (kreator kluczy, uprawnienia okna — „pakiety działały
  tylko w atrapie", zrzuty ekranu). To dobry sygnał co do tempa napraw i zły co do wiarygodności samych atrap.

## 3. Czego nie wiedziałem przed tą sesją, a ma znaczenie

1. **Były trzy kopie kodu na tym samym commicie**: `D:\alfa` (tam Alfa była budowana i uruchamiana), kopia na
   dysku przenośnym `E:\PROGRAMY W CZASIE PRAC\Alfa AI Program\alfa` i migawka ZIP z GitHuba obok niej. Kopia
   na E: miała zepsute `node_modules` (kopiowanie zamieniło dowiązania pnpm na zwykłe katalogi). Decyzją
   właściciela kopią roboczą jest E: — patrz rozdz. 8.
   **Stan po porządkach 2026-10-09:** została jedna kopia — robocza na E:. `D:\alfa`, `D:\alfa-narzedzia` i migawka ZIP
   trafiły do Kosza (za zgodą właściciela, po sprawdzeniu, że nie zawierają niczego spoza GitHuba).
2. **README kłamie o stanie projektu**: „Faza planowania… Nie ma jeszcze kodu".
3. **Cały kod leży na jednej gałęzi, 127 commitów przed `main`.** `main` ma jeden pusty commit. Reguła „jedna
   sesja = jeden moduł = jeden worktree = jedna karta" i przegląd drugiego modelu nie są widoczne w historii:
   wszystkie commity ma jeden autor, a przeglądy istnieją tylko jako dokumenty w `docs/reviews/`. Nie potrafię
   z repozytorium potwierdzić, że GPT-6 Sol cokolwiek recenzował.
4. **Zmiany Jądra nie przeszły wymaganego przeglądu człowieka** (bramka #7). Według własnych reguł projektu
   nie mogą trafić na `main`, dopóki ich nie zatwierdzisz.
5. **Otwarte decyzje bezpieczeństwa**: m.in. P-03 (powłoka agentek startuje z uprawnieniami użytkownika zamiast
   obniżonych, ocena 7,5), P-08 (procesy Jądra bez weryfikacji podpisu), P3-01 (Broker po restarcie traci stan
   bezpieczeństwa), P3-04 (pliki wykonywalne sidecarów bez przypiętych sum), P2-01 (strażnik okien WebView2 —
   warunek włączenia computer use).
6. **Ryzyko dla maszyny docelowej (AMD, bez CUDA).** W wydaniach whisper.cpp nie ma wersji Vulkan
   `whisper-server`, więc na desktopie rozpoznawanie mowy liczy procesor. Próba generalna na 4 rdzeniach dała
   53 s na 2,1 s mowy modelem `small`. Desktop jest szybszy, ale to wymaga pomiaru, zanim głos na baseline
   uznamy za wykonalny w budżecie 2 s.
7. **Lokalne bramki nie działają na Windows w całości**: `cargo fmt --all` pada na długości linii poleceń
   przy 242 crate'ach, a Git Bash podstawia własnego Perla przed Strawberry (budowa OpenSSL pada, dopóki nie
   poprawi się `PATH`).
8. `docs/STATUS.md` ma datę 6.10 i nie zawiera ustaleń z dzisiejszego testu. W przewodniku laptop jest opisany
   jako i7-13700H, faktycznie to i7-13620H.

## 4. Co mogę robić, a czego nie

| Obszar                                                                                               | Ja                                                                                     | Ty                                                |
| ---------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | ------------------------------------------------- |
| Kod, testy, refaktoryzacja, dokumentacja, CI                                                         | w całości                                                                              | —                                                 |
| Budowa i uruchamianie Alfy na tym laptopie, czytanie logów                                           | tak                                                                                    | —                                                 |
| Pomiary na laptopie: tok/s, czas do pierwszego tokenu, RAM, VRAM, start, kill-switch, recall pamięci | tak (to jest maszyna „laptop CUDA" z planu)                                            | podłączony zasilacz                               |
| Sterowanie prawdziwą aplikacją w testach (build `e2e`, Playwright przez CDP)                         | tak                                                                                    | —                                                 |
| Pomiary na desktopie AMD                                                                             | tylko jeśli sesja ruszy na desktopie                                                   | uruchomienie sesji tam                            |
| Instalator NSIS, tryb przenośny Brokera                                                              | budowa i testy automatyczne                                                            | kliknięcia w oknie Brokera (z założenia fizyczne) |
| Usługa Brokera, helper `uiAccess`, klucz minisign, certyfikat                                        | przygotowanie instrukcji i skryptów                                                    | UAC, wygenerowanie i przechowanie kluczy          |
| Korpus głosu, odsłuchy, testy akustyczne                                                             | runnery, protokoły, liczenie wyników                                                   | nagrania i oceny                                  |
| Mosty `claude` / `codex`, klucze API                                                                 | kod, nagrywanie fixture'ów po zalogowaniu                                              | logowanie, podanie kluczy w Alfie                 |
| Przegląd zmian Jądra i decyzje bezpieczeństwa                                                        | pakiet do przeglądu, rekomendacje, wdrożenie decyzji                                   | **decyzja i akceptacja**                          |
| Niezależny przegląd kodu                                                                             | świeży kontekst tego samego modelu — to nie jest niezależność, o którą chodzi w planie | uruchomienie Codexa, jeśli ma recenzować          |
| Zamrażanie zestawów `evals/`, zmiany `AGENTS.md` i progów                                            | szkice                                                                                 | akceptacja                                        |

## 5. Zasada na dalszą część projektu

**Najpierw stabilizacja MVP, potem rozbudowa.** Projekt powstał wszerz: wszystkie fale F0–F9 mają kod, żadna nie
jest zamknięta na sprzęcie. Dopisywanie kolejnych funkcji teraz powiększa niesprawdzony obszar. Proponuję:

- Do zamknięcia Etapu 3 nie zaczynamy nowych modułów ani funkcji — tylko naprawy, pomiary i brakujące zestawy.
- Kryterium uznajemy za spełnione dopiero po przebiegu na prawdziwej maszynie. W `STATUS.md` rozdzielamy
  „✅ CI" i „✅ sprzęt (laptop / desktop)".
- Każdy błąd znaleziony na sprzęcie dostaje test, który by go złapał — najlepiej w teście prawdziwej aplikacji
  (`e2e-app`), bo dzisiejsze błędy przeszły właśnie przez atrapy.
- Jedna sesja = jeden cel z listy niżej. Na końcu sesji aktualizuję `docs/STATUS.md` i `memory/`.

## 6. Plan

Etapy są w kolejności wykonania. „Wyjście" to warunek, po którym etap uznajemy za zamknięty. Liczby sesji to
szacunek, nie zobowiązanie.

### Etap 0 — Porządek w środowisku i repozytorium (1–2 sesje)

| # | Zadanie | Stan (aktualizacja 2026-10-09) |
| - | ------- | ------------------ |
| 0.1 | Jedna kopia robocza: dysk przenośny E:. | **zrobione** — `node_modules` naprawione, narzędzia (Rust 1.94, Node.js + pnpm, Strawberry Perl, magazyn pnpm) skopiowane do `alfa-narzedzia`, `scripts/portable-env.ps1` i `.sh` |
| 0.2 | Zwolnić miejsce na E: (26 GB wolnego, 95 % zajęte). Kandydaci: skopiowane z D: wyniki kompilacji powłoki (36 GB, do odtworzenia budową), migawka ZIP z GitHuba, nieużywana kopia `D:\alfa`. | **zrobione 2026-10-09** (zgoda właściciela): stare wyniki kompilacji usunięte `cargo clean` (36 GB na E:, 37 GB na D: oraz 37 GB ukrytej kopii repozytorium w magazynie pnpm), stare katalogi i migawka w Koszu; E: 107 GB wolnego, D: 59 GB |
| 0.3 | `gh auth login`, żeby sesja widziała CI, PR-y i gałęzie Dependabota. | czeka na właściciela |
| 0.4 | Bramki lokalne na Windows jednym poleceniem. | **zrobione** — `scripts/gates.ps1`; poprawione `check-deps.sh` (CRLF z jq), `check-css.mjs` i `check-svelte5.mjs` (ścieżki Windows i ze spacjami). Brakuje `cargo-deny` |
| 0.5 | Czyste drzewo git po budowie. | **zrobione** dla `permissions/autogenerated/` (`.gitignore`); `src-tauri/Cargo.lock` do sprawdzenia przy następnej budowie powłoki |
| 0.6 | README i STATUS zgodne ze stanem. | **zrobione** — README, `docs/README.md` (mapa), dopisek w STATUS; podział „✅ CI / ✅ sprzęt” wejdzie z pierwszymi pomiarami |
| 0.7 | Przejrzeć 19 gałęzi Dependabota i 2 gałęzie Copilota. | czeka na 0.3 |
| 0.8 | Katalog modułów i crate'ów. | **zrobione** — `docs/CATALOG.md` generowany przez `scripts/catalog.mjs`, z listą niespójności |
| 0.9 | Reguła recenzji zgodna z praktyką (bez Codexa). | **zrobione** — ADR 0016, `AGENTS.md`, `AI_WORKFLOW.md`, odsyłacz w PLAN §4 |
| 0.10 | Uporządkować niespójności z katalogu: 62 crate'y bez README, 3 pliki ponad 400 linii, 7 crate'ów `-impl` bez `module.toml`, SPEC dla `example-module`. | **zrobione 2026-10-08/09** poza dwiema pozycjami: `core-bus-impl` bez `module.toml` (Jądro — decyzja właściciela) i brak SPEC dla `example-module` |
| 0.11 | Pierwsza pełna budowa powłoki na E: w środowisku przenośnym i zielony komplet `gates.ps1 -Full`. | częściowo: powłoka zbudowana na E: (wersje 0.0.2 i 0.0.3), bramki szybkie zielone; `gates.ps1 -Full` (clippy, cały workspace, cargo-deny) nieuruchomione |

Wyjście: jedna kopia robocza, komplet bramek lokalnych przechodzi jednym poleceniem, drzewo git czyste po budowie,
STATUS i README zgodne z rzeczywistością, katalog bez niespójności.

### Etap 1 — Pierwsza prawdziwa rozmowa na laptopie (2–3 sesje)

Stan na 2026-10-09: zadania 1.1 i 1.2 zrobione w kodzie i w testach (commit `e594e61`), niesprawdzone jeszcze
w działającej aplikacji; 1.3 — ostrzeżenie o zajętym skrócie widać w aplikacji; 1.4 czeka na decyzję R-05 i zgodę
na pobranie pakietu (ok. 6 GB); 1.5–1.6 częściowo — testy prawdziwej aplikacji `e2e-app` 11/11, bez odpowiedzi modelu
i bez głosu. Poza planem doszły poprawki skalowania interfejsu — opis w `docs/STATUS.md` (dopisek).

| #   | Zadanie                                                                                                                                                                                                      | Kryteria                      |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------- |
| 1.1 | Model bez silnika: pobranie modelu ma prowadzić do pobrania pasującego `llama-server` (pakiet albo wyraźny krok), a wiadomość bez silnika ma dawać konkretny błąd w rozmowie z przyciskiem „Pobierz silnik". | F1-03                         |
| 1.2 | Poprawić diagnostykę: brak pliku wykonywalnego to nie „fallback na CPU"; bez ponawiania co 15 minut, gdy przyczyna jest trwała.                                                                              | —                             |
| 1.3 | Zajęty skrót globalny: widoczne ostrzeżenie i propozycja innego skrótu zamiast cichego wpisu w logu.                                                                                                         | F1-12                         |
| 1.4 | Zainstalować silniki (CUDA + CPU na zapas, Piper, Silero) przez menedżer, przypiąć sumy SHA-256 zgodne z tabelą z próby generalnej.                                                                          | P3-04, pkt 11 listy człowieka |
| 1.5 | Przejść części 4–9 listy `11-pierwszy-test-na-pc.md` — ja automatem przez build `e2e` i logi, Ty tylko to, co wymaga głosu i uszu.                                                                           | F1-03, F3-01, F3-02           |
| 1.6 | Każdy znaleziony błąd → test w `e2e-app` (prawdziwa aplikacja). Rozszerzyć próbę generalną o scenariusz „model + silnik + odpowiedź".                                                                        | —                             |

Wyjście: rozmowa tekstowa z Bielikiem na karcie, zmierzone tok/s i czas do pierwszego tokenu; odpowiedź czytana
na głos; „Cofnij" po zadaniu na plikach; `Ctrl+Shift+F12` zatrzymuje generowanie. Wyniki wpisane do STATUS.

### Etap 2 — Pomiary MVP na sprzęcie, F0–F3 (4–6 sesji + Twoje bramki)

| #    | Zadanie                                                                                                                                                                       | Kryteria                   | Kto                                  |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------- | ------------------------------------ |
| 2.1  | Spike f: RAM drzewa procesów (1 i 3 okna) → budżety do ADR.                                                                                                                   | F0-11, F1-08, F5-12        | ja (laptop), desktop osobno          |
| 2.2  | Spike h: whisper CUDA 1 h, tok/s modeli 1.5B/4.5B, RTF TTS → progi.                                                                                                           | F0-12, F0-13, F1-03, F3-07 | ja                                   |
| 2.3  | Budżety UI ze śladu CDP (composer, 1000 wiadomości, panele) i Private WS po 500 wiadomościach.                                                                                | F1-06, F1-09               | ja                                   |
| 2.4  | Eval narzędzi fs/shell na lokalnym modelu (35 zadań). Wynik pokaże, czy mały model w ogóle nadaje się na agentkę — to rozstrzyga kształt MVP „bez kluczy".                    | F3-07                      | ja; próg ustalasz Ty                 |
| 2.5  | Instalator NSIS, tryb przenośny Brokera, watchdog jako osobny proces; kill-switch z audio i Job Objects pod obciążeniem, 50 prób.                                             | F3-01, F3-10, luka nr 1    | ja; kliknięcia w oknie Brokera — Ty  |
| 2.6  | Test wstrzykniętych kliknięć do Broker-UI na żywym pulpicie (`kernel_windows.rs`).                                                                                            | F0-16, F3-04               | ja uruchamiam, Ty nie dotykasz myszy |
| 2.7  | Napisać `evals/F3/mvp-scenario.md` i przejść scenariusz MVP bez kluczy.                                                                                                       | F3-12                      | ja + Ty                              |
| 2.8  | Zestaw red-team prompt injection (≥ 100). Powinien go napisać ktoś inny niż autor obron — najlepiej Codex; inaczej ja w świeżym kontekście, z zaznaczeniem tego ograniczenia. | F3-05                      | do decyzji                           |
| 2.9  | Te same pomiary na desktopie AMD (Vulkan), w tym STT na procesorze.                                                                                                           | F0-03, F2-01, ryzyko nr 6  | sesja na desktopie                   |
| 2.10 | Korpus głosu, odsłuchy, akustyka.                                                                                                                                             | F0-08…10, F2-03…10, F2-14  | Ty nagrywasz, ja liczę               |

Wyjście: każde kryterium F0–F3 ma wynik z prawdziwej maszyny (spełnione albo niespełnione z liczbą), scenariusz
MVP 6/6 na laptopie. Jeśli 2.4 albo 2.9 wypadną źle, wracamy do planu z decyzją (większy model, inny silnik STT,
zmiana baseline) zamiast iść dalej.

### Etap 3 — Dług bezpieczeństwa i wejście na `main` (3–5 sesji + Twój przegląd)

| #   | Zadanie                                                                                                                                                                                                                                                                                 | Kto                        |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------- |
| 3.1 | Pakiet do przeglądu Jądra: dla `safety-broker`, `broker-ui`, `watchdog`, `updater`, `app-safety`, `core-*` — co robi, granice zaufania, lista poprawek SR/SR2/SR3 z testami, pytania kontrolne. Uporządkowane od najwyższego ryzyka, tak żeby dało się to przeczytać w kilka wieczorów. | ja                         |
| 3.2 | Decyzje: P-01, P-03, P-05, P-08, P-09, P2-01, P2-03, P2-06, P2-07, P2-10, P3-01…P3-04. Do każdej przygotuję opcje, koszt i rekomendację.                                                                                                                                                | Ty                         |
| 3.3 | Wdrożenie decyzji z testami regresji. Moja kolejność: P3-01 (trwały stan Brokera — dziś restart otwiera zabezpieczenia), P-03 (obniżone uprawnienia powłoki), P3-04 (przypięte sumy sidecarów), P2-01 (przed computer use).                                                             | ja                         |
| 3.4 | Akceptacja ADR-ów 1–15 i makiet 1–3.                                                                                                                                                                                                                                                    | Ty                         |
| 3.5 | Strategia gałęzi: scalić obecną gałąź do `main` po przeglądzie Jądra, potem krótkie gałęzie na zadanie i PR-y z CI. Urealnić `AGENTS.md`: jeśli drugi model nie recenzuje, reguła ma to mówić wprost (zmianę zatwierdzasz Ty).                                                          | Ty decydujesz, ja wykonuję |
| 3.6 | Zamrozić gotowe zestawy `evals/` (F2 test, F3 tools, F5, F8) z hashami.                                                                                                                                                                                                                 | ja szkic, Ty akceptujesz   |

Wyjście: `main` zawiera zweryfikowane MVP, żadne ustalenie z przeglądów nie jest otwarte bez decyzji, zestawy MVP
zamrożone. **To jest koniec MVP w rozumieniu planu.**

### Etap 4 — Jakość głosu (po korpusie; 3–4 sesje)

WER PL, komendy stop/anuluj, backchannel, fałszywe przerwania, prefiks usłyszany, intencje przerwań (F2-03…09);
dobór modelu STT dla ścieżki bez CUDA; odrębność czterech głosów (F2-10); budżet VRAM z LLM + STT + TTS (F2-13);
potem słowa wywoławcze i weryfikacja mówcy (F5-05…08), dyktowanie i czytanie w 5 aplikacjach (F5-10, F5-11).

### Etap 5 — Mosty CLI, klucze i dostawcy chmurowi (2–3 sesje po Twoim logowaniu)

Spike b na prawdziwych `claude` i `codex` (F0-05…07), nagranie fixture'ów (F4-08), postęp i anulowanie (F4-01, F4-02),
prośby o uprawnienia w oknie Brokera (F4-03), monitor odczytów tokenów (F4-04); adaptery API na żywo (F1-02),
profile głosu B/C (F2-02), spike g i domknięcie ADR 0006.

### Etap 6 — Computer use (4–6 sesji; dopiero po P2-01)

Helper `uiAccess` (F6-07), runner zestawu 60 zadań i VM (F6-01), macierz aplikacja × trasa (F6-03), 100 scenariuszy
deny-list (F6-05), 50 prób zakazu sterowania oknami Alfy (F6-06), Przejmij/Pauza/Stop (F6-08), benchmark
zewnętrzny, jeśli jest dostępny (F6-02). Wymaga „mózgu": klucza API albo mostu.

### Etap 7 — Pamięć i transfer (2–3 sesje)

recall@5 na prawdziwym e5-small — model jest już pobrany na laptopie, więc to mogę zmierzyć od razu po Etapie 1
(F7-02); runner migracji w CI i powrót do starszej wersji (F7-08); pełny round-trip `.alfa` między maszynami
(F1-10, F7-06); harmonogram kopii zapasowych w aplikacji (F7-07).

### Etap 8 — Samonaprawa, wtyczki, dopieszczenie (3–5 sesji)

Zamrożony zestaw złośliwych wtyczek (F8-05), izolacja runnera holdoutu (P2-10), pentest według listy 53 punktów
(F9-01), powtórka red-teamu na całości (F9-07), skrypt pokrycia dokumentacji dla 60 manifestów (F9-06), pełny audyt
WCAG 2.2 AA (F9-05), bateria (F9-04), własny pasek tytułu z Mica i generator typów TS (ADR 13).

### Kolejność i zależności

```
Etap 0 ─▶ Etap 1 ─▶ Etap 2 ─▶ Etap 3 ══ koniec MVP ══▶ Etap 5 ─▶ Etap 6
                      │                                  Etap 7 (F7-02 możliwe już po Etapie 1)
                      └─ korpus (Ty) ─▶ Etap 4           Etap 8
```

Etapy 4, 5, 7 i 8 są od siebie niezależne — kolejność po MVP ustalasz według tego, czego najbardziej potrzebujesz.

## 7. Ryzyka

| Ryzyko                                                              | Skutek                                                     | Co z tym robimy                                                                        |
| ------------------------------------------------------------------- | ---------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Atrapy przepuszczają błędy integracji (potwierdzone dziś)           | „Zielone" kryteria okazują się czerwone na sprzęcie        | Etap 1–2 przed nowymi funkcjami; każdy błąd → test na prawdziwej aplikacji             |
| Mały model lokalny nie radzi sobie z narzędziami                    | MVP „bez kluczy" nie spełni F3-07 i F3-12                  | Pomiar w 2.4 wcześnie; decyzja o progu albo o większym modelu                          |
| STT na desktopie AMD za wolne                                       | Głos na baseline poza budżetem                             | Pomiar w 2.9; opcje: własna budowa whisper.cpp z Vulkanem, mniejszy model, inny silnik |
| Przegląd Jądra to dużo czytania dla jednej osoby                    | Zator przed `main`                                         | Pakiet 3.1 uporządkowany według ryzyka, w porcjach                                     |
| Brak faktycznie niezależnego recenzenta                             | Błędy wspólne dla autora i recenzenta                      | Codex do przeglądów Jądra i zestawów red-team albo jawne przyjęcie ograniczenia        |
| Zakres planu v5 jest bardzo szeroki jak na program dla jednej osoby | Rozproszenie, nic nie jest skończone                       | Zasada z rozdz. 5; po MVP wybierasz jeden etap naraz                                   |
| Mało miejsca na dyskach                                             | Budowa pada (`os error 112`), brak pełnych testów lokalnie | Zadanie 0.2                                                                            |
| Ciągłość między sesjami                                             | Powtarzanie ustaleń, rozjazd dokumentów z kodem            | STATUS i `memory/` aktualizowane na końcu każdej sesji                                 |

## 8. Decyzje właściciela

Podjęte 2026-10-07/08:

| Pytanie | Decyzja |
| ------- | ------- |
| Kopia robocza | Dysk przenośny **E:** (`E:\PROGRAMY W CZASIE PRAC\Alfa AI Program\alfa`); narzędzia obok w `alfa-narzedzia` |
| Kolejność prac | Najpierw porządek i doprowadzenie obecnego etapu do wysokiego poziomu; bez nowych funkcji |
| Recenzja przez Codexa | Nie — zapis w dokumentach był błędem (ADR 0016) |

Otwarte:

1. ~~Miejsce na E:~~ — rozwiązane 2026-10-09 (107 GB wolnego).
2. **`gh auth login`** — bez tego sesja nie widzi CI, PR-ów ani gałęzi Dependabota i nie wypchnie commitów.
3. ~~Commit zmian porządkowych~~ — zatwierdzone lokalnie.
4. **Zgoda na pobranie pakietu modeli (ok. 6 GB)** — bez niej nie da się sprawdzić rozmowy z modelem ani głosu.
5. **Ustalenia bezpieczeństwa** czekające na decyzję — lista w [`plan/03-roadmapa.md`](plan/03-roadmapa.md),
   „Decyzje właściciela, które blokują zadania”.
6. **Projekty ADR 0017–0024** (plan v6) — do podpisu.

Podjęte 2026-10-09 (plan v6): dostęp zdalny przez przekaźnik z szyfrowaniem end-to-end; zgody zdalne dla wszystkich
poziomów ryzyka; telefon i panel WWW jako pilot ze 100 % funkcji, bez komputera — rozmowa przez model chmurowy;
plan v6 obok planu v5. Szczegóły: [`plan/01-wizja-i-zasady.md`](plan/01-wizja-i-zasady.md).

## 9. Trzy najbliższe sesje

1. **Porządek** (Etap 0): bramki lokalne jednym poleceniem, czyste drzewo po budowie, README i STATUS zgodne
   ze stanem, pamięć projektu wypełniona.
2. **Model odpowiada** (1.1–1.4): naprawa ścieżki „model bez silnika", instalacja silników z przypiętymi sumami,
   pierwsza odpowiedź Bielika na karcie z pomiarem.
3. **Lista kontrolna automatem** (1.5–1.6): części 4–9 przewodnika przez build `e2e`, nowe testy prawdziwej
   aplikacji, wyniki w STATUS.
