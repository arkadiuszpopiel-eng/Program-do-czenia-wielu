# Przegląd bezpieczeństwa 2026-10 #2 — drugi model-recenzent

Data: 2026-10-02. Gałąź: `ccr-af4b63c6-3fyzaj` (po `4a21420`). Recenzent: drugi model w świeżym kontekście
(AGENTS.md, krok 6 — poprawność, bezpieczeństwo, zgodność ze SPEC). Punkty odniesienia: `docs/THREAT_MODEL.md`,
`docs/PLAN.md` §1.3, §7–§10, §12, `docs/ACCEPTANCE.md` (F4–F8), SPEC-i modułów, przegląd #1
(`docs/reviews/2026-10-security-review-1.md`).

## 1. Zakres i metoda

Przeczytane „jak napastnik” (agentka z wstrzykniętym poleceniem, złośliwy plik/strona/okno, złośliwy serwer MCP,
złośliwa umiejętność do importu, złośliwy wyzwalacz, sfałszowany sygnał na magistrali):
`agent-runtime-*` v1 (delegacja, `RunGrant`, Krytyczka, równoległość, `StepGate`, adapter schedulera), `skills-*`,
`agent-builder-*`, `tools-window/uia/input/screen-*`, `platform-contract/src/{gui,desktop,uia*,keys,synth*,capture,
image,pty}.rs`, `platform-windows-gui-impl`, `platform-windows-pty-impl` (FFI, `unsafe`, w tym poprawka CI
`STARTF_USESTDHANDLES` — poprawna, nie cofana), `ui-terminal-*`, `improver-*`, `diagnostician-*`, `evals-*`,
`scheduler-*`, `triggers-*`, `marshal-*`, `memory-*`, `memory-consolidation-*`, `app-bridges`, `app-tasks`,
`app-memory`, `mcp-*` (status P-05/P-10).

Każde ustalenie „naprawione” ma test reprodukujący, który **przed poprawką nie przechodził** (uruchomiony i
sprawdzony), a po niej przechodzi; testy regresyjne są w plikach `tests/review.rs` odpowiednich crate'ów. Ustalenia
bez reprodukcji (FFI Windows bez maszyny Windows, ścieżki jeszcze niepodpięte) oznaczono „podejrzenie”/„propozycja”.
Wagi: skala CVSS-podobna 0–10 (wektor lokalny, wymaga agentki z wstrzykniętym poleceniem albo złośliwej treści).

## 2. Ustalenia

