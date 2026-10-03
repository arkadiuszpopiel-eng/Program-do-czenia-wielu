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

Wyjątek: **`app-*`** — korzeń kompozycji aplikacji (`app-core`, `app-api`, `app-modules`, `app-safety`: składają
moduły `-impl`, wystawiają komendy i zdarzenia dla powłoki Tauri, budują binaria usług). Tylko `app-*` mogą zależeć
od `*-impl`; od `*-fake` tylko w dev-dependencies; `app-*` mogą zależeć od innych `app-*`, ale żaden crate spoza
`app-*` nie zależy od `app-*`.

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
| `memory` | `memory-contract`, `-impl`, `-fake` | F7: cztery warstwy (robocza, epizodyczna, semantyczna, proceduralna), zakresy sesja/projekt/agentka/globalna z uprawnieniami, wersje faktów, Inspektor, `forget` kaskadowo z zatarciem, recall hybrydowy z rerankingiem, eksport w `.alfa` |
| `artifacts` | `artifacts-contract`, `-impl`, `-fake` | rejestr plików wyjściowych z wersjami i hashami, podgląd, diff, intencje UI |
| `providers` | `providers-contract`, `providers-fake`, `providers-api-impl` | `ModelProvider`, neutralny IR (thinking z podpisem, tool use), zdarzenia strumienia; adaptery Anthropic, OpenAI Chat/Responses, generyczne zgodne z OpenAI/Anthropic; retry, timeouty, anulowanie < 100 ms |
| `lib-markdown` | `lib-markdown` (biblioteka) | Markdown z LLM → bezpieczny HTML (pulldown-cmark + ammonia, 72 wektory XSS), renderowanie przyrostowe dla strumienia, tekst mówiony |
| `personas` | `personas-contract`, `-impl`, `-fake` | Alfa/Beta/Gama/Delta, katalog ról, obsady i szablony, adresowanie z polską odmianą imion, polecenia zmiany obsady, prompt w rodzaju żeńskim |
| `scheduler-lite` | `scheduler-lite-contract`, `-impl`, `-fake` | zasoby wyłączne (mikrofon, głośnik, ekran, pliki), kolejka priorytetowa, voice-first, kolejka mówienia z przekazaniem, wykrywanie zakleszczeń |
| `voice-persona` | `voice-persona-contract`, `-impl`, `-fake` | normalizator PL do mowy (liczby z rodzajem i przypadkiem, daty, godziny, waluty, jednostki, skróty, URL), słownik wymowy, chunker strumieniowy, planista stylu per silnik |
| `voice-cmd` | `voice-cmd-contract`, `-impl`, `-fake` | szybkie komendy PL/EN bez LLM (tolerancja szumu ASR, odmiana imion), reguła „nie" tylko w `Speaking`; zestaw zamrożony: recall 100%, 0 fałszywych |
| `voice-turn` | `voice-turn-contract`, `-impl`, `-fake` | polityka końca tury z cierpliwością i hezytacjami, trait `TurnModel` (Smart Turn ONNX później) |
| `voice-dialog` | `voice-dialog-contract`, `-impl`, `-fake` | czysty automat dialogu §6.5: ducking + twardy stop (p95 350 ms), backchannel, usłyszany prefiks, 6 klas intencji przerwania, wznawianie |
| `lib-openai-compat` | `lib-openai-compat` (biblioteka) | wspólny silnik HTTP/SSE zgodny z OpenAI (Chat Completions, retry, timeouty, klasyfikacja błędów) dla `providers-api-impl` i `providers-local-impl` |
| `router` | `router-contract`, `-impl`, `-fake` | klasy zadań × ograniczenia (prywatność, jurysdykcja, budżet, możliwości), fallback ≤ 2 s bez utraty wiadomości, circuit breaker, `router.decision` z uzasadnieniem; Router sam jest `ModelProvider` |
| `providers-local` | `providers-local-impl` (kontrakt: `providers-contract`) | llama.cpp jako sidecar `llama-server` (127.0.0.1, losowy port i klucz), cykl życia, GPU→CPU, menedżer pobierania z wznawianiem i SHA-256, zakaz kwantów IQ |
| `model-residency` | `model-residency-contract`, `-impl`, `-fake` | zarządca RAM/VRAM: rejestr modeli, budżety z profilu urządzenia, wymiana wg priorytetów (głos > rozmowa > tło), tryb gry |
| `transfer` | `transfer-contract`, `-impl`, `-fake` | paczki `.alfa` (ZIP + manifest), sekrety nigdy w zwykłym eksporcie, szyfrowanie hasłem (Argon2id + XChaCha20-Poly1305 STREAM), podgląd importu, tryby dodaj/scal/zastąp, snapshot i rollback, kopie z rotacją, ochrona przed path traversal i zip-bomb |
| `updater` | `updater-contract`, `-impl` (+ bin `alfa` = launcher), `-fake` | wersje obok siebie, `current.json` atomowo, wybór wersji z fallbackiem i crash-loop, minisign + SHA-256 z wersją w podpisie |
| `voice-audio` | `voice-audio-contract`, `-impl`, `-fake` | WASAPI (crate `wasapi`), mikser z duckingiem i `stop_all`, licznik odtworzonych próbek, referencja AEC, resampler; wątek RT bez alokacji |
| `voice-dsp` | `voice-dsp-contract`, `-impl`, `-fake` | AEC3 (`sonora`, ERLE 49 dB w teście), RNNoise (`nnnoiseless`), AGC, kalibracja pętli |
| `voice-vad` | `voice-vad-contract`, `-impl`, `-fake` | Silero VAD przez `tract-onnx` (bez ONNX Runtime), histereza, próg adaptacyjny, zapas energetyczny |
| `voice-stt` | `voice-stt-contract`, `-impl`, `-fake` | sidecar `whisper-server` (whisper.cpp), bramka VAD, partial + final, hotwords, fallback GPU→CPU |
| `voice-tts` | `voice-tts-contract`, `-impl`, `-fake` | Pocket TTS (sidecar JSON-lines) i Piper, głosy v0 (wysokość/tempo WSOLA), łańcuch fallback per agentka, cache fraz, TTFB |
| `voice-wake` | `voice-wake-contract`, `-impl` (+ bin `alfa-wake-eval`), `-fake` | PTT, przełącznik, DND, stan mikrofonu, adresowanie po imieniu, mikrofon jako zasób wyłączny; v1: słowa wywoławcze „Hej …" (KWS tract-onnx, domyślnie wyłączone, tylko lokalnie, bufor 2 s bez wycieku audio przed wykryciem) |
| `risk-classifier` | `risk-classifier-contract`, `-impl`, `-fake` | deterministyczna tabela reguł ryzyka (odwracalność, zakres, źródło polecenia, pewność STT, taint) |
| `safety-broker` | `safety-broker-contract`, `-impl`, `-fake` | tokeny zdolności z atenuacją i HMAC, L0–L4, twarde blokady Jądra (także na L4), plan do zatwierdzenia, `PhysicalInputProof`, audyt z łańcuchem i kotwicą, kill-switch, IPC bez TCP. **Wymaga przeglądu człowieka** |
| `undo-journal` | `undo-journal-contract`, `-impl`, `-fake` | trwały dziennik cofania `fs.*` z pre-image, kroki, konflikty; 100% przywrócenia w testach losowych |
| `watchdog` | `watchdog-contract`, `-impl`, `-fake` | heartbeat, restart z limitem, safe-mode, rollback konfiguracji i wersji, tabela Job Objects. **Wymaga przeglądu człowieka** |
| `agent-backends` | `agent-backends-contract`, `-impl`, `-fake` | mosty do oficjalnych CLI (`claude -p`, `codex app-server`) jako opaque worker: tylko na żądanie użytkownika, przypięte wersje, zero dostępu do poświadczeń CLI, prośby o uprawnienia do kanału zatwierdzeń |
| `mcp` | `mcp-contract`, `-impl` (+ bin `alfa-mcp-proxy`), `-fake` | klient MCP (stdio, odcisk opisów narzędzi, skaner prompt injection, poziomy zaufania) i serwer MCP Alfy v0 (schowek, okna, `approve`) przez named pipe / gniazdo 0600 z tokenem; bez TCP |
| `app` | `app-core` (korzeń kompozycji `app-*`) | składa moduły, wszystkie komendy z `COMMANDS.md`, zdarzenia paczkowane co klatkę, czat ze strumieniem i markdownem, porty dla modułów jeszcze niepodpiętych |
| `voice-pipeline` | `voice-pipeline-contract`, `-impl` (+ bin `alfa-voice-eval`), `-fake` | runtime potoku głosu: mikrofon → DSP → VAD → STT → komendy/koniec tury → automat dialogu → odpowiedź → persona → TTS → wyjście; dzierżawy głośnika/mikrofonu, barge-in z usłyszanym prefiksem, zdarzenia `voice.*`; runner zestawu F2 |
| `tools-common` | `tools-common-contract` | wspólny kontrakt narzędzi agentek: manifest (JSON Schema, odwracalność, zdolności), `Tool`, `BrokerGate` (decide → verify → revoke), delimitacja treści niezaufanej |
| `tools-fs` | `tools-fs-contract`, `-impl`, `-fake` | 11 narzędzi plikowych przez `FsPort` i Broker; mutacje przez `undo-journal`, usuwanie do Kosza, trwałe tylko po zatwierdzeniu w Broker-UI |
| `tools-shell` | `tools-shell-contract`, `-impl`, `-fake` | polecenia w zakresie katalogu: snapshot przed wykonaniem, Job Object, filtrowane środowisko, tokeny `shell.exec`/`net.egress`, „uruchom w terminalu" |
| `tools-clipboard` | `tools-clipboard-contract`, `-impl`, `-fake` | odczyt (taint) i zapis tekstu/obrazu schowka, cofanie zapisu z wykrywaniem konfliktu |
| `agent-runtime` | `agent-runtime-contract`, `-impl`, `-fake` | v1: równoległe przebiegi z dzierżawami zasobów, granica kroku dla schedulera (steering 20/20), delegacja z atenuacją uprawnień, Krytyczka, równoległe odczyty, raport przebiegu; zdarzenia `agent.*` dla Replay i „Cofnij" |
| `broker-ui` | `broker-ui-contract`, `-impl`, `-fake` | okno zatwierdzeń Brokera: karta prośby, `PhysicalInputProof` tylko z fizycznego wejścia (odrzuca wstrzyknięte, clickjacking ≥ 500 ms), Enter nie zatwierdza. **Wymaga przeglądu człowieka** |
| `app-api`, `app-modules`, `app-safety` | (korzeń kompozycji `app-*`) | `app-api`: DTO, zdarzenia, porty, protokół; `app-modules`: adaptery modułów dla `app-core`; `app-safety`: binaria `alfa-broker`, `alfa-broker-ui`, `alfa-watchdog` złożone z `platform-windows-kernel-impl` |
| `platform-windows-kernel` | `platform-windows-kernel-impl` (kontrakt: `platform-contract`) | prymitywy Windows dla Jądra wydzielone z `platform-windows-impl`: named pipe z chronionym DACL, tożsamość klienta, katalog prywatny, okno zatwierdzeń Win32, start z integralnością High, host usługi, MMCSS |
| `memory-consolidation` | `memory-consolidation-contract`, `-impl`, `-fake` | Strażniczka pamięci: konsolidacja epizodów w fakty i umiejętności (reguły + LLM przez port), okna bezczynności, budżet tła, nie na baterii ani w grze, dziennik i cofanie przebiegu |
| `scheduler` | `scheduler-contract`, `-impl`, `-fake` | pełny scheduler (nadzbiór `scheduler-lite`): DAG z warunkami, równoległe agentki z atomowym przydziałem zasobów, priorytety voice-first, okna czasowe, budżety, ponowienia, steering, trwałość; 0/1000 zakleszczeń |
| `triggers` | `triggers-contract`, `-impl`, `-fake` | wyzwalacze czasowe (cron z polską strefą i DST), zdarzeniowe i ręczne; limity, cisza/DND, taint treści; wyzwalacz nigdy nie uruchamia mostu CLI |
| `marshal` | `marshal-contract`, `-impl`, `-fake` | Marszałek: polecenia użytkownika → reguły, które tylko zawężają uprawnienia; nadzór postępu, eskalacje, raport dzienny |
| `app-agents`, `app-voice` | (korzeń kompozycji `app-*`) | `app-agents`: agentka z narzędziami w aplikacji (runtime + tools + Broker + cofanie, Replay, eval F3); `app-voice`: potok głosu w aplikacji (rozmowa, barge-in z usłyszanym prefiksem, pigułka) |
| `platform-windows-gui`, `platform-windows-pty` | `platform-windows-gui-impl`, `platform-windows-pty-impl` (kontrakt: `platform-contract`) | F6: UIA (wątek MTA z limitami, wzorce, `TextPattern` tylko do odczytu, hasła nigdy nie wychodzą), `SendInput` ze strażnikiem celów (nigdy okna Alfy/Brokera) i przerwaniem przy fizycznym wejściu, zrzuty BitBlt/PrintWindow z maskowaniem; ConPTY w Job Object |
| `tools-window`, `tools-uia`, `tools-input`, `tools-screen` | trójki `-contract`, `-impl`, `-fake` | narzędzia computer use dla agentek przez `BrokerGate` (`gui.control`), wynik = treść niezaufana, weryfikacja po akcji; 0/200 skutków w oknach Alfy (property) |
| `ui-terminal` | `ui-terminal-contract`, `-impl`, `-fake` | terminal ConPTY do logowania w CLI mostów — sterowany wyłącznie przez użytkownika (`UserGesture`), bez API dla agentek, treść nigdy w logach/zdarzeniach |
| `skills` | `skills-contract`, `-impl`, `-fake` | umiejętności: wersjonowane przepisy z walidacją zdolności, instalacja tylko po zatwierdzeniu z hashem, kwarantanna treści niezaufanej, uprawnienia ≤ rola wywołującej |
| `agent-builder` | `agent-builder-contract`, `-impl`, `-fake` | Kreator agentów: persona/rola z rozmowy lub formularza, odmiana imienia, głos v0, autonomia ≤ sesja (nigdy L4), test na sucho, zapis dwufazowy nie-głosem; 41 prób ataku → 0 |
| `evals` | `evals-contract`, `-impl` (+ bin `alfa-evals`), `-fake` | harness zestawów: manifest z SHA-256 i statusem zamrożenia, dev/test/holdout, bootstrap, porównania; holdout tylko przez bramkę Jądra (wynik zbiorczy) |
| `diagnostician` | `diagnostician-contract`, `-impl`, `-fake` | Diagnosta: sygnały z rejestru/logów/watchdoga, katalog 24 awarii, naprawy cofalne z weryfikacją, Jądro tylko przez Brokera, raport „Zdrowie systemu" |
| `improver` | `improver-contract`, `-impl`, `-fake` | Ulepszacz R0–R2: propozycje, piaskownica z holdoutem, wdrożenie po zatwierdzeniu z rollbackiem; nigdy Jądro, progi, deny-listy (0/137 prób obejścia) |
| `plugin-runtime` | `plugin-runtime-contract`, `-impl`, `-fake` | F8: wtyczki Wasm (wasmtime 36 LTS, model komponentów WIT `alfa:plugin`, bez WASI); jedyny import `host.call` przez Broker (`fs.read/write`, `net.egress`), paliwo, epoki, limity pamięci/stosu, nowa instancja na wywołanie; zatwierdzenie kliknięciem z hashem manifestu, propozycje R2 Ulepszacza |
| `app-memory`, `app-tasks`, `app-bridges` | (korzeń kompozycji `app-*`) | pamięć F7 + Inspektor w aplikacji; scheduler/wyzwalacze/Marszałek + panel Zadania; mosty CLI z kartami zgodności, delegacja z czatu, serwer MCP v0 |
| `platform-windows-sys` | `platform-windows-sys-impl` (kontrakt: `platform-contract`) | sygnały systemowe (bezczynność z histerezą, zasilanie, tryb gry/pełny ekran, blokada sesji) i obserwacja katalogów `ReadDirectoryChangesW` (debounce, przeskanowanie po przepełnieniu, deny-lista surowa i kanoniczna) |
| `voice-speaker` | `voice-speaker-contract`, `-impl` (+ bin `alfa-speaker-eval`), `-fake` | weryfikacja właściciela: rejestracja ≥ 3 wypowiedzi, embedding ECAPA (tract-onnx, model ze ścieżki), profil szyfrowany XChaCha20-Poly1305 z kluczem w sejfie, eksport tylko za zgodą; ryzyko głosem bez weryfikacji → potwierdzenie nie-głosem |
| `voice-dictation` | `voice-dictation-contract`, `-impl`, `-fake` | dyktowanie do dowolnej aplikacji: normalizacja odwrotna PL (interpunkcja, liczby), wpisywanie tylko do okna docelowego, nigdy do okien Alfy/Brokera ani pól haseł (fail-closed), „cofnij to" |
| `voice-readaloud` | `voice-readaloud-contract`, `-impl`, `-fake` | czytanie zaznaczenia/okna przez UIA TextPattern głosem agentki, TTS prywatny, treść niezaufana (do modelu tylko za zgodą) |
| `voice-s2s` | `voice-s2s-contract`, `-fake` | kontrakt trybu speech-to-speech w chmurze (Realtime z natywnym obcięciem); sesja prywatna nigdy się nie łączy; adapter chmurowy później |
| `app-gui`, `app-terminal`, `app-skills`, `app-health`, `app-store` | (korzeń kompozycji `app-*`) | computer use w aplikacji (strażnik celów z PID-ami Alfy, panel Ekran, „Zatrzymaj sterowanie", ochrona okien przed zrzutem); terminal ConPTY przez kanał IPC (bez zdarzeń/logów); umiejętności i Kreator agentek; Zdrowie systemu (Diagnosta, Ulepszacz, evale); magazyn aplikacji w bazie sesji |
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

## Testy budżetów czasowych
Testy mierzące czas (`tests/budget.rs` itp.) na współdzielonym CI sprawdzają tylko próg bezpieczeństwa
(×10 budżetu — łapie patologiczne regresje); **ścisłe budżety z planu obowiązują przy `ALFA_PERF_BUDGETS=1`**
(self-hosted runner z emulacją baseline, pomiary lokalne). Budżet zawsze wypisuj przez `eprintln!`.
