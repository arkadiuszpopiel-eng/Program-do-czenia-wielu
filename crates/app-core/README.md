# app-core — korzeń kompozycji Alfy

Biblioteka bez zależności od Tauri. Składa moduły `-impl` i wystawia **każdą komendę z
`apps/desktop/ui/src/lib/api/COMMANDS.md`** jako metodę `AppCore::<przestrzeń>_<nazwa>` oraz strumień
zdarzeń `alfa://events` (paczki `AlfaEvent[]`, najwyżej jedna na klatkę ≈ 16 ms). Powłoka
`apps/desktop/src-tauri` tylko deleguje (handlery generuje makro `app_core::with_commands!`).

Kategoria `app-*` (crates/README.md, `scripts/check-deps.sh`): jedyne crate'y, które mogą zależeć od
`*-impl`; od `*-fake` tylko w `dev-dependencies`; `app-*` mogą zależeć od siebie nawzajem, ale od żadnego
`app-*` nie zależy crate spoza tej kategorii. Ze względu na limit 8 000 linii korzeń jest rozcięty:

| Crate | Zawartość |
|---|---|
| `app-api` | kontrakt IPC: DTO (`dto/*`), lista komend `with_commands!` / `COMMANDS` / `CHANNEL_COMMANDS` (`commands.rs`), `AppError`, identyfikatory DTO (`ids`), `EventHub`, porty (`ports/*`), `AppPaths`, protokół `alfa://`, powiadomienia; zależy tylko od `*-contract` |
| `app-modules` | adaptery portów na modułach `-impl`: `TransferAdapter`, `InprocBroker`, `VoiceAdapter`, `tts::engines`, katalog dostawców (`catalog`), sonda kont (`probe`), Router (`route/*`), sejf kluczy (`secrets`), późne wiązanie `sessions ↔ search` (`late`), osadzacz leksykalny (`embedder`) |
| `app-memory` | pamięć F7: `MemoryModule` (bazy zakresów w `%LOCALAPPDATA%\Alfa\memory`, klucze w sejfie), dostęp agentek z ról i projektu sesji (`RoleAccess`), narzędzia `memory_recall`/`memory_remember`, Strażniczka pamięci (`guardian`), `MemoryApp` (komendy Inspektora, kontekst czatu, zapomnienie sesji) |
| `app-tasks` | `scheduler-impl` (zamiast `scheduler-lite-impl`), `triggers-impl`, `marshal-impl`; wykonawczyni zadań (agentka przez `RuntimeExecutor`, most CLI przez `agent-backends`), Replay zadań, mosty zdarzeń, `TasksApp` (komendy `tasks_*`/`triggers_*`/`marshal_*`), delegacja z czatu |
| `app-bridges` | mosty CLI: karty zgodności (`compliance`), wykrywanie CLI, przypięcia wersji i zgody na harmonogram (`agent_backends.*`), „Zaloguj w terminalu", backend `agent-backends-impl`, serwer MCP Alfy na żądanie (`LazyMcpHost`), rozpoznanie delegacji |
| `app-agents` | agentki z narzędziami: zestaw `tools-fs/shell/clipboard` (+ GUI z `app-gui`) (`AgentTools`), `RunSpec` z obsady i ustawień, `Launch` (`start_with` z `Crew`: delegacja, Krytyczka, umiejętności jako podprzebiegi), `RunHandle` + `RunFamily` (przebieg z podprzebiegami), projekcja `agent.*` → Replay/karty UI (`RunProjector`, `FamilyProjector`), `TicketLog`, eval narzędzi F3 (`eval`) |
| `app-gui` | computer use: porty `WinGui` ze strażnikiem okien Alfy, narzędzia `tools-window/uia/input/screen` (`WatchedTool`), panel „Ekran" (`GuiMonitor`: akcje bez treści, zrzut tylko w pamięci, przejęcie), „zawsze zezwalaj na podgląd pulpitu" przez Brokera |
| `app-terminal` | wbudowany terminal: `TerminalApp` (komendy `terminal_*`, gest tylko z UI), strumień do `FrameSink` (w powłoce `Channel`) |
| `app-skills` | umiejętności (`SkillsApp`: przegląd z diffem i hashem, kwarantanna, uruchomienie = zadanie) i Kreator agentek (`BuilderApp`: podgląd, test na sucho, zapis) |
| `app-health` | „Zdrowie systemu": Diagnosta, Ulepszacz, evale (`HealthApp`, `HealthChanged`) |
| `app-updates` | aktualizacje i „O programie” (`UpdatesApp`, `updates_*`, `UpdateStatus`, `mark_good`, restart przez launcher) |
| `app-models` | menedżer modeli i silników (`ModelsApp`: katalog, pobieranie z wznawianiem, SHA-256 / zgoda TOFU, bezpieczne ZIP; komendy `models_*`), embedder wyszukiwania (`startup_embedder` przy budowie `search`, `embed_model_activate`, przebudowa wektorów `search_reindex_*`, `ReindexStatus`) |
| `app-store` | tabele aplikacji w bazach sesji (`AppStore`: fakty tur, oś czasu, katalog roboczy, przebiegi agentek) |
| `app-voice` | tryb głosowy: `PipelineVoice` (port głosu z pętlą `voice-pipeline`), `ChatReply` (`ReplySource` na czacie sesji), pigułka, `SystemVoice` (produkcyjna fabryka potoku z modeli i sidecarów) |
| `app-core` | kompozycja (`AppOptions` → `parts`), komendy, czat (z delegacją do mostu), `TaskHost` rdzenia (`host.rs`); reeksportuje moduły `app-api` pod starymi ścieżkami (`app_core::dto`, `app_core::ports`, …) |

