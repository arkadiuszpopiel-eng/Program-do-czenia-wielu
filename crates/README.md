# crates/

Workspace Rust jądra i modułów Alfy (docs/PLAN.md §3.2, §3.6; ADR 0002).

## Zasada trójki crate'ów
Każdy moduł `<m>` to trzy crate'y:

| Crate | Zawartość | Kto może od niego zależeć |
|---|---|---|
| `<m>-contract` | traity, typy, nazwy zdarzeń, JSON Schema, współdzielone testy kontraktowe (feature `contract-tests`) | wszyscy |
| `<m>-impl` | implementacja produkcyjna + `module.toml` | tylko rejestr modułów / kompozycja builda |
| `<m>-fake` | deterministyczna atrapa (wirtualny zegar, record/replay) | **tylko `dev-dependencies`** testów innych modułów |

Reguła twarda: **moduł zależy od innego modułu wyłącznie przez `-contract`.** Zależność `-impl`/`-fake`
od cudzego `-impl` (w dowolnym rodzaju zależności) albo od cudzego `-fake` w zależnościach produkcyjnych
jest błędem CI — sprawdza to `scripts/check-deps.sh` (przez `cargo metadata` + `jq`).

Wyjątek: **`lib-*`** — wspólna biblioteka narzędziowa bez logiki modułu (np. `lib-sqlstore`: szyfrowane
połączenie SQLCipher, migracje, rejestracja sqlite-vec). Moduły mogą od niej zależeć; ona sama zależy
wyłącznie od innych `lib-*` i `*-contract`.

Wyjątek: **`app-*`** — korzeń kompozycji aplikacji (np. `app-core`: składa moduły `-impl`, wystawia komendy
i zdarzenia dla powłoki Tauri). Jako jedyny może zależeć od `*-impl`; od `*-fake` tylko w dev-dependencies;
od niego nie zależy żaden crate.

