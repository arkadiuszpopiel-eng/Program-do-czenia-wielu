# app-core — korzeń kompozycji Alfy

Biblioteka bez zależności od Tauri. Składa moduły `-impl` i wystawia **każdą komendę z
`apps/desktop/ui/src/lib/api/COMMANDS.md`** jako metodę `AppCore::<przestrzeń>_<nazwa>` oraz strumień
zdarzeń `alfa://events` (paczki `AlfaEvent[]`, najwyżej jedna na klatkę ≈ 16 ms). Powłoka
`apps/desktop/src-tauri` tylko deleguje (handlery generuje makro `app_core::with_commands!`).

Kategoria `app-*` (crates/README.md, `scripts/check-deps.sh`): jedyne crate'y, które mogą zależeć od
`*-impl`; od `*-fake` tylko w `dev-dependencies`; `app-*` mogą zależeć od siebie nawzajem, ale od żadnego
`app-*` nie zależy crate spoza tej kategorii. Ze względu na limit 8 000 linii korzeń jest rozcięty na trzy:

| Crate | Zawartość |
|---|---|
| `app-api` | kontrakt IPC: DTO (`dto/*`), `AppError`, identyfikatory DTO (`ids`), `EventHub`, porty (`ports/*`), `AppPaths`, protokół `alfa://`, powiadomienia; zależy tylko od `*-contract` |
| `app-modules` | adaptery portów na modułach `-impl`: `TransferAdapter`, `InprocBroker`, `VoiceAdapter`, `tts::engines` |
| `app-core` | kompozycja (`AppOptions` → `parts`), komendy, czat, Router (`route/*`); reeksportuje moduły `app-api` pod starymi ścieżkami (`app_core::dto`, `app_core::ports`, …) |

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
   `SecretStore`), search (osadzacz leksykalny `alfa-lexical-hash-v1` do czasu ONNX), memory, artifacts,
   personas, scheduler-lite. Katalog dostawców wbudowany (`providers-catalog/*.toml`, `include_str!`).
4. Moduły podpięte po F1 (`parts/extra.rs`; błąd budowy = moduł niezdrowy w rejestrze, nie błąd startu):
   model-residency (budżety z profilu urządzenia), providers-local (sidecar `llama-server` z
   `AppPaths::sidecar`), router, risk-classifier, safety-broker (w procesie, tryb deweloperski: audyt
   `local/broker-dev/audit.ndjson` + kotwica), undo-journal (`local/undo-store`), transfer
   (`DirDocumentStore` konfiguracji i logów, snapshoty w `local/snapshots`), voice-audio
   (`AppOptions::audio` albo WASAPI), voice-tts (sidecary Pocket TTS / Piper, jeśli zainstalowane),
   updater (`FsUpdater`; `mark_good` po `healthy_after` ≈ 30 s zdrowego startu).

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
| `TransferPort` | `TransferAdapter` (`transfer-impl`; dialogi zapisu/otwarcia z `ShellPort`; eksport sekretów tylko jawnie, z hasłem; sesji prywatnej nie eksportuje) | `TransferUnavailable` |
| `VoicePort` | `VoiceAdapter` (lista wejść i test mikrofonu `MicLevel` ≤ 30/s z `voice-audio`; czytanie na głos z `voice-tts`, bez silnika — `NO_TTS`) | włączenie mikrofonu/wyciszenie/pigułka: `voice-pipeline` |
| `BrokerPort` | `InprocBroker` (poziomy autonomii, `request_level`, `run_code`, `undo_step`, `kill_all`) | podniesienie poziomu wymaga `ApprovalWindow`; bez Broker-UI — `NEEDS_BROKER_WINDOW` (odmowa); dozwolone `run_code` — brak wykonawcy `tools-shell` |
| `ShellPort` | powłoka Tauri (dialogi `tauri-plugin-dialog`, okna, zasobnik) | `HeadlessShell` (testy; kolejka odpowiedzi dialogów) |

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
- `tests/dto_roundtrip.rs` — każdy ładunek atrapy UI (76 komend, 22 typy zdarzeń) deserializuje się
  do DTO i wraca bez strat; zbiór komend = COMMANDS.md = `app_core::COMMANDS`.
- `tests/ipc_signatures.rs` — sygnatury z `with_commands!` istnieją w `AppCore`, przyszłości `Send + 'static`.
- `tests/scenario.rs`, `tests/errors.rs`, `tests/commands.rs` — scenariusze na atrapach
  (`providers-fake`, `device-profile-fake`, sekrety w pamięci, katalog tymczasowy).
- `tests/routing.rs` — fallback Routera (5xx → drugi kandydat, jedna odpowiedź), sesja prywatna
  (tylko dozwolone trasy), profil lokalny (`NO_LOCAL`, komendy modeli lokalnych).
- `tests/wiring.rs` — krok cofania, kill-switch (≤ 200 ms przy `ALFA_PERF_BUDGETS=1`, audyt Brokera),
  odmowa podniesienia autonomii bez Broker-UI i zgoda przez atrapę okna, czytanie na głos
  (`voice-tts-fake` + `voice-audio-fake`), `mark_good` updatera.
- `tests/transfer.rs` — eksport → import `.alfa` przez komendy (podgląd, tryby, rollback, szyfrowanie,
  eksport sekretów).
- `tests/spy.rs` — ≥ 3 sesje równolegle, zero przecieków (żądania, historia, zdarzenia, wyszukiwanie,
  oś czasu, pliki baz na dysku).
- Progi czasu ścisłe tylko przy `ALFA_PERF_BUDGETS=1` (inaczej ×10).

## Fixture'y z atrapy UI
`tests/fixtures/*.json` generuje `tests/fixtures/gen/generate.ts` (atrapa `FakeAlfaClient` na wirtualnym
zegarze + `TauriAlfaClient` z atrapą `invoke`, żeby argumenty miały dokładnie kształt IPC):
```bash
ES=node_modules/.pnpm/esbuild@0.28.2/node_modules/esbuild/bin/esbuild
$ES crates/app-core/tests/fixtures/gen/generate.ts --bundle --platform=node --format=esm \
  --alias:@tauri-apps/api/core=./crates/app-core/tests/fixtures/gen/tauri-mock.ts \
  --alias:@tauri-apps/api/event=./crates/app-core/tests/fixtures/gen/tauri-mock.ts --outfile="$TMPDIR/gen.mjs"
node "$TMPDIR/gen.mjs" crates/app-core/tests/fixtures
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