## Kompozycja (`AppCore::build(AppPaths, AppOptions)`) — jedno miejsce: `parts/`
1. Jądro (`parts/kernel.rs`): magistrala (`core-bus-impl`), rejestr (`core-registry-impl`), sekrety
   (`SecretStore`: Windows Credential Manager; poza Windows — pamięć procesu), profil urządzenia →
   `MachineId`, konfiguracja (`core-config-impl`, JSON Schema stron ustawień z
   `data/settings-pages.json`), logi NDJSON z magistrali (`core-log-impl`), zdarzenia UI (`EventHub`),
   licznik kosztów.
2. Rejestr dostaje manifesty (`module.toml`) wszystkich modułów i wylicza `start_order()`; każdy moduł
   jest budowany i startowany (`Module::start` z magistralą) **dokładnie w tej kolejności**, a w rejestrze
   zostaje pośrednik (manifest + zdrowie usługi). Poprawki grafu (`compose.rs`): cykl
   `sessions ↔ search` rozcięty (`LateIndexer`); `providers-local` nie jest drugim dostawcą
   `providers-contract` (obaj są kandydatami Routera); Broker w procesie startuje bez `watchdog`.
3. Moduły F1: platform (manifest), device-profile, compliance, accounts-hub (sonda kont = `list_models`
   adapterów), providers-api, cost-meter (kurs NBP przez `reqwest`/rustls), sessions (`KeyVault` na
   `SecretStore`), search (osadzacz leksykalny `alfa-lexical-hash-v1` do czasu ONNX), memory (F7, z
   `memory-consolidation` po Routerze), artifacts, personas, scheduler (`scheduler-impl`: ta sama tablica
   blokad mowy dla głosu i zadań; stan w `%LOCALAPPDATA%\Alfa\scheduler`), triggers, marshal,
   agent-backends i mcp (leniwie — przy pierwszym zadaniu mostu). Katalog dostawców wbudowany (`providers-catalog/*.toml`, `include_str!`).
4. Moduły podpięte po F1 (`parts/extra.rs`; błąd budowy = moduł niezdrowy w rejestrze, nie błąd startu):
   model-residency (budżety z profilu urządzenia), providers-local (sidecar `llama-server` z
   `AppPaths::sidecar`), router, risk-classifier, safety-broker (w procesie, tryb deweloperski: audyt
   `local/broker-dev/audit.ndjson` + kotwica), undo-journal (`local/undo-store`), transfer
   (`DirDocumentStore` konfiguracji i logów, snapshoty w `local/snapshots`), voice-audio
   (`AppOptions::audio` albo WASAPI), voice-tts (sidecary Pocket TTS / Piper, jeśli zainstalowane),
   updater (`FsUpdater` w katalogu instalacji launchera; `mark_good` po `healthy_after` ≈ 30 s zdrowego startu —
   `app-updates`).