| Id | Moduł | Waga | Opis | Reprodukcja | Status |
|---|---|---|---|---|---|
| SR2-01 | agent-runtime (delegacja) | 6,3 średnia | **Obejście sufitu autonomii przy delegacji.** Plan delegacji przycinał identyfikator agentki (`" delta"` → Delta), a zapytanie o jej poziom (`AutonomyOracle`/`BrokerAutonomy`) szło z nieprzyciętym — Broker zwracał poziom domyślny nieznanej agentki. Zlecająca na L3 uruchamiała podzadanie wykonywane przez agentkę z L4 („podniesienie poziomu” przez delegację, §7, S10). | `agent-runtime-impl/tests/review.rs::padded_persona_cannot_bypass_autonomy_ceiling` | naprawione: jedna funkcja wyboru wykonawczyni `delegation_target` dla planu i dla zapytania o poziom (`delegate.rs`, `child.rs`) |
| SR2-02 | skills (kwarantanna) | 4,2 średnia | **Skaner treści umiejętności omijany.** Wartości domyślne parametrów (wstawiane do celu przebiegu), przykłady i testy akceptacyjne nie były skanowane; znaki niewidoczne (U+200B, U+00AD, U+2060, U+FEFF) i zawijanie wiersza rozbijały frazy. Paczka „własna” albo zaufany wpis pamięci z poleceniem obejścia trafiał do `Proposed` zamiast kwarantanny (zatwierdzalny tekstem/głosem). | `skills-contract/tests/review.rs` (3 testy) | naprawione: skan napisów z schematu parametrów, przykładów i testów; normalizacja (znaki niewidoczne usunięte, białe znaki zwinięte) — `validate.rs::scan` |
| SR2-03 | diagnostician | 7,1 wysoka | **Sygnał wybierał dowolny klucz i wartość konfiguracji.** `diagnostics.symptom` (nadawca na magistrali nieuwierzytelniony) z `device_key`/`limit_key`/… i `fallback`/`limit_value` → `SetConfig` dowolnego klucza spoza wąskiej listy prefiksów (np. `providers.openai.base_url = https://evil…` — przekierowanie dostawcy, S14; `mcp.servers.*.command`; `roles.*.prompt`; `modules.watchdog.enabled = false`); naprawa „niskiego ryzyka” = automatyczna przy domyślnej autonomii. Lista zakazana Diagnosty była dużo słabsza niż Ulepszacza (brak `broker`, `watchdog`, `updater`, `audit`, `egress`, `net`, `budgets`, segmentów). | `diagnostician-fake/tests/review.rs` (2 testy) | naprawione: klucz z sygnału tylko w przestrzeni nazw modułu sygnału i tego samego rodzaju ustawienia, nigdy dla modułu Jądra (`plan.rs::signal_key`); wartości tylko tokenem `[a-z0-9_-]{1,32}` (`plain_token`); rozszerzone prefiksy + `DIAGNOSTICIAN_FORBIDDEN_SEGMENTS` (`step.rs`); test kontraktowy `forbidden_keys_and_conflicts` zaostrzony (klucz zakazany/`kernel.*` z sygnału ignorowany, nie trafia też do Brokera) |
| SR2-04 | improver | 3,9 niska | **Auto-wdrożenie ufało polu `auto_eligible` z trwałej kolejki.** Strażnik przed zapisem sprawdzał tylko dopuszczalność zmiany, nie kwalifikację do auto; propozycja neutralna (prompt roli, R0) ze zmienioną kolejką (`improver.json`) wdrażała się bez zatwierdzenia właściciela. Wymaga zapisu w obszarze Jądra (po SR-07 poza zasięgiem narzędzi agentek) — obrona w głąb. | `improver-impl/tests/review.rs::restored_auto_flag_does_not_bypass_approval` | naprawione: kwalifikacja (R0 ∧ zawężająca/bezpieczna) liczona na nowo tuż przed zapisem; inaczej `AwaitingApproval` (`pipeline.rs::deploy`) |
| SR2-05 | marshal | 5,4 średnia | **Zatwierdzenie po cichu zastępowało aktywną regułę.** Szkic (agentka może proponować; tłumaczenie LLM wklejonego tekstu) z identyfikatorem aktywnej reguły i niewinnym efektem („cisza nocna”) po zatwierdzeniu usuwał obowiązujące zawężenie (np. `deny_bridges`) — rozszerzenie bez cofnięcia i bez informacji o konflikcie. | `marshal-fake/tests/review.rs::approval_cannot_silently_replace_active_rule` | naprawione: szkic z zajętym identyfikatorem odrzucany przy propozycji („najpierw ją cofnij”), zatwierdzenie nigdy nie zastępuje aktywnej reguły (`book.rs`) |
| SR2-06 | app-bridges (delegacja do mostu) | 7,3 wysoka | **Most CLI uruchamiany treścią wklejoną do wiadomości.** Wzorzec delegacji nie był zakotwiczony — „przekaż Codexowi: wyślij ~/.ssh…” w środku wklejonego maila uruchamiało zadanie mostu z pochodzeniem `User` i celem autora maila (§1.3 pkt 4, S17/S18: most tylko na żądanie właściciela). | `app-bridges/tests/review.rs` (2 testy) | naprawione: polecenie tylko na początku wiadomości (opcjonalny zwrot „Delta,”/`@delta` i „proszę”) — `delegate.rs` |
| SR2-07 | agent-runtime → app-memory (S19) | 6,8 średnia | **Pranie proweniencji pamięci.** `memory_remember` w przebiegu skażonym (przeczytana strona/plik z wstrzyknięciem) zapisywał wpis jako `Provenance::Agent` (zaufany): heurystyka `untrusted_args` patrzy tylko na argumenty-cele, a `text` to treść. Zaufany wpis mógł awansować do zakresu globalnego, zasilić konsolidację i szkic umiejętności bez kwarantanny (F7-04). | `agent-runtime-impl/tests/review.rs::memory_write_in_tainted_run_is_untrusted` | naprawione: w przebiegu skażonym wywołanie narzędzia ze zdolnością `memory.write` ma `untrusted_args = true` → `app-memory` nadaje `UntrustedContent` (tylko sesja, bez awansu) — `tools.rs::prepare_call` |
| SR2-08 | scheduler (`spawn`) | 5,0 średnia (utajone) | **Agentka mogła zlecić most pod zadaniem użytkownika.** Podzadanie z wykonania dziedziczy pochodzenie rodzica; pod zadaniem `User` agentka (np. po wstrzyknięciu w czytanej treści) zlecała zadanie z wykonawczynią `Bridge` — walidacja przepuszczała (`User` może celować w most). Dziś `StepGate::spawn` nie jest wołany przez runtime (otwarte pytanie SPEC). | `scheduler-fake/tests/review.rs::spawned_subtask_cannot_target_bridge_even_under_user_task` | naprawione: `spawn` odrzuca `ExecutorKind::Bridge` (`BridgeNotAllowed`) — `engine/control.rs` |
| P2-01 | platform-windows-gui-impl / kompozycja (strażnik celów) | 5,5 | Strażnik liczy proces okna `GA_ROOT`. Wyskakujące okna WebView2 Alfy (lista `<select>`, menu kontekstowe, okna przeglądarki — proces `msedgewebview2.exe`) i okna UWP (`ApplicationFrameHost.exe` zamiast właściwej aplikacji — także deny-lista dostawców SR-09 i „zawsze zezwalaj” dla całego UWP) nie są przypisane do właściwego procesu; maskowanie zrzutów pomija takie okna. Kompozycja w toku (`app-gui::alfa_guard`, inna sesja) dodaje drzewo procesów Alfy i katalogi instalacji — ale tylko procesy istniejące przy pierwszym użyciu (`LazyWinGui`); proces WebView2 odtworzony później (awaria renderera, nowe okno) nie jest chroniony. | podejrzenie (bez Windows) | propozycja: sprawdzać `GA_ROOT`, `GA_ROOTOWNER` i proces samego okna (fail-closed — chronione, gdy którykolwiek); dla `ApplicationFrameWindow` proces okna potomnego `Windows.UI.Core.CoreWindow`; drzewo procesów Alfy liczone przy każdym sprawdzeniu (albo zdarzenie `ICoreWebView2::BrowserProcessExited`/nowy `BrowserProcessId`); test self-hosted F6-06 z `<select>` Alfy |
| P2-02 | platform-windows-gui-impl (zrzuty) | 3,0 | TOCTOU maskowania: lista okien jest brana przed `BitBlt`; okno chronione/z deny-listy (Broker-UI, szybkie okno Alfy, menedżer haseł) pojawiające się między wyliczeniem a zrzutem nie jest maskowane. | analiza | propozycja: drugie wyliczenie po zrzucie i maska z sumy (stara i nowa pozycja) |
| P2-03 | tools-input | 4,0 | `input_type_text` nie sprawdza, czy element z fokusem to pole hasła (UIA `SetValue` odmawia — niespójna reguła „agentka nie wpisuje haseł”). | analiza (atrapa nie umie oznaczyć fokusu elementu) | propozycja: `UiaPort::focused(window)` albo `UiaQuery.focused` w `platform-contract` + `platform-fake` (inna sesja) i odmowa dla `is_password` |
| P2-04 | platform-contract (`keys`) | 2,5 | Globalne skróty Alfy (`Ctrl+Alt+Space` — szybkie pytanie) działają ze wstrzykniętego wejścia mimo strażnika (aktywacja okna Alfy; dalsze paczki już odcina strażnik). | analiza | propozycja: konfigurowalne skróty Alfy w `system_scope` (lista z ustawień) |
| P2-05 | platform-windows-pty-impl | 2,0 | Lista atrybutów `PROC_THREAD_ATTRIBUTE_LIST` w `Vec<u8>` (wyrównanie 1 wg Rusta; API wymaga wyrównania wskaźnika); `write_input` trzyma zamek podczas blokującego `WriteFile` — `close()` może czekać (DoS terminala). | analiza | propozycja: bufor `Vec<usize>`; zapis bez zamka na klonie uchwytu albo z limitem |
| P2-06 | app-memory | 4,0 | Dostęp agentki do pamięci = suma uprawnień **wszystkich ról persony** w obsadzie, a nie roli bieżącego przebiegu — ograniczenie Badaczki „tylko sesja” nie działa, gdy persona gra też inne role (Gama: researcher + critic + thinker → odczyt projektu i własnej pamięci w przebiegu na treści niezaufanej). | analiza (`access.rs::RoleAccess::access`) | propozycja: dostęp z `ctx.holder.role` (rola przebiegu) ∩ role persony; decyzja semantyki ról |
| P2-07 | agent-runtime | 4,0 | Taint per przebieg, nie per sesja: kolejna tura czatu (nowy przebieg z historią zawierającą niezaufaną treść) nie jest skażona w runtime (Broker liczy taint sesji, więc egress nadal pyta), więc SR2-07 i `untrusted_args` nie działają dla treści z wcześniejszych tur. Cel podzadania pisze model rodzica, a prompt potomka nazywa go „zadaniem od właściciela”. | analiza | propozycja: `inherited_taint` z historii sesji (stan Brokera `SessionSecurity`) przy starcie przebiegu; etykieta „zadanie od agentki X” dla podprzebiegów |
| P2-08 | skills | 3,5 | `prepare_run` z rodzicem bierze kopertę z ról (nie z `caller.tools`) i taint/proweniencję z opcji startowych rodzica, nie z jego bieżącego stanu; podpięcie w toku (`app-agents::launch::skill_run` — z zadania schedulera, `parent: None`, `caller.tools` = wszystkie narzędzia zestawu). Paczka `.alfa` zaszyfrowana hasłem może pochodzić od kogoś innego, a import z dokumentu ma źródło `OwnPackage` (bez wymuszonej kwarantanny). | analiza | propozycja: przy podpinaniu — koperta `caller.tools` ∩ role, taint z checkpointu rodzica; `OwnPackage` tylko dla kopii kluczem tej maszyny |
| P2-09 | diagnostician | 4,5 | `RollbackConfig` przywraca całą rewizję konfiguracji — także klucze zakazane Diagnoście (prywatność, autonomia, budżety); port `ConfigHistory` w `app-*` jeszcze niepodpięty. | analiza | propozycja: przywracać tylko klucze spoza list zakazanych, resztę przez Brokera |
| P2-10 | evals / improver | 4,0 | Holdout: `CandidateRunner` (Jądro, `app-*`) musi uruchamiać przypadki holdoutu bez trwałych śladów widocznych dla Ulepszacza (pamięć, `core-log`, retrospektywy modelu) — inaczej przeciek holdoutu do propozycji (Goodhart, S20). Budżet zapytań holdoutu, limity dobowe i wychładzanie Ulepszacza są w pamięci — restart je zeruje. Konsument kluczy `agents.*.manifest`/`skills.*.playbook` musi walidować jak Kreator/umiejętności. Wpis słownika wymowy (R0 „bezpieczny”, auto) z modelu może zmienić znaczenie słyszanych słów. | analiza | propozycja: piaskownica runnera bez zapisu; utrwalić liczniki; auto dla słownika tylko ze źródła `rule:pronunciation` |
| P2-11 | ui-terminal / platform-windows-pty-impl | — | Przejrzane bez ustaleń: brak API dla agentek (tylko `app-*`), strumień tylko do `TerminalSink`, bufor zerowany, zdarzenia bez treści, argumenty profili stałe (brak wstrzyknięcia w wiersz poleceń `.cmd`), środowisko z listy dozwolonej, Job Object `KILL_ON_JOB_CLOSE`, dziecko bez dziedziczenia uchwytów (poprawka CI `STARTF_USESTDHANDLES` poprawna). | — | — |

