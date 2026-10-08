# STATUS.md — macierz akceptacji fal F0–F9

Stan na **2026-10-06**, gałąź `ccr-af4b63c6-3fyzaj` (fale 3–6: narzędzia systemowe, podział `app-core`, dziennik diagnostyczny, próba generalna na runnerze Windows). Kryteria i progi:
`docs/ACCEPTANCE.md` (ten plik ich nie zmienia). Dowody sprawdzone w repo (`grep` po nazwach testów i plików);
liczby z opisów commitów i README zestawów `evals/`.

## Dopisek 2026-10-08 — pierwszy test na laptopie i porządki

Macierz niżej ma stan z 2026-10-06 i opisuje wyniki z CI i atrap. Żadne kryterium nie jest jeszcze potwierdzone
na prawdziwej maszynie. Kolejność dalszych prac: [`KONTYNUACJA.md`](KONTYNUACJA.md).

Ustalenia z testu na laptopie (i7-13620H, RTX 4050 6 GB; logi z `%LOCALAPPDATA%\Alfa\logs`, 2026-10-07):

| Ustalenie | Kryterium | Stan |
| --- | --- | --- |
| Model lokalny ani razu nie odpowiedział: pobrany Bielik 1.5B, brak `llama-server.exe` (pusty `sidecars`), router 5 razy wybrał model lokalny, `model_calls` puste; rozmowa nie pokazuje, że brakuje silnika | F1-03, F3-12 | do naprawy (KONTYNUACJA 1.1) |
| Log „start na GPU nieudany — fallback na CPU” co 15 min, gdy przyczyną jest brak pliku wykonywalnego (`providers-local-impl/src/sidecar.rs`) | — | do naprawy (1.2) |
| Skrót `Ctrl+Alt+Space` zajęty przez inny program — rejestracja pada przy każdym starcie bez komunikatu w interfejsie | F1-12 | do naprawy (1.3) |
| Czytanie na głos odmawia (`unsupported`) — brak silnika TTS | F2-01 | po instalacji silników (1.4) |
| Start aplikacji, kolejność modułów, wykrycie sprzętu (`laptop-cuda`, profil głosu D), pobranie modeli ze zgodą TOFU, przebudowa wektorów embedderem e5-small | F7-02 (częściowo) | działa |

Bramki lokalne na Windows (2026-10-08, kopia na dysku przenośnym, `scripts/gates.ps1`): format Rust, graf
zależności (1645 krawędzi, 0 naruszeń), lint, svelte-check i vitest (219/219) — zielone; `cargo test` siedmiu
crate'ów rdzenia — 122 testy zielone. Nie uruchomiono: clippy, pełnego `cargo test --workspace` (za mało miejsca
na dysku) i `cargo-deny` (brak narzędzia). Poprawione przy okazji: `check-deps.sh` zgłaszał na Windows fałszywe
naruszenia (CRLF z `jq`), a `check-css.mjs` i `check-svelte5.mjs` nie działały w ścieżce Windows ze spacjami.

Poprawki z nocy 7/8.10 (gałąź robocza, commity `020a9d4`, `a1c803c`, `e594e61` i następny):

| Obszar | Co się zmieniło | Dowód |
| --- | --- | --- |
| Skalowanie interfejsu | kolumna rozmowy „Dopasowana do okna” (domyślna), `ui.zoom_fit` — skala rośnie z oknem powyżej 1600 px, strony ustawień wyśrodkowane do 1040 px | `src/lib/logic/__tests__/appearance.test.ts`; zrzuty `apps/desktop/ui/scripts/shots.mjs` (1280, 1920, 2560 px) |
| Laptop 1280 px | prawy panel zadokowany obok rozmowy, Sesje jako pasek ikon (wcześniej szuflada zasłaniała rozmowę) | `logic/__tests__/layout.test.ts`, `state/__tests__/stores.test.ts` |
| Ustawienia | 29 stron w 7 grupach; „fala N” → „wkrótce” | `logic/__tests__/settings-groups.test.ts` |
| Model bez silnika (ustalenie 1 i 2 wyżej) | czytelny błąd z przyciskiem „Modele i silniki”, bez fałszywego „fallbacku na CPU” | `providers-local-impl/tests/lifecycle.rs::missing_engine_gives_actionable_error_and_no_fallback`, `missing_gpu_engine_uses_cpu_build`; `app-chat` `outcome::tests`; `e2e/composer.spec.ts` |
| Zdrowie systemu | statusy incydentów po polsku | — |

Skrót `Ctrl+Alt+Space`: na tym laptopie jest zajęty przez inny program. Sprawdzone 2026-10-08 na działającej
aplikacji — komunikat „Skrót Ctrl+Alt+Space jest zajęty przez inny program” **pojawia się** w oknie (wcześniejsze
podejrzenie, że ginie przy starcie, było błędne). Zostaje wybór innego skrótu na stronie Skróty albo zwolnienie go
w drugim programie; trwały wskaźnik na stronie Skróty byłby ulepszeniem, nie naprawą błędu.

Przegląd interfejsu strona po stronie (29 stron ustawień, 7 paneli; commity `e358807`, `8d64fa2`): karty panelu
prawego zawijają się — „Ekran” i „Głos” były poza krawędzią wąskiego panelu i niedostępne myszą; powtórzone nagłówki
stron; nazwy zakresów pamięci bez powtórzeń i spokojniejsze przyciski „Zapomnij cały zakres”; nazwy narzędzi GUI po
polsku; nawigacja ustawień ze stałym nagłówkiem. W drugim przejściu poprawione też: powody zatrzymania wtyczki po polsku, harmonogram
wyzwalacza słowami (`logic/cron-text.ts`), edytor JSON Marszałka zwinięty pod „Zaawansowane”, onboarding na
środku okna.

Przeglądy kodu rdzenia w osobnych sesjach (2026-10-08; tylko odczyt) — instalacja modeli i silników oraz przebieg
rozmowy. Naprawione od razu, z testami:

| Ustalenie | Gdzie | Test |
| --- | --- | --- |
| Model bez silnika był zgłaszany jako zdrowy — Router i zadania w tle próbowały trasy lokalnej w kółko (wpis w logu co 15 min) | `providers-local-impl` `config.rs::engine_present`, `provider.rs`, `module.rs` | `lifecycle.rs::missing_engine_gives_actionable_error_and_no_fallback` |
| Nieudany start na procesorze po nieudanym starcie na karcie zostawiał dzierżawę pamięci | `providers-local-impl/src/sidecar.rs` | przegląd kodu (brak testu) |
| Onboarding proponował sam model, bez silnika | `views/onboarding/LocalBundleCard.svelte` — pakiet zalecany z silnikiem i zgodą na sumy | `e2e/bundles.spec.ts` |
| Błąd w trakcie „Myślę…” zostawiał pulsujący wskaźnik | `logic/apply-event.ts` | `logic/__tests__/turn-tree.test.ts` |
| „Kontynuuj” zostawało po kontynuacji (kolejne kliknięcie tworzyło duplikat) | `logic/turn-tree.ts::isContinued` | jw. |
| Po przełączeniu wariantu ‹ › znikał Stop, a Esc nie zatrzymywał trwającej generacji | `logic/turn-tree.ts::streamingTurn` | jw. |

Otwarte ustalenia z tych przeglądów (do naprawy w kolejnych sesjach; wymagają budowy `app-core`):

| # | Ustalenie | Gdzie | Pewność |
| --- | --- | --- | --- |
| R-01 | **naprawione 2026-10-08**: `flush_queue` gubił kolejkę offline przy błędzie jednej sesji — nieobsłużone sesje wracają teraz do kolejki | `app-core/src/commands/queue.rs` | testy `errors.rs` zielone; brak osobnego testu błędu w środku kolejki |
| R-02 | **naprawione 2026-10-08**: nieudany start odpowiedzi po zapisanej wiadomości daje komunikat i „Ponów” zamiast błędu wysyłki (koniec duplikatów) | `app-core/src/commands/turns.rs::start_after_user` | testy `commands.rs`, `errors.rs` zielone; brak testu regresji |
| R-03 | **naprawione 2026-10-08**: „Edytuj i wyślij ponownie” przenosi bloki (załączniki) pierwotnej wiadomości do nowej gałęzi | `app-core/src/commands/turns.rs` | testy `commands.rs`, `files.rs` zielone; brak osobnego testu regresji |
| R-04 | **naprawione w interfejsie 2026-10-08**: strona Modele i silniki ostrzega, gdy model jest na dysku bez pasującego silnika, i prowadzi do silników (`logic/models.ts::missingEngines`, testy); onboarding proponuje pakiet z silnikiem. Otwarte: pole „brakujące silniki” w DTO rdzenia | `app-models/src/lib.rs`, `view.rs` | potwierdzone odczytem |
| R-05 | Wszystkie silniki mają `sha256: None` — po pobraniu czekają na zgodę i `sidecars` zostaje puste, choć model jest „zainstalowany”; sumy wydań b6710 i whisper są w przewodniku 11 i można je przypiąć (decyzja właściciela) | `app-models/src/data.rs` | mechanizm potwierdzony |
| R-06 | **naprawione 2026-10-08**: nieudana instalacja archiwum czyści katalog roboczy; 416 uznaje plik częściowy tylko przy zgodnym rozmiarze | `app-models/src/jobs.rs`, `fetch.rs` | testy `download.rs`, `unpack.rs` zielone; brak testu regresji |
| R-07 | Interfejs może pokazać „Zainstalowano”, gdy dostawca lokalny uzna model za niezainstalowany (brak rekordu `.sha256`) | `app-models/src/view.rs` vs `providers-local-impl/src/download.rs` | utajone |
| R-08 | **naprawione 2026-10-08**: limit startu modelu rośnie o 20 s na każdy gigabajt pliku (Bielik 4.5B: 140 s) — `providers-local-impl/src/sidecar.rs`; bez pomiaru na żywo | `providers-local-impl/src/config.rs` | podejrzenie |
| R-09 | Panika w zadaniu generacji zostawia turę „w toku”; nieudany zapis tury jest tylko logowany | `app-chat/src/generate.rs`, `finish.rs` | podejrzenie |

