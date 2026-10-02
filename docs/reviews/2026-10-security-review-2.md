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
