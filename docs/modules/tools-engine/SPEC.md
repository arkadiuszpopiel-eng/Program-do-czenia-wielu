# tools-engine — SPEC (v0, 2026-10-27)

## Cel

Połączenie Alfy z silnikiem gier właściciela **NextGO.Gray** (nazwa tymczasowa): agentki
uruchamiają serwer silnika, budują i zmieniają świat transakcjami, odczytują scenę, przesuwają
symulację, podmieniają shadery, robią zrzuty i czytają dokumentację silnika — bez zmian
w repozytorium silnika (pracuje nad nim inny model; Alfa tylko czyta jego katalog).

## Silnik (stan 27.10, etap 5)

`target/release/engine-server.exe` — `--listen IP:PORT` (tylko pętla zwrotna), `--paused`,
`--demo|--industrial|--load PLIK` (≤ 64 MiB), `--audio`, `--preview` (okno GPU), `--frames N
--preview-output DIR` (zrzut `preview.png` + `preview.json` i koniec), `--capture-revisions`
(zrzut każdej rewizji, wymaga `--preview --paused --preview-output`). Pierwszy wiersz stdout:
`{"listening":"ws://127.0.0.1:PORT","mode":"headless|gpu_preview","paused":bool}`. WebSocket
JSON-RPC 2.0: brak uwierzytelnienia (tylko pętla zwrotna), żądania z nagłówkiem `Origin`
odrzucane (403), wiadomość ≤ 1 MiB, 16 połączeń, 120 s ciszy zrywa połączenie, serwer nie
wysyła nic sam. Metody: `engine.execute_transaction` (atomowa, `transaction_id` idempotentny,
`expected_revision`), `rollback_last_change` (64 kroki), `headless_step` (0–10000),
`set/get_spatial_selection`, `hot_reload_shader`, `inspect_scene_telemetry`,
`inspect_world_streaming`, `stream_world` (1–32), `raycast_world`, `query_world_height`.
Błędy: −32700/−32600/−32601/−32602/−32603, −32000 z `data.code` (`revision_conflict`,
`duplicate_transaction`, `no_snapshot`, `entity_not_found`, `streaming_not_ready`,
`capacity_exceeded`, `invalid_shader`, `invalid_input`, `response_too_large`, …).

## Kontrakt (`crates/tools-engine-contract`)

`EnginePort`: `installation()` (katalog, programy, wersja `[workspace.package]`, stan
`artifacts/verification.json`), `state()`, `start(StartOptions)`, `stop()`,
`call(EngineMethod, params)`, `capture()`, `preview(PreviewOptions)`, `docs()`, `read_doc()`,
`kill_all()`. `Listening::parse` (tylko `127.0.0.1`/`[::1]` z portem), `request`/`response`
(JSON-RPC, limit 1 MiB przed wysłaniem), `hint(data.code)` — co zrobić dalej.

Narzędzia (15, grupa `engine` + `shell`):

```
engine_status {}                       fs.read(katalog)      instalacja + stan serwera
engine_docs {file?, offset?}           fs.read(katalog)      lista / treść (30 tys. znaków na część), taint file
engine_start {scene?, load?, paused?=true, window?, audio?}   shell.exec(katalog) (+ fs.read(plik load))
engine_stop {}                         shell.exec(katalog)
engine_preview {scene?=demo, load?, frames?=3 (1–600), width?, height?}  osobny proces → PNG + telemetria
engine_inspect {what?: scene|streaming|selection, radius_meters?, center?, limit?}  gui.control(engine-server), odczyt, taint file
engine_raycast {origin, direction, max_distance?, dimension?}   odczyt
engine_height {x, z}                   odczyt
engine_capture {}                      odczyt: ostatni zrzut rewizji z okna (obraz PNG)
engine_transaction {operations[1–1024], transaction_id?, expected_revision?}   zmiana
engine_rollback {} · engine_step {frames 1–10000} · engine_stream {budget?, until_ready?=true}
engine_select {entity_id|null} · engine_shader {shader_name, wgsl_code, transaction_id?, expected_revision?}
```

Bez `transaction_id` Alfa nadaje stały identyfikator `alfa-<sha256(sesja, przebieg, krok, treść)>`
— ponowienie tego samego wywołania nie zdubluje zmiany.

## Implementacja (`crates/tools-engine-impl`)

`ProcessEngine`: katalog silnika z ustawienia `engine.nextgo.root`; proces `--listen
127.0.0.1:0` (nigdy domyślny port 9001), katalog roboczy procesu, zrzuty i podglądy
w `<dane Alfy>/engine/…`; środowisko z listy dozwolonej (bez kluczy), bez okna konsoli,
`kill_on_drop`, rejestr procesów pobocznych (`lib-sidecar-guard`), ogon stderr w błędach.
Własny klient WebSocket (RFC 6455: maskowane ramki klienta, ping/pong, fragmenty, zamknięcie,
sprawdzenie `Sec-WebSocket-Accept` przez SHA-1) — bez nowych zależności. Po 90 s ciszy
połączenie jest odnawiane przed wywołaniem; wysłane żądanie nigdy nie jest ponawiane
(krok i cofnięcie nie są idempotentne). Limity: start 60 s, wywołanie 30 s, kroki/transakcje
180 s, podgląd czas klatek + 90 s. Kill-switch (`EngineTools::kill_switch`) kończy serwer
i podglądy.

## Bezpieczeństwo

Katalog silnika — tylko odczyt (żaden plik nie powstaje w nim; test sprzętowy to sprawdza).
Każde wywołanie przez Brokera: `fs.read`/`shell.exec` zakresem katalogu silnika,
`gui.control(engine-server.exe)` dla JSON-RPC (zmiany jako akcje zmieniające stan); plik `load`
— deny-lista i osobna zgoda `fs.read(plik)`. Dokumentacja i telemetria (nazwy, właściwości
encji z plików) — treść niezaufana (taint `file`). Serwer tylko na pętli zwrotnej; adres
z pierwszego wiersza sprawdzany przed połączeniem.

## Testy

Kontraktowe (`contract-tests`: manifesty, argumenty spoza zakresu, scenariusz portu), atrapa
`tools-engine-fake` (świat w pamięci z rewizjami, idempotencją, konfliktem, cofaniem),
`tools-engine-impl/tests/tools.rs` (cykl pracy agentki na atrapie i Brokerze), klient
WebSocket na lokalnym serwerze testowym, test sprzętowy `tests/engine.rs` z
`ALFA_NEXTGO_ROOT=<katalog zbudowanego silnika>` (prawdziwy serwer, scenariusz kontraktowy,
kill-switch, brak plików w katalogu silnika).

## Poza zakresem v0 (zależy od silnika)

Zapis/eksport świata i zrzut działającego serwera bez okna — silnik nie ma jeszcze takich
metod (etap 6). Most MCP silnika (`engine-mcp-bridge.exe`) działa niezależnie: można go dodać
w Ustawieniach → Serwery MCP; Alfa łączy się z serwerem bezpośrednio (pełny zestaw metod,
w tym zaznaczenie, którego most nie udostępnia).