Druga tura przeglądów (głos, pamięć, narzędzia agentek i transfer; 2026-10-08). Naprawione od razu:

| Ustalenie | Gdzie |
| --- | --- |
| „Przeczytaj na głos” zawsze odmawiało (`unsupported`): powłoka nie przekazywała schowka funkcjom głosowym — objaw z testu na laptopie | `app-core/src/parts/agents.rs` (schowek systemowy jak w narzędziach agentek) |
| STOP WSZYSTKIEGO nie kończył dyktowania do cudzych okien | `app-core/src/commands/system.rs` |
| Strona Pamięć zawsze pisała „licznik bezczynności jest niedostępny” (`idle_available` na sztywno `false`) | `app-memory/src/app.rs`, `app-core/src/parts/memory.rs` |
| „Porządkuj teraz” pokazywało „zakończone” także przy pominięciu (bateria, tryb gry) | `views/settings/pages/MemoryPage.svelte` |
| Po edycji wpisu pamięci szczegóły zostawały na starej, zastąpionej wersji | `components/panels/MemoryDetail.svelte`, `MemoryPanel.svelte` |
| Cofnięcie utworzenia wpisu zostawiało „Nie udało się wczytać”; licznik „do zatwierdzenia” liczony z pierwszych 100 wpisów | `MemoryDetail.svelte`, `MemoryPanel.svelte`, `MemoryJournal.svelte` |
| Anulowanie okna wyboru pliku odbierało „Cofnij import”; martwy przycisk cofania, gdy import nic nie zmienił | `views/settings/pages/TransferImport.svelte` |

Otwarte z drugiej tury (wymagają decyzji albo większej zmiany):

| # | Ustalenie | Gdzie | Pewność |
| --- | --- | --- | --- |
| R-10 | Częściowo nieudane „Cofnij” jest w dzienniku oznaczane jako wykonane, a interfejs dalej je oferuje — pozostałych plików nie da się już przywrócić; wygasłe kroki wyglądają na cofalne | `undo-journal-contract/src/restore.rs`, `app-core/src/commands/turn_intents.rs`, `UndoCard.svelte` | potwierdzone odczytem |
| R-11 | **naprawione w logice 2026-10-08**: przy zmianie samej wielkości liter kontrola konfliktów nie wymaga już nieobecności starej nazwy (`undo-journal-contract/src/restore.rs::case_only_rename`, test jednostkowy). Niesprawdzone na prawdziwym NTFS | `undo-journal-contract/src/restore.rs`, `engine.rs` | potwierdzone odczytem |
| R-12 | **częściowo naprawione 2026-10-08**: niepełne cofnięcie importu zwraca błąd z liczbami zamiast sukcesu; interfejs pokazuje liczbę pominiętych przy imporcie. Otwarte: osobne pole `failed` i ostrzeżenia w DTO | `app-modules/src/transfer.rs`, DTO `ImportResult` | potwierdzone odczytem |
| R-13 | Cofnięcie po restarcie aplikacji zostawia utworzone katalogi | `undo-journal-contract/src/restore.rs` | odczyt kodu |
| R-14 | **naprawione 2026-10-08**: błąd odczytu hasła kopii zatrzymuje kopię z komunikatem zamiast robić wersję nieszyfrowaną (`app-files/src/backup/mod.rs::password_for_run`); testy `backup.rs` zielone, brak testu regresji | `app-files/src/backup/mod.rs` | podejrzenie |
| R-15 | **naprawione 2026-10-08**: niezapisane ustawienia kopii zostają po „Wybierz katalog” / „Wykonaj teraz” | `BackupSection.svelte` | potwierdzone odczytem |
| R-16 | **częściowo naprawione 2026-10-08**: Piper bez modelu głosu (`*.onnx`) nie jest już zgłaszany jako gotowy głos (`app-modules/src/tts.rs`, test w tym pliku). Otwarte: czytanie i dyktowanie nadal wyglądają na dostępne bez silników | `app-voice/src/features/view.rs`, `app-modules/src/tts.rs` | potwierdzone w Rust |
| R-17 | **naprawione 2026-10-08**: w pamięci zostaje tylko udane wczytanie modelu mówcy — model pobrany po starcie jest wykrywany bez restartu | `app-voice/src/features/mod.rs` | potwierdzone odczytem |
| R-18 | **naprawione 2026-10-08**: model rozpoznawania mowy dobierany do backendu (z kartą duży, bez karty mały; modele tylko angielskie na końcu) — `app-modules/src/stt.rs`, test w tym pliku | `app-modules/src/stt.rs` | potwierdzone odczytem |
| R-19 | **naprawione 2026-10-08** (strona interfejsu): po zainstalowaniu pozycji interfejs odświeża stan głosu | `app-voice/src/port.rs`, `MicIndicator.svelte` | częściowo potwierdzone |
| R-20 | Edycja oczekującej propozycji pamięci po cichu ją zatwierdza; „zapamiętaj” zapisuje treść odpowiedzi jako zaufaną | `memory-contract/src/engine/write.rs`, `app-memory/src/app.rs` | podejrzenie — do decyzji właściciela |

**Sprawdzenie na prawdziwej aplikacji (2026-10-08, ten laptop).** Powłoka z cechą `e2e` zbudowana z bieżącej
gałęzi (`cargo build --features "e2e tauri/custom-protocol"`, osobny katalog wyników `target/shell`) i uruchomiona
na tymczasowym profilu (`APPDATA` / `LOCALAPPDATA` wskazane na katalog roboczy — dane właściciela nietknięte):
`apps/desktop/ui/e2e-app/run.ps1` — **10/10** (start okna na prawdziwym rdzeniu, wprowadzenie, rozmowa bez modelu,
Broker w procesie, Modele i silniki z wykrytą kartą RTX 4050 i pakietem zalecanym 5, eksport `.alfa` przez natywne
okno, zatrzymanie z interfejsu, okna poboczne, konsola bez błędów, CSP z Trusted Types bez naruszeń). Potwierdzone na
zrzutach z działającej aplikacji: wyśrodkowany onboarding, ustawienia w grupach na szerokiej stronie, błąd „brak
modelu” z przyciskami „Modele i silniki” i „Dodaj klucz”. **Niesprawdzone na żywo:** odpowiedź modelu lokalnego
(wymaga pobrania modelu i silnika), głos, poprawki bez własnych testów. Skalowanie sprawdzone na żywo przez CDP
w czterech rozmiarach okna: skala 1,00 / 1,08 / 1,25 / 1,50 przy 1280 / 1920 / 2560 / 3840 px, kolumna rozmowy
691 / 847 / 1210 / 1500 px.

Trzecia tura przeglądów (mosty CLI, computer use, wtyczki i umiejętności, aktualizacje; użycie Brokera):

- Naprawione w interfejsie: przycisk „Uruchom ponownie” przy aktualizacji odblokowywał się po każdym zdarzeniu
  stanu (`state/updates.svelte.ts::applyEvent`); przy niezgodności z rejestrem zgodności „Przypnij” nie robiło nic
  widocznego, a „Odepnij” znikało (`BridgesSection.svelte`); strona uprawnień pokazywała nieaktualny poziom
  autonomii (`PermissionsPage.svelte`).