Przejrzane bez ustaleń: atenuacja `RunGrant` (właściwość potomek ≤ rodzic, `is_within`), Krytyczka (tylko odczyt,
koperta ⊆ autorki, wynik autorki jako dane), adapter `RuntimeExecutor` (pochodzenie ≠ użytkownik → `Agent`, taint z
zadania, `UntrustedContent` zachowane), granica kroku i sterowanie, dzierżawy zasobów; Kreator agentów (lista
dozwolonych grup, znaczniki Jądra, sufit ≤ L3, zakresy w profilu, zapis tylko hasha przejrzanego i po teście na
sucho); strażnik celów w kontrakcie (fail-closed dla nieznanego obrazu, alias 8.3), paczki atomowe `SendInput`,
skróty systemowe, maskowanie haseł UIA i okien chronionych, odczyt UIA tylko przez wzorce, limity czasu UIA
(porzucanie wątku, `CoUninitialize` tylko po sukcesie), `VariantToInt32ArrayAlloc` + `CoTaskMemFree`, `OwnedHandle`;
wyzwalacze (pochodzenie `Trigger`, most tylko w harmonogramie użytkownika, taint z przyczyny, treść niezaufana
osobno — `goal_of` + `origin_of` = `UntrustedContent` → Broker skaża sesję); Marszałek (tylko zawężanie, zatwierdza
tylko użytkownik); pamięć (zakresy względne, `Forbidden` zamiast pustki, prywatne nie zasilają szerszych,
niezaufane tylko w sesji, konsolidacja tylko na zaufanych epizodach); bramka holdoutu (N ≥ 5, budżet zapytań,
ukrywanie klas, ścisła integralność, kanonizacja ścieżek); Ulepszacz (lista zamknięta kluczy, prefiksy i segmenty
zakazane, TOCTOU przed zapisem, zatwierdzenie dokładnie diffu z podpisem); MCP bez zmian od przeglądu #1.

## 3. Regresje względem przeglądu #1