## Crate'y w F0 (pkt 2 §4.5a)
| Moduł | Crate'y | Uwagi |
|---|---|---|
| `core-bus` | `core-bus-contract`, `core-bus-impl`, `core-bus-fake` | zdarzenia §13, schemat `event.v1.json` |
| `core-registry` | `core-registry-contract`, `-impl`, `-fake` | manifest `module.toml`, trait `Module`/`Registry`, graf zależności, cykl życia lazy/on-demand/always, zwalnianie po bezczynności |
| `core-config` | `core-config-contract`, `-impl`, `-fake` | warstwy TOML wspólna/maszyna/sesja/agentka, JSON Schema, `kernel.*` tylko Broker, historia NDJSON, watch |
| `core-log` | `core-log-contract`, `-impl`, `-fake` | NDJSON z rotacją/retencją/limitem dysku, redakcja, audyt pre-broker z łańcuchem SHA-256 |
| `platform-windows` | `platform-contract`, `platform-fake`, `platform-windows-impl` | `SystemPort` + `HardwarePort`; impl: Kosz (IFileOperation), Job Objects, schowek, okna, skróty + `WH_KEYBOARD_LL` (PTT), DXGI/MMDevice; jedyny crate z windows-rs |
| `device-profile` | `device-profile-contract`, `-impl`, `-fake` | autodetekcja sprzętu, klasy §3.5, rekomendacja profilu głosu A–D i rezydencji, emulacja baseline, `MachineId` |
| `compliance` | `compliance-contract`, `-impl`, `-fake` | rejestr tras (zielona/szara/zabroniona, degradacja nieświeżych), tagi prywatności/jurysdykcji, `route_allowed`, deny-listy ścieżek i domen z normalizacją Windows, `KernelAuthority` |
| `accounts-hub` | `accounts-hub-contract`, `-impl`, `-fake` | katalog dostawców (walidacja schematu), konta i klucze (`SecretStore`: Credential Manager / pamięć), kreator jako maszyna stanów, import z env, wykrywanie mostów CLI |
| `cost-meter` | `cost-meter-contract`, `-impl`, `-fake` | koszty w liczbach całkowitych (mikro-USD/PLN), kurs NBP z cache i zapasem, limit miesięczny Enforced/AlertOnly/Off, budżet tła, NDJSON |
| `lib-sqlstore` | `lib-sqlstore` (biblioteka) | SQLCipher kluczem surowym, WAL, migracje, sqlite-vec (`OnceLock`, jedyne `unsafe`), `fold_pl` dla FTS, usuwanie z `-wal/-shm` |
| `sessions` | `sessions-contract`, `-impl`, `-fake` | baza per sesja, historia append-only jako drzewo gałęzi (wyzwalacze blokują UPDATE/DELETE), usłyszany prefiks, katalog `index.db`, `KeyVault`, crypto-shredding |
| `search` | `search-contract`, `-impl`, `-fake` | FTS5 z `fold_pl` + `vec0` (kosinus), hybryda RRF, indeksowanie w transakcji zapisu (`TxIndexer`), szukanie między sesjami tylko dla właściciela |
| `memory` | `memory-contract`, `-impl`, `-fake` | v0: remember/recall/forget w zakresie sesji, proweniencja, treść niezaufana nie awansuje |
| `artifacts` | `artifacts-contract`, `-impl`, `-fake` | rejestr plików wyjściowych z wersjami i hashami, podgląd, diff, intencje UI |
| `providers` | `providers-contract`, `providers-fake`, `providers-api-impl` | `ModelProvider`, neutralny IR (thinking z podpisem, tool use), zdarzenia strumienia; adaptery Anthropic, OpenAI Chat/Responses, generyczne zgodne z OpenAI/Anthropic; retry, timeouty, anulowanie < 100 ms |
| `lib-markdown` | `lib-markdown` (biblioteka) | Markdown z LLM → bezpieczny HTML (pulldown-cmark + ammonia, 72 wektory XSS), renderowanie przyrostowe dla strumienia, tekst mówiony |
| `personas` | `personas-contract`, `-impl`, `-fake` | Alfa/Beta/Gama/Delta, katalog ról, obsady i szablony, adresowanie z polską odmianą imion, polecenia zmiany obsady, prompt w rodzaju żeńskim |
| `scheduler-lite` | `scheduler-lite-contract`, `-impl`, `-fake` | zasoby wyłączne (mikrofon, głośnik, ekran, pliki), kolejka priorytetowa, voice-first, kolejka mówienia z przekazaniem, wykrywanie zakleszczeń |
| `voice-persona` | `voice-persona-contract`, `-impl`, `-fake` | normalizator PL do mowy (liczby z rodzajem i przypadkiem, daty, godziny, waluty, jednostki, skróty, URL), słownik wymowy, chunker strumieniowy, planista stylu per silnik |
| `voice-cmd` | `voice-cmd-contract`, `-impl`, `-fake` | szybkie komendy PL/EN bez LLM (tolerancja szumu ASR, odmiana imion), reguła „nie" tylko w `Speaking`; zestaw zamrożony: recall 100%, 0 fałszywych |
| `voice-turn` | `voice-turn-contract`, `-impl`, `-fake` | polityka końca tury z cierpliwością i hezytacjami, trait `TurnModel` (Smart Turn ONNX później) |
| `voice-dialog` | `voice-dialog-contract`, `-impl`, `-fake` | czysty automat dialogu §6.5: ducking + twardy stop (p95 350 ms), backchannel, usłyszany prefiks, 6 klas intencji przerwania, wznawianie |
| `example-module` | `example-module-contract`, `-impl`, `-fake` | wzorzec dla wszystkich kolejnych modułów |


## Jak dodać moduł
1. `docs/modules/<m>/SPEC.md` (1 strona) → 2. `<m>-contract` (+ `contract_tests` pod feature) →
3. `<m>-fake` → 4. testy kontraktowe na fake → 5. `<m>-impl` z `module.toml` → 6. `scripts/check-deps.sh`,
`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
Wzór: skopiuj `example-module-*`. Wszystkie wersje zależności bierz z `[workspace.dependencies]`.

## Linty
`[workspace.lints]` w `Cargo.toml`: `unwrap_used`/`expect_used`/`todo`/`unimplemented` = deny
(w testach wyłączane przez `#![cfg_attr(test, allow(...))]` / `#![allow(...)]` na górze pliku testu),
`too_many_lines` = warn z progiem 300 (`clippy.toml`), `unsafe_code` = forbid (poza przyszłym `platform-windows-impl`
po ADR), `missing_docs` = warn. Plik ≤ 400 linii, crate ≤ 8 000 linii (AGENTS.md).