5. Agentki i głos (`parts/agents.rs`): `AgentTools` (+ narzędzia GUI z `app-gui` dla ról z `gui.control`)
   nad Brokerem (`TicketLog`), dziennikiem cofania,
   `FsPort`/`ExecPort` z `platform-windows` (**ta sama instancja `ExecPort` zabija procesy w Brokerze**
   — kill-switch obejmuje Job Objects poleceń) i deny-listą bazową; port głosu `PipelineVoice` z
   fabryką `AppOptions::voice_engine` albo `SystemVoice` (bez modeli/sidecarów — stan „głos
   niedostępny: pobierz modele w Ustawieniach → Głos").
6. F8 (`parts/work.rs`, `parts/signals.rs`): `GuiApp` (`AppOptions::gui` albo `WinGui`), `TerminalApp`
   (`AppOptions::pty` albo ConPTY; programy z `cli_probe`), `app_skills::open` (umiejętności wiązane
   z `Launch` agentek), `HealthApp` (Diagnosta z kotwicami Jądra `broker-dev` i `versions`, Ulepszacz
   z mózgiem rdzenia); sygnały systemowe (`AppOptions::signals` albo `WinSignals`: bezczynność →
   Strażniczka, okna zadań, cykl Ulepszacza; tryb gry) i obserwacja katalogów (`AppOptions::dir_watch`
   albo `WinDirWatch` → wyzwalacze plikowe). Zdrowie `agent-runtime`/`tools-*` ustalane po złożeniu
   stosu agentek (bez fałszywych awarii z kolejności startu); usługi `providers-api` i `memory`
   trzymane przez cały czas życia rdzenia. Błędy HTTP dostawcy tury (bez Routera) → `diagnostics.symptom`.

## Agentki z narzędziami (`chat/agent.rs`, `app-store`, `commands/workdir.rs`)
Wiadomość do agentki idzie przez `agent-runtime`, gdy sesja ma katalog roboczy (`sessions_choose_workdir`:
dialog powłoki / katalog sesji / brak; katalogi danych Alfy, deny-lista i segmenty poświadczeń
odrzucane) i role agentki dają narzędzia; inaczej zwykły czat. Budżety z Ustawień → Agentki
(`agents.*`); bez okna Brokera czekanie na zgodę ≤ 60 s, potem odmowa z powodem dla modelu
(`AppOptions::approval_timeout` — testy). Zdarzenia: `AgentRunUpdated`, `AgentStep` (Replay, trwały w
`app_agent_runs/steps`, append-only), `ToolCall` z „Cofnij" i intencją, `ApprovalPending`
(`broker_window`, `expires_at`). Komendy: `agents_runs`, `agents_steer` (wiadomość w trakcie zadania),
`agents_open_terminal` (terminal w katalogu kroku, bez wykonania), `turns_undo_step` (dziennik albo
schowek `"<sesja>:c<id>"`). Stop/Esc i kill-switch anulują przebieg i polecenia.

## Pamięć, zadania, mosty CLI (`parts/memory.rs`, `parts/tasks.rs`, `host.rs`, `chat/delegate.rs`)
- Pamięć: `turns_remember` → zakres sesji/projektu/agentki/globalny (z sesji prywatnej — tylko sesja);
  kontekst czatu = zestaw roboczy agentki (dostęp z ról obsady i projektu sesji); narzędzia
  `memory_recall`/`memory_remember` w zestawie agentek; `memory_*` — Inspektor; usunięcie sesji:
  `forget_as(Session)` (kopie w zakresach szerszych też) przed crypto-shreddingiem. Strażniczka: model
  lokalny przez lokalny rdzeń Routera, budżet tła, licznik bezczynności — port bez platformy
  (`UnknownIdle`: „nigdy bezczynny", porządkowanie nocne startuje tylko ręcznie).
- Zadania: `tasks_*` (DAG, sterowanie, pauza, anulowanie z poddrzewem, ponowienie), wykonawczyni wiązana
  po złożeniu rdzenia (`TaskBinder`, rdzeń trzymany słabo); zadanie bez sesji → sesja „Zadania w tle"
  (`tasks.background_session`, warstwa maszyny). Obsada schedulera z `Roster::from_cast` (obsada
  domyślna) i `scheduler.max_parallel`; warunki okien: tryb gry z `model-residency`, bezczynność —
  „nigdy". Kill-switch zatrzymuje też zadania (`SchedulerLite::kill_all`).
- Wyzwalacze (`triggers_*`): czas/zdarzenie/ręczne; obserwacja katalogów — `NoFileWatch`
  (`watch_unavailable`), DND z `voice-wake`. Marszałek (`marshal_*`): tłumacz przez Router
  (`LlmTranslator`, wiązany po portach), reguły tylko zawężają, zatwierdza wyłącznie UI; polityka →
  limit równoległości, zakaz mostów, budżety zadań użytkownika; raport dnia → `MarshalReportReady`.
- Mosty (`bridges_*`): karty zgodności, zgoda na harmonogram (≤ 24/dobę), przypięcie wykrytej wersji,
  „Zaloguj w terminalu" (polecenie do skopiowania; Alfa nie czyta tokenów CLI). Delegacja z czatu
  („Delta, zleć to Claude Code") = zadanie mostu od użytkownika; prośby o uprawnienia mostu → Broker
  (`BrokerSink`, kopie robocze pod `%USERPROFILE%\Alfa\Mosty`), Replay „niezweryfikowane przez Alfę".
  `AppOptions::bridges` (fabryka nad kanałem zatwierdzeń) i `AppOptions::cli_probe` — testy.

## Tryb głosowy (`voice_chat.rs`, `app-voice`)
`voice_set_mic_enabled/ptt/set_muted/stop_speech/status/preview`; wypowiedź → tura użytkownika w
aktywnej sesji (albo nowej sesji „Asystent głosowy"), odpowiedź strumieniuje się do UI i do TTS;
barge-in → `record_heard_prefix` (tura agentki ma `heard_prefix`, kolejne żądanie widzi, co
użytkownik usłyszał). Pigułka (`VoicePill`: kto mówi, poziom, transkrypt częściowy), `MicLevel` ≤ 30/s,
`VoiceStatusChanged`; „stop wszystko" głosem = kill-switch, „anuluj" = Stop aktywnej sesji.

## Router (`route/`)
Trzy rdzenie `router-impl`: **hybryda** (API + lokalne), **chmura** (tylko API) i **lokalny** (tylko
lokalne — profil `Local` albo sesja z tagiem `LocalOnly`). Kandydaci = konta z `accounts-hub`
(`providers-api-impl`, przypisane agentce najpierw) + modele lokalne (`providers-local-impl`);
tabela tras z konfiguracji (`router.*` → nadpisania TOML `[router]`, obserwowane na żywo), budżet z
`cost-meter` (`CostMeterGate`), zgodność z `compliance` (`route_allowed`, sesja prywatna — tylko
dozwolone trasy). Fallback przed pierwszą treścią (5xx/limit → następny kandydat; UI dostaje jedną
odpowiedź). Decyzja `router.decision` → oś czasu („Router → …", zapasowe/odrzucone) i kapsuła
aktywności („Odpowiada X · model (lokalnie/przez API)"). „Brak mózgu" (`NO_BRAIN`) tylko gdy rdzeń nie
ma żadnego kandydata; w profilu lokalnym bez modelu — `NO_LOCAL`. Pobieranie modelu lokalnego:
`models_local_list/download/cancel` + zdarzenie `LocalModelProgress`.

## Porty (`app-api::ports`) — podmiana w `AppOptions`
Kolejność: nadpisanie z `AppOptions` → adapter na module → „moduł niepodłączony" (błąd z nazwą modułu).

| Port | Adapter | Bez modułu / czego brakuje |
|---|---|---|
| `BrainPort` | `RouterBrain` (Router + konta hubu + dostawcy lokalni; `AppOptions::providers` dokłada dostawców testowych) | `RouterUnavailable` |
| `TransferPort` | `TransferAdapter` (`transfer-impl`; dialogi zapisu/otwarcia z `ShellPort`; sekretów nie eksportuje nigdy — CX-a; sesji prywatnej nie eksportuje) | `TransferUnavailable` |
| `VoicePort` | `PipelineVoice` (`app-voice`): rozmowa, PTT, wyciszenie, pigułka — `voice-pipeline`; lista wejść, test mikrofonu (`MicLevel` ≤ 30/s) i czytanie na głos — `VoiceAdapter` (`voice-audio`, `voice-tts`; bez silnika — `NO_TTS`) | bez modeli/sidecarów: `VoiceStatus::Unavailable` z listą braków |
| `BrokerPort` | `InprocBroker` (poziomy autonomii, `request_level`, `run_code`, `undo_step`, `kill_all`) | podniesienie poziomu wymaga `ApprovalWindow`; bez Broker-UI — `NEEDS_BROKER_WINDOW` (odmowa); dozwolone `run_code` — brak wykonawcy `tools-shell` |
| `ShellPort` | powłoka Tauri (dialogi `tauri-plugin-dialog` w tym wybór katalogu, terminal w katalogu bez wykonania, okna, zasobnik) | `HeadlessShell` (testy; kolejka odpowiedzi dialogów) |
| GUI (`AppOptions::gui`) | `GuiPorts::system` — `WinGui` ze strażnikiem okien Alfy (WebView2, Broker-UI, watchdog, `%LOCALAPPDATA%\Alfa`) | poza Windows: panel „Ekran" z powodem niedostępności |
| `PseudoConsolePort` (`AppOptions::pty`) | ConPTY (`platform-windows-pty-impl`) | terminal: „niedostępny na tej platformie" |
| `SystemSignalsPort` / `DirWatchPort` (`AppOptions::signals`, `dir_watch`) | `WinSignals` / `WinDirWatch` (`platform-windows-sys-impl`) | „nigdy bezczynny", wyzwalacze plikowe tylko przez `file_created` |

Kill-switch (`system_kill_all`, `Ctrl+Shift+F12`, zasobnik): anulowanie generacji i pobierań →
`BrokerPort::kill_all` (audyt `broker.kill_switch`) → zatrzymanie mowy → toast.

## Czat (append-only)
`turns_send` → tura użytkownika → rezerwacja numeru tury agentki (numery są kolejne; wszystkie zapisy
historii sesji idą przez blokadę sesji i czekają na zapis aktywnej generacji) → `ModelProvider::stream`
z `CancellationToken` → `TextDelta` z blokami HTML z `lib-markdown::IncrementalRenderer` → zapis tury
(odpowiedź, która nie powstała — błąd/anulowanie przed tekstem — jako komunikat systemowy) → koszt w
`cost-meter` (grosze) → oś czasu. `regenerate` = wariant (`fork_from`), `edit_and_resend` = gałąź,
`continue` = tura-dziecko. Fakty spoza kontraktu `sessions` (stan, błąd, agentka, adresatka, zużycie PLN,
oceny, kolejka offline, oś czasu v0) są w tabelach `app_*` szyfrowanej bazy sesji (tylko INSERT).
Identyfikatory DTO niosą sesję: tura `"<sesja>:t<n>"`, plik `"<sesja>:a<id>"`, krok cofania
`"<sesja>:u<krok>"` (`turns_undo_step` → `undo-journal` przez Brokera, wpis „Cofnięto: …" na osi czasu).

## Testy
- `tests/dto_roundtrip.rs` (+ `tests/dto_spec/`) — każdy ładunek atrapy UI (162 komendy, 32 typy zdarzeń) deserializuje się
  do DTO i wraca bez strat; zbiór komend = COMMANDS.md = `app_core::COMMANDS`.
- `tests/ipc_signatures.rs` — sygnatury z `with_commands!` istnieją w `AppCore`, przyszłości `Send + 'static`
  (także `terminal_open` z `FrameSink` — w powłoce `Channel`).
- `tests/computer.rs` — F8 przez komendy: Delta na wirtualnym pulpicie (bez zgody — odmowa; okna Alfy —
  blokada Jądra, 0 skutków), panel „Ekran"/`GuiActivity` bez wpisywanej treści, przejęcie/oddanie;
  terminal (wejście tylko komendą, treść poza zdarzeniami); umiejętność propozycja → przegląd →
  zatwierdzenie hashem → zadanie; Kreator — zapis po teście na sucho; 401 dostawcy → incydent →
  naprawa → „Cofnij".
- `tests/signals.rs` — cykl Ulepszacza w bezczynności tylko z monitorem sygnałów; nowy plik w
  obserwowanym katalogu → wyzwalacz plikowy.
- `tests/scenario.rs`, `tests/errors.rs`, `tests/commands.rs` — scenariusze na atrapach
  (`providers-fake`, `device-profile-fake`, sekrety w pamięci, katalog tymczasowy).
- `tests/routing.rs` — fallback Routera (5xx → drugi kandydat, jedna odpowiedź), sesja prywatna
  (tylko dozwolone trasy), profil lokalny (`NO_LOCAL`, komendy modeli lokalnych).
- `tests/wiring.rs` — krok cofania, kill-switch (≤ 200 ms przy `ALFA_PERF_BUDGETS=1`, audyt Brokera),
  odmowa podniesienia autonomii bez Broker-UI i zgoda przez atrapę okna, czytanie na głos
  (`voice-tts-fake` + `voice-audio-fake`), `mark_good` updatera.
- `tests/transfer.rs` — eksport → import `.alfa` przez komendy (podgląd, tryby, rollback, szyfrowanie,
  brak komendy eksportu sekretów).
- `tests/memory.rs` — pamięć projektu wraca w innej sesji projektu (nie w innym projekcie); sesja
  prywatna nie wycieka; usunięcie sesji zapomina jej wpisy i kopie.
- `tests/tasks.rs` — DAG dwóch agentek (kolejność, Replay z `task_id`), kill-switch zatrzymuje zadanie,
  wyzwalacz ręczny → zadanie `trigger`.
- `tests/bridges.rs` — delegacja z czatu na `agent-backends-fake` (pochodzenie `UserRequest`, dopisek
  „niezweryfikowany"), 0 startów mostu z wyzwalacza, karty zgodności i „Zaloguj w terminalu".
- `tests/spy_work.rs` — 3 sesje z pamięcią i zadaniami, zero przecieków (kontekst, Replay, zdarzenia,
  Inspektor).
- `tests/spy.rs` — ≥ 3 sesje równolegle, zero przecieków (żądania, historia, zdarzenia, wyszukiwanie,
  oś czasu, pliki baz na dysku).
- `tests/agents.rs` — zadanie fs przez agentkę → krok w Replay → „Cofnij" przywraca stan; odmowa
  Brokera (blokada Jądra) → powód w wyniku narzędzia; zgoda bez decyzji → odmowa po czasie;
  kill-switch zatrzymuje pętlę i wiszące polecenie; steering i Stop.
- `tests/workdir.rs` — wybór katalogu roboczego (dialog, odrzucenia), sesja bez katalogu = czat;
  intencja „uruchom w terminalu" bez wykonania.
- `tests/voice.rs` — rozmowa na atrapach potoku (zegar wirtualny) z barge-in → usłyszany prefiks w
  historii i w kontekście kolejnego żądania; pigułka i stan trybu.
- `tests/spy_modes.rs` — agentka z narzędziami, rozmowa głosowa i czat równolegle: 0 przecieków
  (żądania, historia, Replay, zdarzenia, katalogi robocze, pliki danych Alfy).
- Progi czasu ścisłe tylko przy `ALFA_PERF_BUDGETS=1` (inaczej ×10).

## Fixture'y z atrapy UI
`tests/fixtures/*.json` generuje `tests/fixtures/gen/generate.ts` (atrapa `FakeAlfaClient` na wirtualnym
zegarze + `TauriAlfaClient` z atrapą `invoke`, żeby argumenty miały dokładnie kształt IPC):
```bash
ES=node_modules/.pnpm/esbuild@0.28.2/node_modules/esbuild/bin/esbuild
$ES crates/app-core/tests/fixtures/gen/generate.ts --bundle --platform=node --format=esm \
  --alias:@tauri-apps/api/core=./crates/app-core/tests/fixtures/gen/tauri-mock.ts \
  --alias:@tauri-apps/api/event=./crates/app-core/tests/fixtures/gen/tauri-mock.ts --outfile="$TMPDIR/gen.mjs"
node "$TMPDIR/gen.mjs" crates/app-core/tests/fixtures   # F5–F7: gen/generate-work.ts, F8: gen/generate-computer.ts
```
`tests/fixtures/extra.json` — ręcznie: warianty, których atrapa nie emituje (np. `VoicePill`,
`OpenSession`, `LocalModelProgress`, kody błędów). Regeneracja nadpisuje pozostałe pliki — dopisuj
zmiany w `extra.json` ręcznie, bez formatowania całego pliku.

## Komendy
```bash
CARGO_INCREMENTAL=0 cargo fmt -p app-api -p app-modules -p app-core
CARGO_INCREMENTAL=0 cargo clippy -p app-api -p app-modules -p app-core --all-targets --all-features --no-deps -- -D warnings
CARGO_INCREMENTAL=0 cargo test -p app-api -p app-modules -p app-core --all-features
ALFA_PERF_BUDGETS=1 CARGO_INCREMENTAL=0 cargo test -p app-core --test wiring   # ścisłe progi czasu
```