Brak. Obszary poprawek SR-01…SR-10 (`safety-broker-*`, `compliance-contract`, `platform-windows-impl/src/fs`,
`agent-backends-impl/src/workspace.rs`, `transfer-contract`) nie zmieniły się od `1e06f1a`; testy `tests/review.rs`
z przeglądu #1 są na miejscu i przechodzą w `cargo test --workspace`. Nowe moduły nie omijają Brokera: narzędzia GUI
biorą `gui.control(<aplikacja>)`/`desktop` przez `BrokerGate`, mosty startują wyłącznie z pochodzeniem `User`/
`Schedule` (po SR2-06 i SR2-08 — tylko z jawnego polecenia), Diagnosta i Ulepszacz nie mają ścieżki do `kernel.*`
(Broker/`core-config::authorize`). Status propozycji #1: **P-05** (potok MCP przez `SecurePipePort`) — bez zmian,
nadal decyzja; **P-10** (`initialize.instructions` poza odciskiem) — nadal nieużywane w promptach (`instructions()`
nigdzie nie czytane poza `mcp-impl`), brak regresji; P-01…P-04, P-06…P-13 — bez zmian.

## 4. Reguły THREAT_MODEL dla F4–F8 → testy

| Reguła / próg | Test(y) | Luka |
|---|---|---|
| F4-04, S15: 0 odczytów poświadczeń CLI | `agent-backends-impl/tests/{rules,review}.rs`, `compliance-contract/tests/*`, `platform-windows-impl/tests/review.rs` | ETW (self-hosted), dowiązania twarde (P-04 z #1) |
| F4-07, S07/S08/S14: MCP tylko potok/stdio, hash opisów | `mcp-impl/tests/static_rules.rs`, `mcp-contract` `trust.rs` | P-05, P-10 (#1) |
| F5-01/F5-03: równoległość, brak zakleszczeń | `agent-runtime-impl/tests/parallel.rs`, `scheduler-fake/tests/{props,parallel}.rs` | — |
| F5-04, S17, §7 „most z wyzwalacza”: 0/100 | `triggers-impl/tests/compliance.rs`, `scheduler-fake/tests/fake.rs::delegation_inherits_origin_and_taint_and_bridges_stay_closed`, **SR2-08** `scheduler-fake/tests/review.rs`, **SR2-06** `app-bridges/tests/review.rs` | delegacja z czatu głosem (S04) — adresat/PTT poza tym zakresem |
| F5-09, §7 zmiany polityk: reguły tylko zawężają | `marshal-fake/tests/narrowing.rs`, **SR2-05** `marshal-fake/tests/review.rs` | egzekucja `EffectivePolicy` w Brokerze/schedulerze (otwarte w SPEC) |
| S10, §7 podniesienie poziomu: potomek ≤ rodzic | `agent-runtime-impl/tests/delegation.rs` (512 przypadków), `agent-runtime-contract` proptest atenuacji, **SR2-01** `agent-runtime-impl/tests/review.rs`, `agent-builder-contract/tests/policy.rs` (41 ataków) | P-01 (#1) |
| S09/S23 zatruta umiejętność | `skills-contract/tests/library.rs`, **SR2-02** `skills-contract/tests/review.rs`, `skills-impl/tests/module.rs` | P2-08 |
| F6-04 weryfikacja po akcji | `tools-input-impl/tests/input.rs`, `tools-uia-impl/tests/uia.rs::actions_are_verified_and_passwords_never_set` | — |
| F6-05, S26: hasła i deny-listy w zrzutach/wejściu | `platform-fake/tests/desktop.rs::capture_masks_protected_windows_passwords_and_unverified`, `tools-screen-impl/tests/screen.rs`, `tools-uia-impl/tests/uia.rs::reads_are_tainted_redacted_and_hide_passwords`, `platform-windows-gui-impl/tests/gui_windows.rs` (`#[ignore]`, Windows) | P2-01 (WebView2/UWP), P2-02 (TOCTOU), P2-03 (pisanie w pole hasła) |
| F6-06, S11, §7 `gui.control` wobec Alfy/Brokera: 0/50 | `tools-input-impl/tests/input.rs::zero_input_events_reach_protected_windows`, `tools-uia-impl/tests/uia.rs::zero_uia_actions_on_protected_windows`, `tools-window-impl/tests/window.rs::zero_window_changes_on_protected_windows`, `platform-fake/tests/desktop.rs::no_effect_ever_reaches_protected_windows`, `safety-broker-contract` `flows::kernel_blocks_on_l4` | test Windows z popupami WebView2 (P2-01), globalne skróty (P2-04) |
| F7-01 izolacja pamięci | `memory-contract` `contract_tests_f7/{spy,access}.rs` | P2-06 (suma ról) |
| F7-03 kaskada `forget` | `memory-contract` `contract_tests_f7/forget.rs` | — |
| F7-04, S19 proweniencja, brak awansu niezaufanego | `memory-contract` `contract_tests_f7/{changes,access}.rs`, **SR2-07** `agent-runtime-impl/tests/review.rs` | P2-07 (taint z wcześniejszych tur) |
| F8-01/F8-06 Diagnosta, §7 „zmiana Jądra przez Diagnostę” | `diagnostician-fake/tests/contract.rs` (24 awarie), `diagnostician-contract` `contract_tests/kernel.rs`, **SR2-03** `diagnostician-fake/tests/review.rs` | P2-09 |
| F8-02/F8-04, S20, §7 „zmiana Jądra przez Ulepszacza” | `improver-contract` `contract_tests/attacks.rs` (137 prób), `tests/guard_props.rs`, **SR2-04** `improver-impl/tests/review.rs` | P2-10 |
| F8-03 holdout | `evals-impl/tests/holdout.rs`, `evals-contract` `contract_tests` | izolacja runnera (P2-10) |
| F8-05, S09 Wasm | — | moduł `plugin-runtime` nie istnieje |
| S21 audyt, S22 pętle/budżety | jak w #1; `agent-runtime-impl/tests/loop_flow.rs`, `scheduler-fake` (budżet tła) | — |

## 5. Bramki

| Bramka | Wynik |
|---|---|
| `cargo fmt --all -- --check` | pliki tego przeglądu czyste; różnice wyłącznie w plikach równoległych sesji (`app-core`, `app-gui`, `app-health`, `app-skills`, `app-terminal`, `app-tasks/src/app.rs`, `voice-*`) |
| `cargo clippy --workspace --all-targets -D warnings` | czysto z wyłączeniem `app-core`, `app-voice`, `voice-wake-impl` — w chwili bramki nie kompilują się przez pracę równoległych sesji (`AgentKit.launch`, `voice_dsp_contract::Fbank`) |
| clippy `--target x86_64-pc-windows-msvc` | czysto: `platform-windows-gui-impl`, `platform-windows-pty-impl`, `agent-runtime-impl`, `scheduler-contract`, `improver-contract`, `marshal-contract`, `diagnostician-contract` (z `--features contract-tests` — bez niej `tests/props.rs` nie kompiluje się także na Linuksie, stan sprzed przeglądu); `skills-contract`, `app-bridges` pominięte — zależności testowe budują `openssl-sys`/`sqlite-vec`, a środowisko krzyżowe nie ma `perl` |
| `cargo test --workspace` (bez 3 crate'ów jw.) | zielono (673 binarki testów, w tym 11 plików `tests/review.rs` z przeglądów #1 i #2) |
| `cargo deny check` | advisories/bans/licenses/sources ok |
| `scripts/check-deps.sh` | OK (1195 krawędzi, zero naruszeń) |

## 6. Zmienione i nowe ścieżki (do commita przez koordynatora)

Zmienione: `crates/agent-runtime-impl/src/{child.rs,delegate.rs,lib.rs,tools.rs}`,
`crates/skills-contract/src/validate.rs`, `crates/diagnostician-contract/src/{plan.rs,plan_rules.rs,step.rs,lib.rs}`,
`crates/diagnostician-contract/src/contract_tests/kernel.rs`, `crates/improver-contract/src/pipeline.rs`,
`crates/marshal-contract/src/book.rs` (współedytowany przez sesję portów — jej zmiany: limity propozycji),
`crates/scheduler-contract/src/engine/control.rs`, `crates/app-bridges/src/delegate.rs`,
`docs/modules/{agent-runtime,skills,diagnostician,improver,marshal,scheduler,tools-input,memory}/SPEC.md` (sekcja „Przegląd bezpieczeństwa #2” na końcu; `marshal` i `scheduler` SPEC oraz `scheduler-contract` współedytowane przez sesję portów systemowych).

Nowe: `crates/agent-runtime-impl/tests/review.rs`, `crates/skills-contract/tests/review.rs`,
`crates/diagnostician-fake/tests/review.rs`, `crates/improver-impl/tests/review.rs`,
`crates/marshal-fake/tests/review.rs`, `crates/scheduler-fake/tests/review.rs`, `crates/app-bridges/tests/review.rs`,
`docs/reviews/2026-10-security-review-2.md`.

## 7. Do decyzji człowieka

1. SR2-03, SR2-04 (Diagnosta, Ulepszacz — obszar samonaprawy i bramki S20) oraz SR2-05/SR2-08 (polityki Marszałka,
   mosty) zmieniają zachowanie zabezpieczeń — przegląd człowieka zgodnie z AGENTS.md (polityki bezpieczeństwa).
2. P2-01: strażnik celów GUI dla WebView2/UWP i skład `TargetGuard` w `app-*` (PID-y WebView2, katalog instalacji)
   — przed włączeniem computer use (F6-06 na self-hosted).
3. P2-03: rozszerzenie `platform-contract` o element z fokusem (inna sesja) — reguła „agentka nie wpisuje haseł”
   dla `SendInput`.
4. P2-06: semantyka uprawnień pamięci — rola bieżącego przebiegu czy suma ról persony.
5. P2-07: taint sesji w runtime (wcześniejsze tury) — źródło prawdy: Broker (`SessionSecurity`) czy historia sesji.
6. P2-10: izolacja `CandidateRunner` dla holdoutu i trwałość liczników Ulepszacza/bramki.
7. Z przeglądu #1 nadal otwarte: P-01, P-03, P-05, P-08, P-09 (bez zmian).

## 8. Utwardzenia po przeglądzie #2

Data: 2026-10-03. Sesja utwardzająca (autonomiczna; przy wyborze wariantu — bezpieczniejszy, zgodnie z
`docs/PLAN.md` i `docs/THREAT_MODEL.md`). Zakres: P2-03, P2-01, P2-02, P2-04, P2-05, P-07 (przegląd #1), P2-07,
P2-09 i P2-06. Zmiany w kontraktach addytywne (nowe typy, metody z domyślną implementacją fail-closed, pola
`#[serde(default)]`); `app-*` — tylko podpięcia (niżej).

Metoda: test reprodukujący przed poprawką tam, gdzie lukę da się odtworzyć na atrapie (P2-02, P2-07, P2-09 —
uruchomione, **nie przechodziły**, po poprawce przechodzą). P2-01 i P2-06 — test zawiera asercję, że stary
warunek (strażnik po procesie okna, suma ról) nie chroni danego przypadku. P2-03 — test napisany po dodaniu
atrapy fokusu; przebiegu „przed poprawką” nie uruchomiono (tymczasowe wyłączenie sprawdzenia w kodzie zablokowały
zabezpieczenia środowiska) — luka wynika wprost z kodu (ani `execute_input`, ani `tools-input` nie czytały
fokusu). P2-04, P2-05, P-07 — FFI Windows: test logiki na Linuksie + test Windows (CI `windows-latest` albo
`#[ignore]` self-hosted), bez reprodukcji.

### 8.1. Poprawki

| Id | Poprawka | Test(y) | Reprodukcja |
|---|---|---|---|
| P2-03 | Tekst/dyktowanie/skróty edytujące nigdy do pola hasła. Kontrakt: `UiaPort::focused(window)` (UIA `GetFocusedElement` + `IsPassword`; domyślnie błąd = fokus nieznany), `InputBackend::focused_field()` (domyślnie `Unknown`), `FocusedField`, `batch_writes_text`, `ChordKey::edits_field` (`platform-contract/src/focus.rs`). `execute_input` odmawia **każdej** paczki wpisującej treść (Unicode, litery, cyfry, spacja, Backspace/Delete/Insert — także `Ctrl+V`, `Shift+Insert`), gdy fokus jest w polu hasła albo nieznany; Enter/Tab/strzałki dozwolone. `tools-input` sprawdza fokus po jego ustawieniu, przed wysłaniem (`Policy`). Atrapa: `FakeElement::focused()`, `FakeDesktop::focus_element`, `ScriptEvent::FocusPassword`, `UiaPort::focused` (element okna, gdy fokus nie jest w elemencie potomnym). Windows: `uia/fields.rs::{focused, focused_field}`, `WinInputBackend::focused_field` (wątek UIA z limitem; błąd = `Unknown`). Dyktowanie (`voice-dictation`) pisze przez `InputPort`, więc też jest objęte. | `tools-input-impl/tests/review.rs` (5: pole hasła, `Ctrl+V`/litery/`Shift+Insert` odrzucone a Enter dozwolony, zwykłe pole działa, fokus przechodzący do hasła w trakcie pisania — tylko 1. paczka, fokus nieznany — port bez odczytu i UIA zawieszone); `platform-contract/src/synth_tests.rs::nothing_is_typed_into_password_fields_or_unknown_focus`; `platform-fake/tests/review_contract.rs` (2); Windows `#[ignore]` `gui_windows.rs::child_process_windows_of_alfa_are_protected_and_focus_is_read` | analiza (jw.) |
| P2-01 | Strażnik celów: `ProcessLink` (rola, PID, obraz, przodkowie) + `TargetGuard::{is_protected_link, is_protected_window, check_window, is_protected_root}` (`platform-contract/src/target.rs`). Okno chronione, gdy chroniony którykolwiek proces powiązany: okno, `GA_ROOT`, łańcuch `GW_OWNER`/`GA_ROOTOWNER`, treść UWP (proces `Windows.UI.Core.CoreWindow` w `ApplicationFrameWindow`; ramka bez treści = `UwpUnresolved`, chroniona), a przodek w drzewie bieżącego procesu albo PID-u z `pids` = chroniony. **Drzewo procesów z migawki Toolhelp32 przy każdym sprawdzeniu** (lista okien, fokus/położenie/stan, każda paczka wejścia, UIA: okno i proces każdego elementu) — nie tylko przy starcie `LazyWinGui` (`platform-windows-gui-impl/src/links.rs`). Okno UWP ma obraz aplikacji (zdolność `gui.control(<aplikacja>)`, deny-lista dostawców SR-09, „zawsze zezwalaj” nie obejmuje całego UWP). `TargetWindow.links` (`serde(default)`). Atrapa: tabela procesów, `FakeWindow::{in_process, child_of, owned_by, uwp}`, `FakeDesktop::add_process`, ochrona liczona przy każdym sprawdzeniu, `GuiRecord.links`. | `platform-fake/tests/review.rs::zero_effects_in_alfa_popups_uwp_and_respawned_webview` (proptest **500** przypadków: wejście, kliknięcia, fokus, położenie, stan, UIA, wyskakiwanie okien w trakcie, renderer WebView2 odtworzony w trakcie — 0 skutków w oknach Alfy, wszystkie oznaczone `protected`), `old_check_by_window_process_alone_missed_these_windows` (stary strażnik nie chronił ≥ 3 okien sceny; UWP z aplikacją — obraz aplikacji, sterowalne); `review_contract.rs` (2); `synth_tests.rs::linked_alfa_processes_stop_input`; Windows `#[ignore]` (dziecko procesu = chronione) | asercja starego warunku w teście |
| P2-02 | Zrzut: okna → klatka → okna ponownie; zbiór okien istotny dla maskowania w obszarze zmieniony (`capture_set_stable`: id, prostokąt, stan, ochrona, obraz) → klatka odrzucona i ponowiona (`CAPTURE_ATTEMPTS = 3`); po wyczerpaniu prób maska z sumy wyliczeń (`union_for_mask` — stara i nowa pozycja okien chronionych/maskowanych) i okna zmienione maskowane w całości w obu położeniach (`unstable_masks`, `Unverified`) — `platform-contract/src/capture_check.rs`, `platform-windows-gui-impl/src/{backend,capture}.rs` (`grab` + `finish`). Atrapa w tej samej kolejności co Windows (`FakeDesktop::show_during_capture`). | `platform-fake/tests/review.rs::protected_window_appearing_during_capture_is_masked` (Broker-UI w trakcie zrzutu; okno zmieniane przy każdej próbie); `review_contract.rs::capture_set_detects_appearing_and_moving_windows` | **tak** — przed poprawką piksel Broker-UI niezamaskowany |
| P2-04 | Skróty globalne z wejścia wstrzykniętego: hook `WH_KEYBOARD_LL` wątku skrótów zapisuje pochodzenie wciśnięć (`LLKHF_INJECTED`/`LLKHF_LOWER_IL_INJECTED`, czas) i puszczenia; `WM_HOTKEY` liczy pochodzenie kombinacji (klawisz główny w oknie `HOOK_WINDOW_MS` + modyfikatory; którykolwiek wstrzyknięty = wstrzyknięte) i odrzuca naciśnięcie, jeśli `HotkeyPressOrigin::admits` = `false`: **kill-switch zawsze**, pozostałe (szybkie pytanie `Ctrl+Alt+Space`, PTT — także łańcuch „wstrzyknięty PTT + własny TTS = polecenie głosowe”) tylko z wejścia fizycznego; nieznane pochodzenie przyjmowane tylko przy oknie podniesionym na pierwszym planie (hook go nie widzi, a UIPI blokuje wtedy `SendInput` procesów zwykłej integralności). **Gdzie wymuszone:** `platform-windows-impl/src/hotkey/thread.rs::on_hotkey` (zdarzenie nie trafia do kolejki `HotkeyPort`), logika `hotkey/origin.rs`, polityka `platform-contract/src/hotkey.rs::HotkeyPressOrigin::admits`; atrapa `FakeHotkeys::press_injected`. | `platform-windows-impl` `hotkey::origin::tests` (fizyczne/wstrzyknięte/nieznane, modyfikator z niższej integralności, puszczenie, zawinięcie zegara), `platform-contract` `hotkey::tests::injected_presses_are_ignored_except_kill_switch`, `platform-fake` `hotkeys::tests` | analiza (Windows) |
| P2-05 | ConPTY: lista atrybutów w `Vec<usize>` (wyrównanie wskaźnika; `attrs.rs::AttrList`, `DeleteProcThreadAttributeList` w `Drop` — także gdy `UpdateProcThreadAttribute` zawiedzie); `write_input` klonuje uchwyt (`Arc`) pod zamkiem i pisze **bez** niego, porcjami 4 KiB, przerywa po `close()`; kolejność zapisów osobnym zamkiem, którego `close()` nie bierze. | `platform-windows-pty-impl` (Windows CI): `conpty::tests::close_never_waits_for_a_blocked_writer`, `attrs::tests::buffer_is_word_aligned_and_large_enough` | analiza |
| P-07 (#1) | Start Broker-UI: potok bez dziedziczenia, dziedziczny tylko koniec do odczytu, `STARTUPINFOEXW` + `EXTENDED_STARTUPINFO_PRESENT` + `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` z jednym uchwytem (bufor wyrównany) — `bInheritHandles = TRUE` ograniczone do jawnej listy (`platform-windows-kernel-impl/src/win_launch.rs`). Reszta ryzyka: inny wątek usługi tworzący w tym czasie proces z dziedziczeniem bez listy mógłby odziedziczyć koniec potoku (usługa dziś takich nie tworzy). **Ścieżka Jądra — przegląd człowieka.** | `win_launch::tests::startup_info_is_extended_with_aligned_attribute_list` (Windows CI; start wymaga `SeTcbPrivilege`) | brak (FFI usługi) |
| P2-07 | Skażenie **sesji**, monotoniczne: kontrakt `SessionTaint` + `MemorySessionTaint` + `TaintReset` (`agent-runtime-contract/src/taint.rs`); runtime dziedziczy taint sesji przy każdym starcie pętli (`Shared::launch`: kolejna tura czatu, delegacja, Krytyczka, zadanie schedulera, wznowienie), a każde `Tainted` skaża sesję (`RunHandle::record`). W aplikacji źródło prawdy = Broker: `BrokerSessionTaint` (`SessionSecurity` + rejestr procesu; podpięte w `app-agents::Launch`). Reset wyłącznie jawnie przez właściciela gestem nie-głosowym (`TaintReset::by_owner` przyjmuje tylko `UserText`; głos/agentka/treść niezaufana — odmowa; `Runtime::reset_session_taint`) — przy tainted Brokerze w praktyce dopiero nowa sesja. Cel podprzebiegu w prompcie: „zlecone przez inną agentkę”, nie „od właściciela”. **Decyzja do potwierdzenia przez człowieka** (SPEC `agent-runtime`, `safety-broker`). | `agent-runtime-impl/tests/review.rs::next_turn_in_tainted_session_inherits_taint` (+ inna sesja nie dziedziczy), `agent-runtime-contract` `taint::tests` | **tak** — przed poprawką zapis pamięci w kolejnej turze `untrusted_args = false` |
| P2-09 | Diagnosta nie przywraca całej rewizji: „ostatnia dobra rewizja” = `SetConfig` (porównaj-i-zamień) tylko dla kluczy dozwolonych z różnicy rewizji (`RepairContext::revision_diff`, domyślnie brak → bez rollbacku, wyłączenie modułu), klucze Jądra/zakazane zostają + zadanie dla właściciela; wykonawca odrzuca propozycję z `RollbackConfig` (`needs_human`). | `diagnostician-fake/tests/review.rs::rollback_never_restores_keys_forbidden_to_diagnostician`; katalog 24 awarii i testy kontraktowe zielone | **tak** — przed poprawką `privacy.cloud_allowed` wracało do `true` |
| P2-06 | Pamięć w przebiegu: zakresy **roli bieżącego przebiegu** (`holder.role`) ∩ role persony; rola spoza obsady/brak = tylko sesja (wariant ściślejszy; `app-memory::RoleAccess::access_for_run`, `run_grants`). Kontekst pamięci czatu (poza narzędziami) — bez zmian (suma ról). **Semantyka do potwierdzenia przez człowieka** (SPEC `memory`). | `app-memory/src/access.rs::run_role_not_sum_of_persona_roles` | asercja starego warunku (suma ról Gamy daje odczyt projektu) |

Podpięcia w `app-*` (minimalne): `app-gui/src/ports.rs` — `LazyWinGui` przekazuje `UiaPort::focused` (bez tego
domyślna metoda = fokus nieznany i wpisywanie byłoby odrzucane); `app-agents/src/launch.rs` — runtime z
`BrokerSessionTaint`, gdy jest Broker; `app-memory/src/{access,tools}.rs` — P2-06.

Skutek uboczny P2-01 (bezpieczny kierunek): okna procesów uruchomionych przez Alfę jako dzieci (także przez
powłokę agentki) są chronione — tak jak dotąd procesy istniejące przy pierwszym użyciu `LazyWinGui`. Test Windows
Notatnika uruchamia go przez `cmd /c start` (poza drzewem procesu testu).

### 8.2. Statusy pozostałych propozycji

| Id | Status |
|---|---|
| P2-08 (skills: koperta i taint przy podpinaniu, źródło `.alfa`) | otwarte, bez zmian (podpięcie w `app-agents::launch::skill_run` — przy kolejnej zmianie umiejętności; taint rodzica przy uruchomieniu w tej samej sesji obejmuje teraz P2-07) |
| P2-10 (holdout `CandidateRunner`, trwałość liczników) | otwarte, bez zmian — decyzja (izolacja runnera w `app-*`) |
| P-01 (kaskada obniżeń autonomii) | bez zmian — decyzja człowieka (semantyka ADR 15) |
| P-03 (izolacja powłoki ≤ L3: Low IL / AppContainer) | bez zmian — decyzja architektoniczna (THREAT_MODEL §11) |
| P-05 (asynchroniczny potok MCP) | bez zmian — wymaga zmiany kontraktu `platform-contract` (osobna sesja MCP) |
| P-07 | **naprawione** (wyżej) |
| P-08 (Authenticode, obrazy Jądra w Program Files) | bez zmian — bramka #10 (certyfikat) |
| P-09 (egress do sieci lokalnej) | bez zmian — decyzja polityki egressu |
| P-02, P-04, P-06, P-10…P-13 | bez zmian |

### 8.3. Bramki

| Bramka | Wynik |
|---|---|
| `cargo fmt --check` (crate'y tej sesji) | czysto |
| `cargo clippy --workspace --all-targets -D warnings` | czysto (cały workspace, 2026-10-03); po ostatnich poprawkach ponownie per crate — czysto, także `diagnostician-contract --features contract-tests` |
| clippy `--target x86_64-pc-windows-msvc --all-targets -D warnings` | czysto: `platform-contract`, `platform-windows-gui-impl`, `platform-windows-impl`, `platform-windows-kernel-impl`, `platform-windows-pty-impl` |
| `cargo test` crate'ów zmienionych i zależnych | zielono: `platform-contract`, `platform-fake`, `platform-windows-{gui,kernel,pty}-impl`, `platform-windows-impl`, `tools-input-impl`, `tools-{window,uia,screen}-impl`, `voice-{dictation,readaloud}-impl`, `agent-runtime-{contract,impl}`, `diagnostician-{contract (contract-tests),fake,impl}`, `app-{gui,agents,memory,health}` — 89 binarek testów, 309 testów, 0 porażek. Pełny `cargo test --workspace` przerwany na polecenie koordynatora (kompilacja crate'ów równoległych sesji) — do uruchomienia przy commicie |
| `cargo deny check` | advisories/bans/licenses/sources ok |
| `scripts/check-deps.sh` | jedno naruszenie poza zakresem tej sesji: `lib-embed` → `model-residency-fake` (dev) — nowy crate równoległej sesji |
| Limity rozmiaru | pliki ≤ 400 linii; `platform-contract` 7 8xx linii `.rs` (testy logiki P2-01/02/03 w `platform-fake/tests/review_contract.rs`, by zmieścić się w 8 000) |

### 8.4. Zmienione i nowe ścieżki (do commita przez koordynatora)

Zmienione: `crates/platform-contract/src/{hotkey.rs,lib.rs,synth.rs,synth_tests.rs,uia.rs}`,
`crates/platform-fake/src/{hotkeys.rs,lib.rs}`, `crates/platform-fake/src/desktop/{capture.rs,input.rs,mod.rs,uia.rs,windows.rs}`,
`crates/platform-windows-gui-impl/{Cargo.toml,tests/gui_windows.rs}`,
`crates/platform-windows-gui-impl/src/{backend.rs,capture.rs,desktop.rs,input.rs,lib.rs,portable.rs,win.rs}`,
`crates/platform-windows-gui-impl/src/uia/{fields.rs,mod.rs,read.rs}`,
`crates/platform-windows-impl/src/hotkey/{mod.rs,thread.rs}`, `crates/platform-windows-impl/src/process/mod.rs`,
`crates/platform-windows-kernel-impl/src/win_launch.rs`, `crates/platform-windows-pty-impl/src/{conpty.rs,lib.rs}`,
`crates/tools-input-impl/src/{lib.rs,plan.rs}`, `crates/agent-runtime-contract/src/lib.rs`,
`crates/agent-runtime-impl/src/{engine.rs,handle.rs,lib.rs,prompt.rs,shared.rs}`, `crates/agent-runtime-impl/tests/review.rs`,
`crates/diagnostician-contract/src/{exec.rs,lib.rs,plan_rules.rs,ports.rs}`, `crates/diagnostician-fake/src/world.rs`,
`crates/diagnostician-fake/tests/review.rs`, `crates/app-gui/src/ports.rs`, `crates/app-agents/src/launch.rs`,
`crates/app-memory/src/{access.rs,tools.rs}`,
`docs/modules/{tools-input,platform-windows,agent-runtime,safety-broker,diagnostician,memory,ui-terminal}/SPEC.md`,
`docs/reviews/2026-10-security-review-2.md` (ta sekcja).

Nowe: `crates/platform-contract/src/{focus.rs,target.rs,capture_check.rs}`, `crates/platform-fake/src/desktop/procs.rs`,
`crates/platform-fake/tests/{review.rs,review_contract.rs}`, `crates/platform-windows-gui-impl/src/links.rs`,
`crates/platform-windows-impl/src/hotkey/origin.rs`, `crates/platform-windows-pty-impl/src/attrs.rs`,
`crates/tools-input-impl/tests/review.rs`, `crates/agent-runtime-contract/src/taint.rs`.

Bez zmian w `Cargo.lock` (nowa cecha `Win32_System_Diagnostics_ToolHelp` tej samej wersji `windows` w
`platform-windows-gui-impl`), `deny.toml`, root `Cargo.toml`, `crates/README.md`.

### 8.5. Do decyzji człowieka

1. P2-07: skażenie sesji w runtime (Broker jako źródło prawdy, reset tylko gestem nie-głosowym; przy taincie
   Brokera — nowa sesja) i etykieta celu podprzebiegu.
2. P2-06: uprawnienia pamięci wg roli bieżącego przebiegu (wariant ściślejszy) zamiast sumy ról persony.
3. P-07 i P2-04: zmiany w ścieżkach Jądra (`platform-windows-kernel-impl` — start Broker-UI; kill-switch i skróty
   globalne w `platform-windows-impl`) — przegląd człowieka zgodnie z AGENTS.md.
4. P2-01: okna procesów potomnych Alfy (także uruchomionych przez powłokę agentki) poza zasięgiem `gui.control`;
   minimalizowane okna UWP (ramka bez `CoreWindow`) — chronione do czasu przywrócenia przez właściciela.
5. P2-04: przy oknie administratora na pierwszym planie skróty Alfy (poza kill-switchem) działają tylko, gdy
   pochodzenie jest nieznane i okno jest podniesione — potwierdzić na self-hosted (UIPI a hook LL).
6. Nadal otwarte z #1/#2: P-01, P-03, P-05, P-08, P-09, P2-08, P2-10.