- Użycie Brokera: osiem ustaleń B-01…B-08 w [`reviews/2026-10-08-broker-usage-review.md`](reviews/2026-10-08-broker-usage-review.md)
  — obszar Jądra, **bez zmian w kodzie**, do decyzji właściciela (bramka #7). Najważniejsze: STOP WSZYSTKIEGO
  zgłasza sukces także wtedy, gdy Broker nie został osiągnięty (B-03); obniżony poziom autonomii nie wraca po
  starcie, gdy łącze z Brokerem jeszcze nie działa (B-01).

| # | Ustalenie | Gdzie | Pewność |
| --- | --- | --- | --- |
| R-21 | Dzienny limit uruchomień mostu z harmonogramu zeruje się przy każdej zmianie przypięcia albo harmonogramu i po restarcie (licznik tylko w pamięci) | `app-bridges/src/app.rs`, `agent-backends-impl/src/gate.rs` | potwierdzone odczytem |
| R-22 | „Zatrzymaj sterowanie” zatrzymuje tury rozmowy, ale prawdopodobnie nie zadania mostów CLI ani schedulera; błędy `turns_stop` są pomijane | `app-core/src/commands/work.rs` | podejrzenie |
| R-23 | Wyłączonej umiejętności nie da się włączyć ponownie (brak operacji; wtyczki ją mają) | `app-skills`, `SkillsPage.svelte` | potwierdzone — może zamierzone |
| R-24 | Podwójne wywołanie pobierania aktualizacji może zamienić stan „gotowa” na „nieudana” | `app-updates/src/lib.rs`, `updater-impl/src/download.rs` | podejrzenie |
| R-25 | Włączenie wtyczki przekazuje tylko identyfikator — przy dwóch wyłączonych wersjach suma może nie pasować do wybranej; „Usuń” wtyczki bez potwierdzenia | `app-plugins/src/app.rs`, `PluginsPage.svelte` | podejrzenie |

**Podział pozostałych otwartych ustaleń (stan na koniec nocy 7/8.10):**

- *Czekają na decyzję właściciela:* R-05 (przypięcie sum kontrolnych silników), R-10 (częściowe „Cofnij” — adapter
  Brokera), R-20 (edycja oczekującej propozycji pamięci i zaufanie do „zapamiętaj”), R-21 (licznik limitu mostów —
  bramka zgodności), R-24 (aktualizator), B-01…B-08 (użycie Brokera).
- *Do zrobienia bez decyzji, nietknięte:* R-07, R-09, R-13, R-22, R-23, rdzeniowa część R-25 oraz pola w DTO dla
  R-04, R-12 i R-16.
- *Bez własnych testów regresji* (przeszły tylko testy istniejące): R-01, R-02, R-03, R-06, R-08, R-14, schowek dla
  głosu, dyktowanie przy STOP, wyciek dzierżawy pamięci.

Testy E2E na tym laptopie (Edge, `PW_CHANNEL=msedge`): komplet 125 zielony w ostatnim przebiegu; test palety bywa
czerwony pod obciążeniem. Test „paleta: otwarcie ≤ 50 ms” ma
medianę ok. 9 ms, ale 1–2 z 7 prób trwają 230–440 ms (próg maksimum 150 ms) — tak samo przed zmianami tej nocy.
Pomiar rozdzielony: samo otwarcie zawsze trwa ok. 7 ms; opóźnia się pojedyncza klatka obrazu (ok. 250–450 ms)
mniej więcej 2 s po starcie, bez długiego zadania JavaScript i niezależnie od rozmycia tła palety — wygląda na
harmonogram klatek przeglądarki bez okna, nie na kod Alfy. Do potwierdzenia w prawdziwej aplikacji (F1-09).
Testy `slow-core` miały za krótki limit na start aplikacji przy wolnym rdzeniu — poprawione.

Zmiana procesu: recenzję robi osobna sesja Claude w świeżym kontekście, nie drugi model (ADR 0016). Wszędzie
niżej „model-recenzent” ma to znaczenie.

## Legenda

| Znak | Znaczenie                                                                                                    |
| ---- | ------------------------------------------------------------------------------------------------------------ |
| ✅   | Spełnione w CI lub na atrapach (deterministycznie) — podany test i wynik.                                    |
| 🟡   | Zaimplementowane, ale wymaga pomiaru na sprzęcie / korpusie / kluczu (kto i która bramka ludzka, PLAN §4.6). |
| ⛔   | Brak implementacji albo brak zestawu/testu, którego wymaga kryterium (powód i bloker).                       |
| ⚖️   | Wymaga decyzji lub akceptacji człowieka (nic nie trzeba mierzyć).                                            |

Bramki ludzkie (PLAN §4.6): #1 logowanie do CLI i klucze API · #2 UAC / Hello · #3 korpus własny · #4 makiety ·
#5 odsłuchy · #6 testy akustyczne · #7 ADR-y, decyzje §19, PR-y Jądra/Brokera · #8 przygotowanie maszyn i VM ·
#9 repo i runnery self-hosted · #10 klucz minisign, certyfikat, konto usługi Brokera.

## Stan CI i bramek lokalnych

- Workflow `.github/workflows/ci.yml`: Rust (ubuntu-24.04, windows-latest), Powłoka Tauri (Windows; testy z cechą
  `e2e` i bez), cargo-deny, UI (Svelte 5), Dokumentacja — wszystkie zielone na `a5c642d` i kolejnych commitach fali 6.
- Workflow `.github/workflows/rehearsal.yml` („Próba generalna (Windows)”, fala 6 — przed testem na laptopie
  właściciela): `hf-catalog` (adresy, rozmiary i SHA-256 plików z katalogów modeli w API Hugging Face — bramka),
  `setup-dev` (`scripts/setup-dev.ps1 -Ci` pod PowerShell 5.1 i 7 — zielony), `live-engines` (prawdziwe pobranie,
  rozpakowanie i uruchomienie `llama-server`, `whisper-server`, `piper` przez `ModelsApp` na CPU, raport SHA-256)
  i `app-e2e` (prawdziwa aplikacja: build z cechą `e2e`, Playwright przez CDP do WebView2). Wyniki — punkt 12 luk.
- Bramki lokalne (sesja w chmurze, Linux): fmt, clippy `-D warnings` (+ cross-clippy `x86_64-pc-windows-msvc`),
  `cargo test --workspace`, cargo deny, check-deps; UI: lint, svelte-check, build, budżet JS, Storybook, prettier,
  vitest, Playwright E2E na atrapie z axe.

## F0 — Fundament i spike'i

| ID    | Kryterium                                                                   | Status | Dowód / co dalej                                                                                                                                                                                                                                                                |
| ----- | --------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F0-01 | CI zielone (fmt, clippy, test, svelte-check, cargo-deny, diff generowanych) | ✅     | `.github/workflows/ci.yml`; pełna zieleń m.in. na `a5c642d` (Linux i Windows); próba generalna na runnerze Windows — `rehearsal.yml`.                                                                                                                                           |
| F0-02 | Trójka crate'ów + test kontraktowy na fake i impl                           | ✅     | `example-module-impl/tests/module.rs::contract_suite`, `example-module-fake/tests/fake.rs::contract_suite`; `scripts/check-deps.sh` — 0 naruszeń.                                                                                                                               |
| F0-03 | Spike (a): profil A p50 ≤ 2000 / p95 ≤ 3000 ms                              | 🟡     | Atrapy: p50 1380 / p95 1610 ms (`voice-pipeline-impl/tests/e2e_turn.rs::time_to_first_audio_profile_a_budget_100_scenarios`). Pomiar ≥ 50 tur loopbackiem na desktopie z emulacją i laptopie wg `evals/spikes/a-petla-glosowa/` (`results/` pusty). Bramki #6, #8.              |
| F0-04 | Spike (a): AEC bez fałszywych przerwań przy własnym TTS                     | 🟡     | Atrapy: `e2e_barge.rs::own_tts_echo_through_room_does_not_interrupt`; AEC3 w `voice-dsp` (ERLE 49 dB). 30 min TTS przez głośniki + odsłuch, wynik do ADR 11. Bramki #6, #8.                                                                                                     |
| F0-05 | Spike (b): prośby o uprawnienia CLI w naszym kanale (n ≥ 20)                | 🟡     | Fałszywe CLI 20/20: `agent-backends-fake/tests/compliance.rs::twenty_of_twenty_permission_requests_reach_sink_and_mcp_host`. Prawdziwe `claude`/`codex` wg `evals/spikes/b-most-cli/`. Bramka #1.                                                                               |
| F0-06 | Spike (b): 0 odczytów tokenów CLI                                           | 🟡     | Statycznie: `agent-backends-impl/tests/rules.rs::sources_never_touch_cli_credentials`, `accounts-hub-impl/tests/cli.rs::no_cli_credential_paths_in_sources`, SR-06. Monitor ETW/Procmon na desktopie. Bramki #1, #8.                                                            |
| F0-07 | Spike (b): zimny start mostu zmierzony                                      | 🟡     | 10 startów prawdziwego CLI, wynik do ADR 5. Bramka #1.                                                                                                                                                                                                                          |
| F0-08 | Spike (e): WER PL ≤ 12 %                                                    | 🟡     | Runner `alfa-voice-eval` (`evals/F2/README.md`); korpus nienagrany. Bramka #3.                                                                                                                                                                                                  |
| F0-09 | Spike (e): ślepa ocena TTS ≥ 4,0                                            | 🟡     | Protokół `evals/spikes/e-voice-lab/`; ocenia właściciel. Bramka #5 (no-go nie blokuje F1).                                                                                                                                                                                      |
| F0-10 | Spike (e): tabela kandydatów głosu na obu maszynach                         | 🟡     | Tabela `evals/spikes/e-voice-lab/candidates.md` bez pomiarów TTFB i zasobów. Bramka #8.                                                                                                                                                                                         |
| F0-11 | Spike (f): RAM drzewa Tauri/WebView2 (1 i 3 okna)                           | 🟡     | `evals/spikes/f-ram-tauri/measure-ram.ps1`, `results/` pusty. Od wyniku zależą F1-08, F5-12, F9-02. Bramka #8.                                                                                                                                                                  |
| F0-12 | Spike (h): whisper.cpp Vulkan i CUDA 1 h bez crashy                         | 🟡     | `evals/spikes/h-sprzet/bench-whisper.ps1` na RX 9070 XT i RTX 4050. Bramka #8.                                                                                                                                                                                                  |
| F0-13 | Spike (h): RTF Pocket-PL, tok/s modeli 4B/8B                                | 🟡     | `bench-llama.ps1`, `bench_pocket_tts.py`, szablon `results/TEMPLATE.md`. Wynik wyznacza próg F1-03 i F3-07. Bramka #8.                                                                                                                                                          |
| F0-14 | Spike (i): SQLCipher + sqlite-vec + FTS5                                    | ✅     | `crates/spike-data/tests/spike.rs` (4 testy), `evals/spikes/i-dane/RESULT.md`, ADR 0008.                                                                                                                                                                                        |
| F0-15 | Spike (j): Snap Layouts, Mica, pisownia PL, toast z AUMID                   | 🟡     | Protokół `evals/spikes/j-powloka/`. Dziś natywne dekoracje okna, bez Mica i AUMID z launchera. Właściciel + model-recenzent, bramka #8.                                                                                                                                         |
| F0-16 | Spike (k): Broker-UI na High IL z usługi, SendInput odrzucony               | 🟡     | Kod w `platform-windows-kernel-impl`; `tests/kernel_windows.rs::send_input_into_broker_window_is_always_flagged_injected` (`#[ignore]`, pulpit); atrapy: `app-safety/tests/chain.rs::approval_crosses_all_processes_and_injection_never_succeeds`. Self-hosted. Bramki #9, #10. |
| F0-17 | Makiety 1–3 zaakceptowane                                                   | ⚖️     | `packages/ui-kit/src/stories/Screens.stories.ts` (Start, Conversation, VoiceMode; jasny/ciemny). Akceptacja — bramka #4.                                                                                                                                                        |
| F0-18 | ADR 1–15 zaakceptowane                                                      | ⚖️     | `docs/ADR/README.md`: 14 „Zaakceptowany”, ADR 0006 „Tymczasowy” (spike g wymaga klucza Anthropic). Potwierdzenie — bramka #7.                                                                                                                                                   |
| F0-19 | `THREAT_MODEL.md` i rdzeń `evals/` z hashami w CI                           | ✅     | `docs/THREAT_MODEL.md`; `evals-impl/tests/repo_suites.rs::f8_suite_is_intact_and_listed`, `alfa-evals verify evals`. `evals/acceptance/HASHES` pusty — nic jeszcze nie zamrożono.                                                                                               |

## F1 — Rdzeń czatu

| ID    | Kryterium                                                 | Status | Dowód / co dalej                                                                                                                                                                                                                                                                                                                   |
| ----- | --------------------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F1-01 | ≥ 3 adaptery API zielone na fixture'ach syntetycznych     | ✅     | `providers-api-impl/tests/{anthropic,anthropic_errors,openai_chat,openai_responses,catalog}.rs` na serwerze SSE z `tests/support`; generyczny: `catalog.rs::openai_compatible_entry_streams_via_generic_adapter`, `anthropic_compatible_entry_uses_messages_api_without_betas`. Fixture'y w crate, nie w `evals/F1/api-fixtures/`. |
| F1-02 | Walidacja adapterów na żywo (nb)                          | 🟡     | Wymaga kluczy; nightly nieskonfigurowany. Bramka #1.                                                                                                                                                                                                                                                                               |
| F1-03 | llama.cpp na żywo: 20 promptów PL, tok/s ≥ F0-13          | 🟡     | Sidecar `llama-server`, pobieranie Bielik 4.5B Q8_0 (`providers-local-impl`); testy na `fake-llama-server` (`tests/{lifecycle,contract,download}.rs`). Zestaw 20 promptów PL: szkic `evals/F1/prompts-pl/` (warunki `must` = „bez błędu”, jakość raportowana; do akceptacji człowieka). Brak runnera i pomiaru na self-hosted. Bramka #8. |
| F1-04 | Fallback ≤ 2 s, 0 utraconych wiadomości (50 prób)         | ✅     | `router-impl/tests/fallback.rs::fifty_trials_5xx_and_timeout_switch_within_2s_without_loss` (5xx: 0,28 ms); w aplikacji `app-core/tests/routing.rs::router_falls_back_after_5xx_and_ui_gets_one_answer`.                                                                                                                           |
| F1-05 | Izolacja ≥ 3 sesji równolegle                             | ✅     | `app-core/tests/spy.rs::parallel_sessions_never_leak_into_each_other` (4 sesje), `spy_modes.rs::agent_voice_and_chat_sessions_never_leak`, `sessions-impl/tests/storage.rs::three_parallel_sessions_do_not_leak`.                                                                                                                  |
| F1-06 | Private WS ≤ +5 % po 1 h / 500 wiadomościach              | 🟡     | Brak skryptu E2E; pomiar na self-hosted z emulacją baseline. Bramki #8, #9.                                                                                                                                                                                                                                                        |
| F1-07 | axe 0 critical/serious, obsługa klawiaturą                | ✅     | `apps/desktop/ui/e2e/a11y.spec.ts`, `keyboard.spec.ts` (+ axe w `agents-voice`, `memory-tasks`, `computer`): E2E 62/62 w obu motywach.                                                                                                                                                                                             |
| F1-08 | Budżety lekkości §3.4                                     | 🟡     | Czeka na wartości z F0-11 (ADR) i pomiar drzewa procesów na self-hosted.                                                                                                                                                                                                                                                           |
| F1-09 | Budżety UI §14.7                                          | 🟡     | W CI (bez limitów baseline): JS 90/150 KB i CSS (`scripts/bundle-size.mjs`), paleta ≤ 50 ms i DOM raz na klatkę (`keyboard.spec.ts`). Brak: composer ≤ 16 ms p95, 60 kl./s, panel ≤ 100 ms, 1000 wiadomości ≤ 150 ms — ślad CDP na self-hosted.                                                                                    |
| F1-10 | Round-trip `.alfa` desktop → laptop → desktop, 0 sekretów | 🟡     | CI: `transfer-contract` `contract_tests::round_trip_restores_identical_state`, `no_secrets_in_plain_export`; `app-core/tests/transfer.rs::export_import_roundtrip_through_commands`. Przebieg między maszynami — bramka #8.                                                                                                        |
| F1-11 | Klucz dodany bez restartu, trasa aktywna ≤ 5 s            | ✅     | `app-core/tests/commands.rs::accounts_store_key_in_vault_and_never_return_it` (konto działa od razu w tym samym procesie). Czasu ≤ 5 s test nie mierzy jawnie.                                                                                                                                                                     |
| F1-12 | Reguła AltGr i kill-switch zarejestrowany                 | ✅     | `platform-contract/src/hotkey.rs::altgr_rule_rejects_all_polish_letters`, `kill_switch_reserved_and_display`; `app-core/tests/units.rs::altgr_rule_and_kill_switch_are_enforced`; UI `src/lib/logic/__tests__/shortcuts.test.ts`.                                                                                                  |
| F1-13 | Wyłączona trasa: 0 wywołań w 100 próbach                  | ✅     | `router-impl/tests/decisions.rs::disabled_route_gets_zero_calls_in_100_tries`.                                                                                                                                                                                                                                                     |

## F2 — Głos rdzeniowy + agentki

| ID    | Kryterium                                                 | Status | Dowód / co dalej                                                                                                                                                                                            |
| ----- | --------------------------------------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F2-01 | Profil A p50 ≤ 2000 / p95 ≤ 3000 ms (≥ 200 tur)           | 🟡     | Atrapy: p50 1380 / p95 1610 ms (`e2e_turn.rs`). Prawdziwe modele: `voice-pipeline-impl` `real_models_on_corpus` (`#[ignore]`). Bramki #3, #8.                                                               |
| F2-02 | Profile B/C (nb)                                          | 🟡     | Wymaga kluczy usług mowy; chmurowe STT/TTS są dziś tylko typami w `voice-stt`/`voice-tts`. Bramka #1.                                                                                                       |
| F2-03 | WER PL ≤ 12 %                                             | 🟡     | `alfa-voice-eval score`; CI sprawdza tylko format na 3 próbkach (`voice-pipeline-impl/tests/f2_eval.rs`). Korpus i zamrożenie `evals/acceptance/F2/test.sha256`. Bramka #3.                                 |
| F2-04 | Recall „stop/anuluj” ≥ 99 %, reakcja < 300 ms             | 🟡     | Atrapy: `e2e_barge.rs::stop_and_wait_commands_react_under_300ms` (190 ms); gramatyka: `voice-cmd-impl/tests/frozen_set.rs::recall_and_negative_precision` (113/113, 0/88). ≥ 200 prób z korpusu. Bramka #3. |
| F2-05 | Precision backchannelu ≥ 95 %                             | 🟡     | Atrapy: `e2e_barge.rs::backchannel_every_3s_for_60s_never_interrupts`, `voice-dialog-impl/tests/metrics.rs::backchannel_every_3s_for_60s_gives_zero_false_interruptions`. Korpus. Bramka #3.                |
| F2-06 | Fałszywe przerwania ≤ 1/h                                 | 🟡     | 1 h TTS przez głośniki laptopa z TV, dziennik → `alfa-voice-eval score --session-log`. Bramka #6.                                                                                                           |
| F2-07 | Usłyszany prefiks ±1 słowo w ≥ 90 %                       | 🟡     | Atrapy: `voice-dialog-impl/tests/metrics.rs::heard_prefix_accuracy`, `e2e_barge.rs::barge_in_ducks_stops_and_keeps_heard_prefix`. ≥ 100 przerwań z korpusu. Bramka #3.                                      |
| F2-08 | Intencje przerwań ≥ 90 % dla każdej klasy                 | 🟡     | 6 klas w `voice-dialog` (`tests/classifier.rs`); ≥ 50 przykładów na klasę z korpusu. Bramka #3.                                                                                                             |
| F2-09 | „Nie” przerywa tylko samodzielnie w `Speaking`            | 🟡     | Reguła: `voice-dialog-impl/tests/scenarios.rs::standalone_nie_from_voice_cmd_interrupts_only_while_speaking`. ≥ 50 nagranych fraz „nie no, dobrze”. Bramka #3.                                              |
| F2-10 | Odrębność głosów v0 (ECAPA ≤ 0,6, ABX ≥ 90 %)             | 🟡     | `voice-tts-contract/src/voices.rs::v0_has_four_distinct_voices_from_two_bases`. Pomiar wymaga modeli TTS i ECAPA; ABX — właściciel. Bramka #5.                                                              |
| F2-11 | Zmiana obsady w locie 20/20                               | ✅     | `voice-pipeline-impl/tests/e2e_persona_ptt.rs::persona_switches_live_twenty_times`, `ui_switch_changes_voice_of_next_reply`.                                                                                |
| F2-12 | Wyłączność głośnika/mikrofonu (100 scenariuszy)           | ✅     | `scheduler-lite-impl/tests/module.rs::hundred_concurrent_scenarios_without_overlap`; 1000 losowych w `scheduler-lite-contract/src/contract_tests/`.                                                         |
| F2-13 | STT + TTS + LLM w budżecie VRAM; pełny ekran → zwolnienie | 🟡     | Logika: `model-residency-impl/tests/manager.rs::budgets_follow_device_profile`, `fullscreen_and_battery_signals_switch_modes`. Pomiar VRAM na desktopie i laptopie. Bramka #8.                              |
| F2-14 | Ducking < 50 ms, twardy stop ≤ 400 ms w 95 %              | 🟡     | Automat: `voice-dialog-impl/tests/metrics.rs::hard_stop_within_400ms_and_duck_immediately` (p95 350 ms). ≥ 100 prób na sprzęcie. Bramki #6, #8.                                                             |

## F3 — Safety Kernel + system (koniec MVP)

| ID    | Kryterium                                                   | Status | Dowód / co dalej                                                                                                                                                                                                                                                                                                                      |
| ----- | ----------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F3-01 | Kill-switch: cisza + zabite Job Objects < 200 ms p95        | 🟡     | Logika: `safety-broker-impl/tests/budget.rs::kill_switch_p95_under_200ms` (≈ 0,14 ms), `watchdog-impl/tests/watchdog.rs::kill_switch_p95_50_trials`, `app-core/tests/wiring.rs::kill_switch_cancels_running_generations_quickly` (3,3 ms). 50 prób z audio i Job Objects pod obciążeniem UI na self-hosted.                           |
| F3-02 | Cofalność `fs.*` 100 % (≥ 200 losowych operacji)            | ✅     | `tools-fs-impl/tests/props.rs::random_sequences_are_fully_undoable` (50 × 4–10 operacji), `undo-journal-contract/tests/props.rs` (256 przypadków), `undo-journal-impl/tests/journal.rs` (200).                                                                                                                                        |
| F3-03 | Snapshot zakresu shella: 100 % po ≥ 50 skryptach            | ✅     | `tools-shell-impl/tests/props.rs::scripts_in_scope_are_fully_undoable` (60), `run.rs::snapshot_then_script_then_undo_restores_scope`.                                                                                                                                                                                                 |
| F3-04 | „Agentka zmienia Jądro / zatwierdza sama siebie”: 0 w ≥ 100 | 🟡     | Logika: `safety-broker-impl/tests/negative.rs::zero_successful_attacks` (116 scenariuszy), `app-safety/tests/chain.rs` (100 wstrzykniętych kliknięć → 0), `tests/review.rs`. Prawdziwy SendInput do Broker-UI (`kernel_windows.rs`, `#[ignore]`) na self-hosted.                                                                      |
| F3-05 | Red-team prompt injection ≥ 100: 0 eskalacji, 0 egressu     | ⛔     | Zestaw `evals/F3/redteam/` nie istnieje — tworzy model-recenzent. Częściowe obrony z testami: taint (F3-06), skaner MCP `mcp-contract/src/injection.rs`, XSS `lib-markdown/tests/xss.rs`.                                                                                                                                             |
| F3-06 | Sesja `tainted`: wysokie ryzyko i `net.egress` pytają       | ✅     | `risk-classifier-contract/tests/props.rs::tainted_egress_never_proceeds_even_on_l4`, `safety-broker-contract` `flows::tainted_egress_asks_on_l4`, SR-01 `safety-broker-impl/tests/review.rs::auto_approved_plan_does_not_cover_tainted_egress`.                                                                                       |
| F3-07 | Eval narzędzi fs/shell na lokalnym 3–4,5B ≥ próg            | 🟡     | 35 zadań `evals/F3/tools/tasks.json`, CI na atrapie: `cargo test -p app-agents --test eval_tools`; pomiar `measure_on_local_model` (`#[ignore]`, Windows + `llama-server`). Próg ustala właściciel (ADR 14).                                                                                                                          |
| F3-08 | Destrukcja głosem → potwierdzenie nie-głosem, także na L4   | ✅     | `risk-classifier-contract/tests/props.rs::voice_destruction_never_proceeds`, `safety-broker-contract` `flows::voice_destruction_asks_on_l4`. E2E z prawdziwym głosem — w F3-12.                                                                                                                                                       |
| F3-09 | Audyt: Broker jedynym writerem, łańcuch, ACL append-only    | 🟡     | `safety-broker-impl/tests/audit.rs::chain_of_10k_events_verifies_and_survives_reopen`, `tampering_is_detected`; `app-safety/tests/windows_ports.rs` (DACL potoku). Brak próby zapisu z procesu agentki do plików Audytu na Windows; SR-08 bez testu Windows.                                                                          |
| F3-10 | Aktualizacja + rollback launchera (10 cykli)                | 🟡     | Pełny cykl gotowy: pobieranie ze wznawianiem, minisign + SHA-256, instalacja obok, rollback i crash-loop — `updater-impl/tests/update_flow.rs::good_update_full_cycle`, `user_rollback_and_watchdog_rollback`, `update_safety.rs::downgrade_needs_explicit_user_rollback`, `launcher.rs::crash_of_new_version_falls_back_to_previous`; `app-updates/tests/updates.rs`. Zostały: klucz minisign (bramka #10), test instalatora NSIS i 10 cykli na desktopie i laptopie (self-hosted). |
| F3-11 | Watchdog: safe-mode po N awariach, restart (20 scenariuszy) | ✅     | `watchdog-impl/tests/watchdog.rs::crash_loop_enters_safe_mode_and_rolls_back_once`, `restarts_outside_window_never_loop` (20 iteracji), `missed_heartbeats_and_failing_health`. Brak jednego zestawu 20 scenariuszy.                                                                                                                  |
| F3-12 | Scenariusz MVP bez kluczy 6/6                               | ⛔     | Brak `evals/F3/mvp-scenario.md`. Instalator jest: menedżer modeli i sidecarów `app-models` (Ustawienia → „Modele i silniki”, onboarding) — `llama-server`, `whisper-server`, `piper`, GGUF, whisper, Piper, Silero, openWakeWord, model mówcy; HTTPS z wznawianiem, SHA-256 / zgoda TOFU, bezpieczne ZIP (`app-models/tests/{download,unpack}.rs`). Blokery: okno zatwierdzeń przez Brokera poza procesem (`app-broker`) niezweryfikowane na sprzęcie; adresy/hashe sidecarów i modeli głosu do przypięcia przez człowieka (HF/GitHub zablokowane w sesji); pomiary F0/F2. |
| F3-13 | Port CDP zamknięty w buildzie produkcyjnym (test CI)        | ✅     | `apps/desktop/src-tauri/src/cdp.rs`: `production_build_adds_no_browser_args` (bez cechy `e2e` — `None`), `cdp_flag_exists_only_behind_e2e_feature` (jedyne wystąpienie flagi w powłoce i `tauri*.conf.json` za `#[cfg(feature = "e2e")]`), `production_refuses_environment_that_opens_cdp` (odmowa startu przy `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` ze zdalnym debugowaniem). CI: job „Powłoka Tauri (Windows)” — `cargo test --lib` z cechą `e2e` i bez (fala 5; pierwszy przebieg po commicie). |

## F4 — Mosty i MCP

| ID    | Kryterium                                          | Status | Dowód / co dalej                                                                                                                                                                                                        |
| ----- | -------------------------------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F4-01 | Postęp delegowanego zadania ≤ 1 s                  | 🟡     | Fałszywe CLI: `agent-backends-fake/tests/bridges.rs::progress_latency_within_budget`. 20 zadań z prawdziwym CLI. Bramka #1.                                                                                             |
| F4-02 | Anulowanie ≤ 2 s                                   | 🟡     | `bridges.rs::cancel_kills_process_tree`. Prawdziwe CLI. Bramka #1.                                                                                                                                                      |
| F4-03 | 20/20 próśb o uprawnienia trafia do Broker-UI      | 🟡     | `compliance.rs::twenty_of_twenty_permission_requests_reach_sink_and_mcp_host` (fałszywe CLI). Wymaga okna Brokera podłączonego w aplikacji i prawdziwego CLI. Bramka #1.                                                |
| F4-04 | 0 odczytów `~/.claude`, `~/.codex` (ETW)           | 🟡     | Testy źródeł + SR-06 `agent-backends-impl/tests/review.rs::copy_skips_credential_stores`. ETW podczas 20 zadań — model-recenzent. Bramki #1, #8.                                                                        |
| F4-05 | Wyłącznik trasy: 0 wywołań w 100 próbach           | ✅     | `agent-backends-fake/tests/compliance.rs::disabled_route_means_zero_calls_0_of_100`.                                                                                                                                    |
| F4-06 | Nieświeży rejestr degraduje trasę do „szarej”      | ✅     | `compliance-contract` `contract_tests::stale_route_degrades`, `status.rs::degradation_rules`; `compliance-impl/tests/service.rs::repo_registry_private_sessions_and_staleness`.                                         |
| F4-07 | Serwer MCP tylko stdio/potok, bez TCP; hash opisów | ✅     | `mcp-impl/tests/static_rules.rs::no_network_listeners_in_sources`; `mcp-contract/src/trust.rs::changed_description_blocks_even_on_trusted_server`, `injected_description_is_never_auto_approved`.                       |
| F4-08 | Kontrakty mostów na nagranych fixture'ach          | 🟡     | Na skryptach fałszywego CLI: `agent-backends-fake/tests/contract.rs::claude_bridge_passes_contract`, `codex_bridge_passes_contract`. Brak `evals/F4/bridge-fixtures/` nagranych z prawdziwego CLI (spike b). Bramka #1. |

## F5 — Agentki i orkiestracja + głos rozszerzony

| ID    | Kryterium                                               | Status | Dowód / co dalej                                                                                                                                                                                                            |
| ----- | ------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F5-01 | Agentki równolegle: 0 konfliktów zasobów w 100          | ✅     | `scheduler-fake/tests/parallel.rs::parallel_agents_without_resource_conflicts`, `agent-runtime-impl/tests/parallel.rs::hundred_random_scenarios_have_no_collisions`.                                                        |
| F5-02 | Steering w ≤ 1 kroku atomowym 20/20                     | ✅     | `scheduler-fake/tests/steering.rs::steering_within_one_atomic_step_20_of_20`.                                                                                                                                               |
| F5-03 | Scheduler: 0 zakleszczeń w 1000                         | ✅     | `scheduler-fake/tests/props.rs::zero_deadlocks_in_1000_random_scenarios`.                                                                                                                                                   |
| F5-04 | Most nie startuje z wyzwalacza 0/100                    | ✅     | `triggers-impl/tests/compliance.rs::bridge_never_starts_from_trigger_0_of_100`, `agent-backends-fake/tests/compliance.rs::bridges_never_start_from_trigger_or_improver_0_of_100`, SR2-08 `scheduler-fake/tests/review.rs`.  |
| F5-05 | Słowa wywoławcze: FAR ≤ 1/dzień                         | 🟡     | `alfa-wake-eval`; CI sprawdza mechanikę (`voice-wake-impl/tests/eval.rs::synthetic_samples_far_frr`). ≥ 24 h tła PL i wytrenowany model KWS. Moduł niepodpięty w aplikacji. Bramki #3, #8.                                  |
| F5-06 | Słowa wywoławcze: FRR ≤ 5 %                             | 🟡     | Jak F5-05; ≥ 200 pozytywów właściciela. Bramka #3.                                                                                                                                                                          |
| F5-07 | Weryfikacja właściciela: EER ≤ 3 %                      | 🟡     | `alfa-speaker-eval` (`voice-speaker-impl/tests/eval.rs::synthetic_samples_eer`); model ECAPA, Common Voice PL + TTS. Niepodpięte w aplikacji. Bramka #3.                                                                    |
| F5-08 | Weryfikacja właściciela: FAR ≤ 0,1 % przy progu ścisłym | 🟡     | Jak F5-07; ≥ 3000 prób obcych. Bramka #3.                                                                                                                                                                                   |
| F5-09 | Reguły Marszałka tylko zawężają                         | ✅     | `marshal-fake/tests/narrowing.rs::zero_widening_rules_pass_50` (0/22 rozszerzających), `effective_policy_never_exceeds_ceiling`; SR2-05 `marshal-fake/tests/review.rs`.                                                     |
| F5-10 | Dyktowanie do 5 aplikacji ≥ 95 %                        | 🟡     | Normalizacja 30/30 (`voice-dictation-contract/src/tests.rs`, `evals/F5/voice/dictation-cases.json`); 0 wpisów do okien Alfy/Brokera i pól haseł (property). Niepodpięte w aplikacji; pomiar w 5 aplikacjach na self-hosted. |
| F5-11 | Czytanie zaznaczenia w 5 aplikacjach                    | 🟡     | Testy kontraktowe `voice-readaloud-*` na `platform-fake`. Niepodpięte w aplikacji (dostępne tylko „Przeczytaj na głos” wiadomości). Self-hosted.                                                                            |
| F5-12 | Pigułka: RAM okna ≤ budżet z F0-11                      | 🟡     | Pigułka bez frameworka, ~4/8 KB gzip (`scripts/bundle-size.mjs`); RAM okna po F0-11.                                                                                                                                        |

## F6 — Computer use

| ID    | Kryterium                                               | Status | Dowód / co dalej                                                                                                                                                                                                                                                                                                                                                         |
| ----- | ------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| F6-01 | Własny zestaw ≥ 50 zadań: ≥ 85 % w każdej z 5 kategorii | ⛔     | Szkic `evals/F6/tasks/` (60 zadań, 5 kategorii po 12, JSON Schema, strony ćwiczebne `web/`; do akceptacji — m.in. ≥ 20 zadań na klasę, poziom L4, zatwierdzająca na hoście). Brak runnera F6, VM i „mózgu” (klucz albo most). Moduły gotowe i podpięte w `app-agents`: `tools-browser`, `tools-office`, `tools-vision` (OCR, opis obrazu), `tools-system`, `tools-net`, `tools-media`. Bramki #1, #8.                               |
| F6-02 | Benchmark zewnętrzny ≥ wynik modelu − 5 pp              | ⛔     | Nieprzygotowany (dostępność OSWorld / Windows Agent Arena do weryfikacji).                                                                                                                                                                                                                                                                                               |
| F6-03 | Macierz aplikacja × trasa                               | ⛔     | Szkic `evals/F6/app-matrix.md` (45 pozycji: trasa wybrana i zapasowa, oczekiwana jakość UIA, arkusz pomiaru); spike (c) niewykonany. Do pomiaru: próg `sparse` (≤ 5 węzłów łącznie z ramą okna) może nie wykrywać pustych drzew.                                                                                                                                         |
| F6-04 | Weryfikacja po każdej akcji GUI                         | ✅     | `tools-uia-impl/tests/uia.rs::actions_are_verified_and_passwords_never_set`, `tools-window-impl/tests/window.rs::move_and_state_are_verified`, `tools-input-impl/tests/input.rs`.                                                                                                                                                                                        |
| F6-05 | Deny-listy w zrzutach/OCR/wejściu: 0 naruszeń w 100     | 🟡     | Atrapy: `tools-screen-impl/tests/screen.rs::monitor_capture_is_masked_tainted_and_pixel_free_in_events`, `tools-uia-impl/tests/uia.rs::reads_are_tainted_redacted_and_hide_passwords`; Windows: `platform-windows-gui-impl` `monitor_capture_masks_and_hook_detects_user` (`#[ignore]`). 100 scenariuszy w VM; otwarte P2-01, P2-02, P2-03.                              |
| F6-06 | Zakaz `gui.control` wobec Alfy/Brokera: 0 w 50          | 🟡     | Property: `tools-input-impl/tests/input.rs::zero_input_events_reach_protected_windows`, `tools-uia-impl/tests/uia.rs::zero_uia_actions_on_protected_windows`, `tools-window-impl/tests/window.rs::zero_window_changes_on_protected_windows`, `platform-fake/tests/desktop.rs::no_effect_ever_reaches_protected_windows`. 50 prób na desktopie z oknami WebView2 (P2-01). |
| F6-07 | Helper `uiAccess`                                       | ⛔     | Helper nie istnieje (port zwraca błąd „wymaga helpera uiAccess”). Bramka #2.                                                                                                                                                                                                                                                                                             |
| F6-08 | Przejmij / Pauza / Stop ≤ 500 ms                        | 🟡     | `gui_stop`/`gui_release`, przerwanie fizycznym wejściem: `tools-input-impl/tests/input.rs::physical_input_interrupts_and_user_activity_blocks`, `platform-fake/tests/desktop.rs::physical_input_interrupts_and_blocks_start`. Spike (d) i pomiar na desktopie.                                                                                                           |

## F7 — Pamięć pełna + transfer pełny

| ID    | Kryterium                                             | Status | Dowód / co dalej                                                                                                                                                                                                                             |
| ----- | ----------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F7-01 | Izolacja pamięci: 0 przecieków                        | ✅     | `memory-contract` `contract_tests_f7::spy::spy_three_sessions` (4 sesje z prywatną, 4 agentki, 1000 zapytań) na `memory-impl/tests/f7_contract.rs` i `memory-fake`.                                                                          |
| F7-02 | recall@5 ≥ 0,85 (≥ 200 zapytań PL)                    | 🟡     | Embedder ONNX (`lib-embed`) podpięty w aplikacji (`app-models`): `[search.embedder] model`, pobranie e5-small z menedżera (zgoda TOFU), `preload` z dzierżawą RAM, wymiana embeddera w działającym `search` (`set_embedder`) i przebudowa wektorów baz sesji i zakresów pamięci w tle (`app-models/tests/embed.rs` na modelu zabawkowym: aktywacja → przebudowa → wyszukiwanie wektorowe). Próg niesprawdzony na prawdziwym modelu (HF niedostępny w chmurze; SHA-256 katalogu do przypięcia); runner `ALFA_F7_EMBEDDER`/`ALFA_F7_STRICT`, 0,964 na `HashEmbedder`; 249 zapytań niezamrożonych. |
| F7-03 | Kaskada `forget`: 100 % usunięć zweryfikowanych       | ✅     | `contract_tests_f7::forget::forget_fifty_verified`, `forget_session_cascade`, `memory-impl/tests/forget_props.rs`.                                                                                                                           |
| F7-04 | Niezaufane nie awansuje, brak auto-`remember`         | ✅     | `contract_tests_f7::access::untrusted_never_promotes`, `memory-consolidation-contract/tests/guardian.rs`, SR2-07 `agent-runtime-impl/tests/review.rs::memory_write_in_tainted_run_is_untrusted`.                                             |
| F7-05 | Konsolidacja nie na baterii / w grze (20 scenariuszy) | ✅     | `memory-consolidation-impl/tests/adapters.rs::never_starts_on_battery_or_in_game_mode_f7_05`.                                                                                                                                                |
| F7-06 | Round-trip `.alfa` pełny desktop ↔ laptop             | 🟡     | Symulacja: `memory-impl/tests/documents.rs::alfa_round_trip_desktop_to_laptop_without_loss`. Brak `evals/F7/alfa-full/` i przebiegu na dwóch maszynach. Bramka #8.                                                                           |
| F7-07 | Kopia zapasowa + restore 1:1 w CI                     | ✅     | `transfer-contract` `contract_tests::more::backup_rotation` (stan 1:1; test robi 4 kopie z rotacją do 2, ACCEPTANCE mówi o 5). Harmonogram kopii niepodpięty w aplikacji.                                                                    |
| F7-08 | Migracje schematu od F1 bez błędu                     | ⛔     | Szkic `evals/F7/migrations/` (24 scenariusze: paczki `.alfa` v0/v1, dokumenty pamięci, bazy SQLite; oczekiwania sprawdzone na kodzie 23/24), brak runnera w CI. Mechanizmy: `lib-sqlstore/tests/sqlstore.rs::migrations_apply_once_per_namespace`, upcaster `transfer-contract/src/migrate.rs`. Otwarte: powrót do starszej wersji po migracji (m-23, `UnknownMigration`). |

## F8 — Samonaprawa i ulepszanie

| ID    | Kryterium                                                        | Status | Dowód / co dalej                                                                                                                                                                                                |
| ----- | ---------------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F8-01 | ≥ 20 awarii chaosowych naprawionych i cofalnych                  | ✅     | `diagnostician-impl/tests/contract.rs::chaos_catalog_on_impl` — 24/24 wykryte, naprawione, cofnięte 1:1 (świat atrap; na prawdziwym systemie — self-hosted).                                                    |
| F8-02 | Ulepszacz nie zmienia Jądra, prywatności, budżetów, progów       | ✅     | `improver-impl/tests/contract.rs::f8_02_boundaries_hold` — 0/137 prób; SR2-04.                                                                                                                                  |
| F8-03 | Bramka z ukrytym holdoutem, N ≥ 5                                | ✅     | `evals-impl/tests/holdout.rs::catalog_never_reads_sealed_directories`, `verdict_and_events_carry_no_holdout_data`, `gate_refuses_weak_policy_and_public_manifest_as_holdout`. Otwarte P2-10 (izolacja runnera). |
| F8-04 | Zmiany R0 tylko zawężające i cofalne (50)                        | ✅     | `improver-impl/tests/contract.rs::f8_04_r0_only_narrowing_and_reversible` — 0 rozszerzających wdrożonych automatycznie.                                                                                         |
| F8-05 | Wtyczki Wasm: złośliwa zatrzymana 20/20                          | ⛔     | Moduł gotowy i podpięty (`plugin-runtime-impl`, `app-plugins`; wasmtime + WIT, ADR 12, bez WASI). W CI: `plugin-runtime-impl/tests/plugins/exec.rs::malicious_execution_is_stopped` (20/20 zatrzymanych: paliwo, czas, bomba pamięci, stos, pułapki, zły wynik), `load.rs::malicious_modules_rejected_at_load` (22/22, m.in. importy WASI), `forbidden_manifests_rejected` (11/11 manifestów). Brak zamrożonego zestawu `evals/F8/wasm-malicious/` — tworzy model-recenzent, akceptuje człowiek. |
| F8-06 | Propozycja Diagnosty: diff, uzasadnienie, ryzyko, plan cofnięcia | ✅     | 100 % kart kompletnych w `chaos_catalog_on_impl`; ocena model-recenzenta do potwierdzenia.                                                                                                                      |

## F9 — Dopieszczenie

| ID    | Kryterium                                             | Status | Dowód / co dalej                                                                                                                                                                   |
| ----- | ----------------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F9-01 | Pentest: 0 otwartych ustaleń CVSS ≥ 9                 | ⛔     | Szkic listy kontrolnej `evals/F9/pentest.md` (53 punkty: THREAT_MODEL + OWASP LLM Top 10 2025 + ASVS 5.0, CVSS 3.1, mapowanie na testy i przeglądy #1–#3 oraz fali 3; 9 luk ⛔); pentest niewykonany. Żadne otwarte ustalenie ≥ 9; wektory wstępne otwartych propozycji ≤ 7,5. |
| F9-02 | Budżety lekkości utrzymane                            | 🟡     | Jak F1-08.                                                                                                                                                                         |
| F9-03 | Budżety UI utrzymane                                  | 🟡     | Jak F1-09.                                                                                                                                                                         |
| F9-04 | Bateria: idle ~0 % CPU, bez konsolidacji/Ulepszacza   | 🟡     | Logika: konsolidacja nie na baterii (F7-05), Ulepszacz w bezczynności. Pomiar 1 h na laptopie. Bramka #8.                                                                          |
| F9-05 | Audyt WCAG 2.2 AA                                     | 🟡     | axe 0 critical/serious (F1-07), kontrast tokenów `check:contrast`. Pełny audyt AA (forced-colors, cele ≥ 24 px, `aria-live`) — model-recenzent.                                    |
| F9-06 | Dokumentacja użytkownika i ustawień dla 100 % modułów | ⛔     | Brak skryptu sprawdzającego (60 manifestów `module.toml`). Przewodnik: `docs/user-guide/`.                                                                                         |
| F9-07 | Red-team powtórzony na pełnym systemie                | ⛔     | Zależy od F3-05.                                                                                                                                                                   |

## Najważniejsze luki techniczne (dla sesji AI)

1. **Broker poza procesem — do weryfikacji na Windows.** Powłoka uruchamia procesy Jądra (`app-broker`): usługa
   `AlfaBroker` albo tryb przenośny (`alfa-broker --console`, okno Broker-UI uruchamia Broker), `alfa-watchdog` z
   `Ctrl+Shift+F12`; release bez izolowanego Brokera = bezpieczny stan. Logika i protokół przetestowane na atrapach
   i gniazdach Unix (`app-broker/tests`); prawdziwe procesy, potoki i UIPI — self-hosted. Instalacja usługi (osobne
   konto, wiązanie roli jądra z obrazem z `versions\<ver>`) — bramka #10. F3-12 i F4-03 odblokowane po teście na sprzęcie.
2. **Instalator modeli i sidecarów — dane do przypięcia.** Menedżer `app-models` (Ustawienia → „Modele i silniki”,
   onboarding) pobiera i instaluje modele (LLM, whisper, Piper, Silero, openWakeWord, mówca, embeddingi) i sidecary
   (`llama-server`, `whisper-server`, `piper`; Pocket TTS — ręcznie). Potwierdzone i przypięte są tylko pozycje
   z PyPI (Silero VAD, cechy openWakeWord); pozostałe są „do potwierdzenia przez człowieka” (adres, rozmiar,
   licencja, SHA-256, numer wydania llama.cpp, układ archiwów) — do tego czasu instalacja wymaga zgody TOFU w UI.
3. **Funkcje głosu F5 podpięte w aplikacji** (`app-voice`: słowa wywoławcze — domyślnie wyłączone, bez kalibracji
   tylko z `accept_risk`; kreator weryfikacji właściciela; dyktowanie `Ctrl+Alt+D`; czytanie `Ctrl+Alt+R`; panel Głos).
   Brakuje pomiarów FAR/FRR i EER na korpusie właściciela (bramka #3) oraz portu UIA do odczytu zaznaczenia (zapas: Ctrl+C).
4. **Embedder semantyczny podpięty** (`app-models`: wybór `[search.embedder] model`, przebudowa wektorów w tle);
   brakuje pomiaru progu F7-02 na prawdziwym modelu e5-small i przypiętych SHA-256 w `lib_embed::CATALOG`.
5. **Aktualizator**: pełny cykl (pobieranie ze wznawianiem, minisign, rollback, instalator NSIS) gotowy; brak klucza minisign i testu instalatora na Windows — F3-10.
6. **Powłoka i diagnostyka (fala 5):** dziennik diagnostyczny wszystkich procesów (`app-logs`: `%LOCALAPPDATA%\Alfa\logs\<proces>.<data>.<NNN>.log`,
   rotacja, 7 dni, `ALFA_LOG` / `[logs] level`, redakcja sekretów i pominięcie treści — test szpiegowski); `llama-server`
   CUDA z `cudart` („do potwierdzenia”) i zastępstwo CUDA → Vulkan → CPU z ostrzeżeniem; port CDP — test CI (F3-13) i odmowa
   startu przy `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` ze zdalnym debugowaniem; CSP z Trusted Types (PT-33; E2E pod CSP
   wydania). Do sprawdzenia na Windows: Trusted Types ze skryptami Tauri w WebView2, `style-src` nadal z `'unsafe-inline'`
   (bits-ui, xterm — lista w `docs/modules/ui-shell/SPEC.md`), kompilacja powłoki (job „Powłoka Tauri (Windows)”).
7. **Brakujące moduły**: helper `uiAccess` (F6-07). `plugin-runtime`, `tools-office`, `tools-browser`, `tools-vision`, `tools-system`,
   `tools-net` i `tools-media` są gotowe i podpięte w aplikacji; prawdziwy `Windows.Media.Ocr`, ffmpeg (do potwierdzenia: build i licencja)
   i odtwarzanie WASAPI sprawdzone tylko na atrapach — test na self-hosted Windows; `net_search` czeka na wybór dostawcy.
8. **Brakujące zestawy** (tworzy model-recenzent, akceptuje człowiek): `evals/F3/redteam/`, `evals/F3/mvp-scenario.md`,
   `evals/F4/bridge-fixtures/`, `evals/F7/alfa-full/`, `evals/F8/wasm-malicious/`, 100 scenariuszy F6-05. Szkice fali 4
   do akceptacji (bez `MANIFEST.json` i bez runnerów): `evals/F1/prompts-pl/` (F1-03), `evals/F6/tasks/` i
   `evals/F6/app-matrix.md` (F6-01, F6-03), `evals/F7/migrations/` (F7-08), `evals/F9/pentest.md` (F9-01).
9. **CI**: wszystkie joby zielone na `f7bd2df` (Linux, Windows, Powłoka Tauri, UI, cargo-deny, Dokumentacja); przy porażce testów log joba kończy się podsumowaniem paniki.
10. Drobne: własny pasek tytułu z Mica i Snap Layouts (spike j); generator typów TS (ADR 13) — dziś typy ręczne
    z testem round-trip DTO; eksport rozmowy do PDF (dziś MD/HTML — HTML drukowalny do PDF); zrzut ekranu z composera.
    Załączniki w composerze, eksport rozmowy MD/HTML, kopie zapasowe z harmonogramem i eksport artefaktów/logów/
    nakładki maszyny są w `app-files` (fala 3) — przeciąganie plików i dialogi powłoki do sprawdzenia na Windows.
11. **Rozmiar korzenia kompozycji (fala 4):** silnik tury czatu wydzielony z `app-core` do `app-chat` (`app-core`
    7 987 → 6 410 / 8 000 linii, `app-chat` 2 001). Domknięte podpięcia fali 3: karta „Cofnij” zapisu zmiennej
    (`UndoService::System`, token `"<sesja>:v<id>"`), kill-switch zatrzymuje odtwarzanie `media_play`
    (`AgentTools::kill_switch` → `MediaTools::stop_all`), import `.alfa` tylko jednorazowym uchwytem (dialog,
    podgląd, „Przywróć…” z listy kopii — `backups_restore`). Pełna ścieżka „agentka zapisuje zmienną → Cofnij”
    w `app-core` wymaga zgody w oknie Brokera — w testach sprawdzana na poziomie `app-agents` (`tests/registry.rs`)
    i `tools-system-impl` (`tests/undo_card.rs`); E2E z Broker-UI — self-hosted Windows.
    Przegląd fali 3 W3-04 naprawione: skażenie sesji z załączników (`meta.tainted`) zgłaszane Brokerowi przed każdą
    turą, przy otwarciu sesji i przed przebiegiem zadania — także po restarcie (`app-core/tests/files.rs::
    attachment_taint_reaches_broker_and_survives_restart`); trwały stan bezpieczeństwa w samym Brokerze — nadal P3-01.

12. **Próba generalna w chmurze (fala 6, `rehearsal.yml`) — przed testem na laptopie (RTX 4050 6 GB).** Wszystkie
    joby zielone na `cb28daa` (2026-10-06):
    - `setup-dev.ps1 -Ci` pod PowerShell 5.1 i 7 — bez błędów składni i wykonania.
    - Prawdziwa aplikacja na runnerze Windows (WebView2, Playwright przez CDP) — **10/10**: start okna, onboarding,
      rozmowa bez modelu → czytelny błąd „brak modelu”, Ustawienia → Modele i silniki (lista z katalogu), eksport
      `.alfa` przez natywne okno „Zapisz jako” (UIA + WM_CHAR/WM_COMMAND, bez fokusu), kill-switch z UI, axe na
      3 widokach, **CSP z Trusted Types: 0 naruszeń, konsola bez błędów**.
    - Silniki na żywo (CPU 4 vCPU): `llama-server` b6710, `whisper-server` 1.8.1, `piper` z prawdziwych wydań;
      Bielik 4.5B Q8_0 — 6,8 tok/s, wywołanie narzędzia z `--jinja`; Bielik 1.5B Q8_0 — 21 tok/s; architektura
      z logu serwera zgodna z manifestem (60/32 warstwy, 2 głowice KV, KV 480/256 MiB przy `-c 8192`;
      `n_ctx_train` 4.5B = 8192 → poprawiony `ctx` w manifeście); Piper RTF 0,34; whisper `small-q5_1` WER 0
      („Dzień dobry. Jestem Alfa.”), po nieudanym starcie wersji CUDA (runner bez sterownika NVIDIA) przejście na CPU.
      Rozpoznanie 2,1 s mowy na CPU trwało 71 s; po partialach adaptacyjnych (`d60660b`) — 53 s. Reszta to koszt
      samego whispera `small` na 4 vCPU (każdy przebieg liczy okno 30 s, final z wiązką 5): bez karty graficznej
      rozmowa głosowa potrzebuje mniejszego modelu STT albo innego silnika — decyzja przy pomiarach F0/F2; laptop
      i desktop liczą STT na karcie (CUDA/Vulkan).
    - **Adres modelu Bielik w katalogu był błędny (404)** — oficjalne repozytorium `speakleash` ma tylko `Q8_0`
      i `fp16`; katalog wskazuje oficjalne `Q8_0` (4.5B domyślny, 1.5B lekki), `Q4_K_M` tylko u osób trzecich —
      decyzja człowieka (ADR 0014, aktualizacja). Bramka `hf-catalog` sprawdza każdy adres i rozmiar GGUF w API
      Hugging Face. Sumy SHA-256 pobranych plików: przewodnik 11 (tabele) i log joba „Silniki na żywo”.

## Co musi zrobić człowiek (w kolejności priorytetu)

1. **Przegląd i akceptacja zmian Jądra i zabezpieczeń (bramka #7).** Moduły `safety-broker`, `broker-ui`, `watchdog`,
   `updater`, `app-safety`, `core-*` oraz poprawki z przeglądów: SR-01, SR-02, SR-03, SR-07, SR-09 (#1) i SR2-03,
   SR2-04, SR2-05, SR2-08 (#2). Bez tego zmiany nie mogą trafić na `main`.
2. **Decyzje bezpieczeństwa z przeglądów** (`docs/reviews/…-1.md` §7, `…-2.md` §7): P-01 kaskada obniżeń poziomu,
   P-03 izolacja powłoki ≤ L3 (Low IL / AppContainer), P-05 potok MCP, P-08 Authenticode i położenie obrazów Jądra,
   P-09 egress do sieci lokalnej, potwierdzenie SR-07; P2-01 strażnik GUI dla WebView2/UWP (przed włączeniem
   computer use), P2-03, P2-06 uprawnienia pamięci (rola przebiegu czy suma ról), P2-07 taint sesji w runtime,
   P2-10 izolacja runnera holdoutu.
3. **Akceptacje dokumentów (bramki #4, #7):** ADR-y 1–15 (ADR 0006 tymczasowy do spike'u g), makiety 1–3 (F0-17)
   i pozostałe makiety przed falami, które je implementują; próg F3-07 (ADR 14).
4. **Maszyny i runner (bramki #8, #9, #10):** toolchainy na desktopie i laptopie, przypięte sterowniki, wirtualny
   kabel audio, runner self-hosted na osobnym koncie Windows, VM (edycja Windows do potwierdzenia), klucz minisign,
   certyfikat podpisu, konto usługi Brokera. Odblokowuje większość pozycji 🟡.
5. **Spike'i F0 na sprzęcie** wg `evals/spikes/README.md` (kolejność f → h → e → a → j → k → b), wyniki do
   `results/` i ADR-ów; ocenia model-recenzent.
6. **Korpus własny (bramka #3)** wg `evals/spikes/e-voice-lab/protokol-nagran.md`: typy 1–9 dla F2 (≥ 300 wypowiedzi,
   ≥ 200 „stop/anuluj”, ≥ 100 backchanneli i ≥ 100 przerwań, ≥ 50 na klasę intencji, ≥ 50 „nie no, dobrze”),
   typy 10–12 dla F5 (≥ 200 pozytywów słów wywoławczych, rejestracja mówcy, dyktowanie) + ≥ 24 h tła PL;
   potem zamrożenie podziału test (`alfa-voice-eval freeze` → `evals/acceptance/F2/test.sha256`, `HASHES`).
7. **Odsłuchy i akustyka (bramki #5, #6):** ślepa ocena TTS (F0-09), ABX głosów v0 (F2-10), 1 h TTS przez głośniki
   laptopa z TV (F2-06), 30 min AEC (F0-04).
8. **Logowanie do CLI i klucze (bramka #1):** `claude` i `codex` we wbudowanym terminalu (F0-05…07, F4-01…04, nagranie
   F4-08); klucz Anthropic (spike g, ADR 0006), klucze do F1-02, F2-02 i „mózgu” F6.
9. **Zamrożenie zestawów** (status `proposed` → `frozen`, wpis `accepted_by`): `evals/F5/MANIFEST.json`,
   `evals/F5/voice/MANIFEST.json`, `evals/F8/MANIFEST.json`, `evals/F7/recall/`, `evals/F3/tools/` (ujednolicić nazwę
   z `tools-local/` z ACCEPTANCE), zestaw F2 test.
10. **UAC / Hello (bramka #2):** instalacja usługi Brokera, helper `uiAccess` (F6-07), opcjonalnie Windows Hello.
11. **Katalog modeli i sidecarów** (`crates/app-models/src/data.rs`, `lib-embed/src/catalog.rs`,
    `providers-local-impl/models.toml`): potwierdzić adresy, rozmiary i licencje (głos Piper, VoxCeleb), przypiąć
    SHA-256 i numer wydania llama.cpp, sprawdzić układ archiwów `whisper-server`/`piper` oraz `sidecar-llama-cuda`
    (wariant CUDA 12.4, archiwum `cudart`, licencja redystrybucji bibliotek NVIDIA); potem `confirmed: true`.
    Wybrać źródło modelu lokalnego: oficjalny Bielik `Q8_0` (domyślnie, 4,8 GB; na 6 GB VRAM częściowe odciążenie)
    albo kwantyzacja `Q4_K_M` osoby trzeciej (2,9 GB; zaufanie do autora kwantyzacji) — tabela SHA-256 w jobie `hf-catalog`.
