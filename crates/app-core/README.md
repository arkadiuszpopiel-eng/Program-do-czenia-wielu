# app-core — korzeń kompozycji Alfy

Biblioteka bez zależności od Tauri. Składa moduły `-impl` i wystawia **każdą komendę z
`apps/desktop/ui/src/lib/api/COMMANDS.md`** jako metodę `AppCore::<przestrzeń>_<nazwa>` oraz strumień
zdarzeń `alfa://events` (paczki `AlfaEvent[]`, najwyżej jedna na klatkę ≈ 16 ms). Powłoka
`apps/desktop/src-tauri` tylko deleguje (handlery generuje makro `app_core::with_commands!`).

Kategoria `app-*` (crates/README.md, `scripts/check-deps.sh`): jedyny crate, który może zależeć od `*-impl`;
od `*-fake` tylko w `dev-dependencies`; od app-core nie zależy żaden crate workspace.

## Kompozycja (`AppCore::build(AppPaths, AppOptions)`)
1. Jądro: magistrala (`core-bus-impl`), rejestr (`core-registry-impl`), sekrety (`SecretStore`:
   Windows Credential Manager; poza Windows — pamięć procesu), profil urządzenia → `MachineId`,
   konfiguracja (`core-config-impl`, JSON Schema stron ustawień z `data/settings-pages.json`),
   logi NDJSON z magistrali (`core-log-impl`), zdarzenia UI (`EventHub`).
2. Rejestr dostaje manifesty (`module.toml`) wszystkich modułów i wylicza `start_order()`; każdy moduł
   jest budowany i startowany (`Module::start` z magistralą) **dokładnie w tej kolejności**, a w rejestrze
   zostaje pośrednik (manifest + zdrowie usługi). Cykl `sessions ↔ search` z manifestów jest rozcięty:
   `sessions` dostaje indekser wiązany później (`LateIndexer`), w grafie zostaje krawędź search → sessions.
3. Moduły: platform (manifest), device-profile, compliance, accounts-hub (sonda kont = `list_models`
   adapterów), providers-api, cost-meter (kurs NBP przez `reqwest`/rustls), sessions (`KeyVault` na
   `SecretStore` — klucze baz w Credential Managerze), search (osadzacz leksykalny `alfa-lexical-hash-v1`
   do czasu ONNX), memory, artifacts, personas, scheduler-lite. Katalog dostawców wbudowany
   (`providers-catalog/*.toml`, `include_str!`).

## Porty niepodłączonych modułów (`ports.rs`) — podmiana w `AppOptions`
| Port | Domyślnie | Docelowy moduł |
|---|---|---|
| `BrainPort` | `DirectBrain`: pierwszy skonfigurowany dostawca czatu z hubu (najpierw konta przypisane agentce); bez kluczy — „brak mózgu"; profil lokalny — błąd `no_keys` z nazwą `providers-local` | `router` (+ `providers-local`) |
| `TransferPort` | błąd „dostępne po podłączeniu modułu transfer" | `transfer` |
| `VoicePort` | mikrofony z `device-profile`; stop/wycisz = no-op; test mikrofonu, czytanie — błąd | `voice-*` |
| `BrokerPort` | błąd z nazwą `safety-broker` / `undo-journal` | `safety-broker`, `undo-journal` |
| `ShellPort` | `HeadlessShell` (testy) | powłoka Tauri |

## Czat (append-only)
`turns_send` → tura użytkownika → rezerwacja numeru tury agentki (numery są kolejne; wszystkie zapisy
historii sesji idą przez blokadę sesji i czekają na zapis aktywnej generacji) → `ModelProvider::stream`
z `CancellationToken` → `TextDelta` z blokami HTML z `lib-markdown::IncrementalRenderer` → zapis tury
(odpowiedź, która nie powstała — błąd/anulowanie przed tekstem — jako komunikat systemowy) → koszt w
`cost-meter` (grosze) → oś czasu. `regenerate` = wariant (`fork_from`), `edit_and_resend` = gałąź,
`continue` = tura-dziecko. Fakty spoza kontraktu `sessions` (stan, błąd, agentka, adresatka, zużycie PLN,
oceny, kolejka offline, oś czasu v0) są w tabelach `app_*` szyfrowanej bazy sesji (tylko INSERT).
Identyfikatory DTO niosą sesję: tura `"<sesja>:t<n>"`, plik `"<sesja>:a<id>"`.

## Testy
- `tests/dto_roundtrip.rs` — każdy ładunek atrapy UI (86 komend, 20 typów zdarzeń) deserializuje się
  do DTO i wraca bez strat; zbiór komend = COMMANDS.md = `app_core::COMMANDS`.
- `tests/ipc_signatures.rs` — sygnatury z `with_commands!` istnieją w `AppCore`, przyszłości `Send + 'static`.
- `tests/scenario.rs`, `tests/errors.rs`, `tests/commands.rs` — scenariusze na atrapach
  (`providers-fake`, `device-profile-fake`, sekrety w pamięci, katalog tymczasowy).
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
  --alias:@tauri-apps/api/event=./crates/app-core/tests/fixtures/gen/tauri-mock.ts --outfile=/tmp/gen.mjs
node /tmp/gen.mjs crates/app-core/tests/fixtures
```
`tests/fixtures/extra.json` — ręcznie: warianty, których atrapa nie emituje (np. `VoicePill`, kody błędów).

## Komendy
```bash
CARGO_INCREMENTAL=0 cargo fmt -p app-core
CARGO_INCREMENTAL=0 cargo clippy -p app-core --all-targets --all-features -- -D warnings
CARGO_INCREMENTAL=0 cargo test -p app-core --all-features
```
