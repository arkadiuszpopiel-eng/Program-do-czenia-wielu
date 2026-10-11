# Katalog modułów i crate’ów Alfy

<!-- Plik generowany: `node scripts/catalog.mjs`. Nie edytuj ręcznie. -->

Jedno miejsce, w którym widać cały kod Rust: do jakiej grupy należy moduł, w której fali powstał, z jakich
crate’ów się składa, jak jest duży i gdzie leży jego specyfikacja. Grupy i fale pochodzą z indeksu
[`docs/modules/README.md`](modules/README.md), rodzaj i izolacja z `module.toml`, liczby z kodu.

| Miara | Wartość |
| --- | --- |
| Crate’y | 246 |
| Moduły (wspólna baza nazwy) | 110 |
| Specyfikacje w indeksie | 89 |
| Manifesty `module.toml` | 68 |
| Linie Rust (z testami) | 392 527 |
| Testy (`#[test]`, `#[tokio::test]`) | 2707 |

Kolumna „Crate’y”: C — `-contract`, I — `-impl`, F — `-fake`; „pojedynczy” — crate bez trójki.
Stan kryteriów akceptacji: [`STATUS.md`](STATUS.md). Zasady zależności między crate’ami: [`crates/README.md`](../crates/README.md).

## Rdzeń

| Moduł | Fala | Rodzaj | Izolacja | Crate’y | Linie | Testy | SPEC | Opis |
| --- | --- | --- | --- | --- | ---: | ---: | --- | --- |
| `core-bus` | F0 | — | — | C · I · F | 1413 | 24 | [core-bus](modules/core-bus/SPEC.md) | Magistrala typowanych zdarzeń między modułami, UI i procesami potomnymi. |
| `core-config` | F0 | service | inproc | C · I · F | 3011 | 31 | [core-config](modules/core-config/SPEC.md) | Konfiguracja TOML warstwowa (wspólna, maszyna, sesja, agentka) z JSON Schema i przeładowaniem; klucze `kernel.*` tylko przez Brokera. |
| `core-log` | F0–F1 | service | inproc | C · I · F | 2388 | 30 | [core-log](modules/core-log/SPEC.md) | Trwały zapis strumieni zdarzeń (NDJSON, rotacja, retencja, redakcja) i audyt `pre-broker` z łańcuchem SHA-256. |
| `core-registry` | F0 | service | inproc | C · I · F | 3683 | 33 | [core-registry](modules/core-registry/SPEC.md) | Rejestr manifestów `module.toml`, graf zależności, cykl życia i health-check modułów. |

## Platforma i system

| Moduł | Fala | Rodzaj | Izolacja | Crate’y | Linie | Testy | SPEC | Opis |
| --- | --- | --- | --- | --- | ---: | ---: | --- | --- |
| `device-profile` | F1 | service | inproc | C · I · F | 2649 | 22 | [device-profile](modules/device-profile/SPEC.md) | Autodetekcja sprzętu, klasa maszyny, rekomendacja profilu głosu A–D i budżetów, tryb baterii i gry. |
| `platform` | — | — | — | C · F | 14 317 | 111 | kontrakt wspólny: platform-windows, platform-apps |  |
| `platform-apps` | F6 | — | — | C · F | 4837 | 25 | [platform-apps](modules/platform-apps/SPEC.md) | Porty Office COM, izolowanej przeglądarki (CDP przez potok, filtr egressu) i rejestru tylko do odczytu z deny-listą sekretów. |
| `platform-windows` | F1, F5, F6 | service | inproc | I | 7844 | 70 | [platform-windows](modules/platform-windows/SPEC.md) | Jedyne miejsce z windows-rs za `SystemPort`: pliki, procesy, schowek, okna, skróty, UIA, wejście, zrzuty, ConPTY, sygnały systemowe. |
| `platform-windows-gui` | F1, F5, F6 | — | — | I | 3008 | 7 | [platform-windows](modules/platform-windows/SPEC.md) |  |
| `platform-windows-kernel` | F1, F5, F6 | — | — | I | 2161 | 5 | [platform-windows](modules/platform-windows/SPEC.md) |  |
| `platform-windows-ocr` | F1, F5, F6 | — | — | I | 266 | 2 | [platform-windows](modules/platform-windows/SPEC.md) |  |
| `platform-windows-office` | F1, F5, F6 | — | — | I | 3507 | 10 | [platform-windows](modules/platform-windows/SPEC.md) |  |
| `platform-windows-pty` | F1, F5, F6 | — | — | I | 694 | 8 | [platform-windows](modules/platform-windows/SPEC.md) |  |
| `platform-windows-sys` | F1, F5, F6 | — | — | I | 3603 | 26 | [platform-windows](modules/platform-windows/SPEC.md) |  |
| `updater` | F1, F3 | service | inproc | C · I · F | 6317 | 49 | [updater](modules/updater/SPEC.md) | Stały launcher, wersje obok siebie, podpis minisign, rollback i ochrona przed pętlą awarii. |

## Dane

| Moduł | Fala | Rodzaj | Izolacja | Crate’y | Linie | Testy | SPEC | Opis |
| --- | --- | --- | --- | --- | ---: | ---: | --- | --- |
| `artifacts` | F1 | service | inproc | C · I · F | 1944 | 15 | [artifacts](modules/artifacts/SPEC.md) | Rejestr plików oddanych przez agentki: wersje, podgląd, diff, akcje jako intencje. |
| `memory` | F1, F7 | service | inproc | C · I · F | 10 526 | 35 | [memory](modules/memory/SPEC.md) | Pamięć w 4 warstwach i 4 zakresach z proweniencją, wersjami faktów, Inspektorem i kaskadowym `forget`. |
| `memory-consolidation` | F7 | service | inproc | C · I · F | 2901 | 19 | [memory-consolidation](modules/memory-consolidation/SPEC.md) | Strażniczka pamięci: nocna konsolidacja z budżetem, dziennikiem i cofaniem, nie na baterii ani w grze. |
| `search` | F1 | service | inproc | C · I · F | 4011 | 31 | [search](modules/search/SPEC.md) | Wyszukiwanie FTS5 i wektorowe w bazie sesji, hybryda RRF; „szukaj wszędzie” tylko dla właściciela. |
| `sessions` | F1 | service | inproc | C · I · F | 5512 | 39 | [sessions](modules/sessions/SPEC.md) | Sesje z osobną szyfrowaną bazą, historia append-only jako drzewo gałęzi, usłyszany prefiks, crypto-shredding. |
| `transfer` | F1, F7 | service | inproc | C · I · F | 9085 | 54 | [transfer](modules/transfer/SPEC.md) | Paczki `.alfa`: eksport i import z podglądem, trybami, szyfrowaniem hasłem, snapshotem i kopiami z rotacją. |

## Modele, mosty i zgodność

| Moduł | Fala | Rodzaj | Izolacja | Crate’y | Linie | Testy | SPEC | Opis |
| --- | --- | --- | --- | --- | ---: | ---: | --- | --- |
| `accounts-hub` | F1 | service | inproc | C · I · F | 6285 | 36 | [accounts-hub](modules/accounts-hub/SPEC.md) | Katalog dostawców, konta i klucze w Menedżerze poświadczeń, kreator, wykrywanie mostów CLI. |
| `agent-backends` | F4 | service | process | C · I · F | 7027 | 48 | [agent-backends](modules/agent-backends/SPEC.md) | Mosty do oficjalnych CLI (Claude Code, Codex) jako „opaque worker” w worktree, bez dostępu do poświadczeń. |
| `compliance` | F1, F4 | service | inproc | C · I · F | 3306 | 30 | [compliance](modules/compliance/SPEC.md) | Rejestr zgodności tras, tagi prywatności i jurysdykcji, deny-listy Jądra z normalizacją ścieżek Windows. |
| `cost-meter` | F1 | service | inproc | C · I · F | 2795 | 24 | [cost-meter](modules/cost-meter/SPEC.md) | Koszty w PLN po kursie NBP, limit miesięczny i budżet tła. |
| `mcp` | F4, F6 | service | inproc | C · I · F | 6442 | 51 | [mcp](modules/mcp/SPEC.md) | Klient MCP z odciskiem opisów narzędzi i serwer MCP Alfy (v0: schowek, okna; v1: UIA, zrzuty, rejestr) bez TCP. |
| `model-residency` | F2 | service | inproc | C · I · F | 2957 | 21 | [model-residency](modules/model-residency/SPEC.md) | Zarządca RAM/VRAM: dzierżawy modeli, wymiana wg priorytetów, tryb gry i baterii. |
| `providers` | — | — | — | C · F | 4733 | 44 | kontrakt wspólny: providers-api, providers-local |  |
| `providers-api` | F1 | provider | inproc | I | 4976 | 38 | [providers-api](modules/providers-api/SPEC.md) | Adaptery `ModelProvider` dla Anthropic, OpenAI i endpointów zgodnych; strumień z anulowaniem. |
| `providers-local` | F1 | provider | process | I | 5298 | 49 | [providers-local](modules/providers-local/SPEC.md) | llama.cpp jako sidecar `llama-server`, pobieranie modelu z SHA-256, fallback GPU → CPU. |
| `router` | F1 | service | inproc | C · I · F | 4448 | 27 | [router](modules/router/SPEC.md) | Wybór trasy wg klasy zadania i ograniczeń, fallback ≤ 2 s, circuit breaker, decyzja z uzasadnieniem. |

## Głos

| Moduł | Fala | Rodzaj | Izolacja | Crate’y | Linie | Testy | SPEC | Opis |
| --- | --- | --- | --- | --- | ---: | ---: | --- | --- |
| `voice-audio` | F2 | voice-engine | inproc | C · I · F | 5538 | 43 | [voice-audio](modules/voice-audio/SPEC.md) | WASAPI: urządzenia, wątek RT bez alokacji, mikser z duckingiem, licznik próbek, referencja AEC. |
| `voice-cmd` | F2 | service | inproc | C · I · F | 2026 | 21 | [voice-cmd](modules/voice-cmd/SPEC.md) | Szybkie komendy głosowe PL/EN bez LLM („stop”, „czekaj”, „przełącz na Deltę”…). |
| `voice-dialog` | F2 | service | inproc | C · I · F | 4683 | 28 | [voice-dialog](modules/voice-dialog/SPEC.md) | Automat rozmowy: ducking i twardy stop, backchannel, usłyszany prefiks, 6 klas intencji przerwania. |
| `voice-dictation` | F5 | voice-engine | inproc | C · I · F | 2711 | 14 | [voice-dictation](modules/voice-dictation/SPEC.md) | Dyktowanie do okna docelowego z normalizacją odwrotną PL; nigdy do okien Alfy ani pól haseł. |
| `voice-dsp` | F2 | voice-engine | inproc | C · I · F | 2923 | 28 | [voice-dsp](modules/voice-dsp/SPEC.md) | AEC3 z własną referencją TTS, redukcja szumu, AGC; fbank log-mel dla słów wywoławczych i mówcy. |
| `voice-persona` | F2 | service | inproc | C · I · F | 5362 | 45 | [voice-persona](modules/voice-persona/SPEC.md) | Normalizator PL do mowy, słownik wymowy, chunker i planista stylu per silnik. |
| `voice-pipeline` | F2 | service | inproc | C · I · F | 9094 | 52 | [voice-pipeline](modules/voice-pipeline/SPEC.md) | Runtime potoku: mikrofon → DSP → VAD → STT → dialog → TTS → wyjście, z przerywaniem i runnerem zestawu F2. |
| `voice-readaloud` | F5 | voice-engine | inproc | C · I · F | 1930 | 7 | [voice-readaloud](modules/voice-readaloud/SPEC.md) | Czytanie zaznaczenia lub okna przez UIA `TextPattern` głosem agentki; treść niezaufana. |
| `voice-s2s` | F5 | — | — | C · F | 1088 | 6 | [voice-s2s](modules/voice-s2s/SPEC.md) | Kontrakt trybu speech-to-speech w chmurze z obcinaniem odpowiedzi po stronie dostawcy (adapter później). |
| `voice-speaker` | F5 | voice-engine | inproc | C · I · F | 2780 | 15 | [voice-speaker](modules/voice-speaker/SPEC.md) | Weryfikacja właściciela (ECAPA), profil szyfrowany; ryzyko głosem bez weryfikacji → potwierdzenie nie-głosem. |
| `voice-stt` | F2 | voice-engine | process | C · I · F | 4497 | 38 | [voice-stt](modules/voice-stt/SPEC.md) | whisper.cpp jako sidecar `whisper-server`: transkrypt częściowy i końcowy, hotwords, fallback GPU → CPU. |
| `voice-tts` | F2 | voice-engine | process | C · I · F | 3404 | 25 | [voice-tts](modules/voice-tts/SPEC.md) | Pocket TTS i Piper, głosy v0 (wysokość, tempo), łańcuch zapasowy per agentka, cache fraz. |
| `voice-turn` | F2 | voice-engine | inproc | C · I · F | 1978 | 29 | [voice-turn](modules/voice-turn/SPEC.md) | Wykrywanie końca tury z polityką cierpliwości wobec hezytacji. |
| `voice-vad` | F2 | voice-engine | inproc | C · I · F | 1808 | 12 | [voice-vad](modules/voice-vad/SPEC.md) | Silero VAD przez `tract-onnx` z progiem adaptacyjnym — bramka dla STT i sygnał przerwania. |
| `voice-wake` | F2, F5 | voice-engine | inproc | C · I · F | 4922 | 27 | [voice-wake](modules/voice-wake/SPEC.md) | PTT, przełącznik, adresowanie po imieniu; v1 — słowa wywoławcze „Hej …” lokalnie, domyślnie wyłączone. |

## Jądro bezpieczeństwa

| Moduł | Fala | Rodzaj | Izolacja | Crate’y | Linie | Testy | SPEC | Opis |
| --- | --- | --- | --- | --- | ---: | ---: | --- | --- |
| `broker-ui` | F3 | service | process | C · I · F | 2656 | 20 | [broker-ui](modules/broker-ui/SPEC.md) | Natywne okno zatwierdzeń na wyższym poziomie integralności; decyzje tylko z fizycznego wejścia. |
| `risk-classifier` | F3 | service | inproc | C · I · F | 1945 | 22 | [risk-classifier](modules/risk-classifier/SPEC.md) | Deterministyczna ocena ryzyka akcji: odwracalność, zakres, egress, głos, taint, trifecta. |
| `safety-broker` | F3 | service | process | C · I · F | 10 179 | 64 | [safety-broker](modules/safety-broker/SPEC.md) | Tokeny zdolności, poziomy L0–L4, twarde blokady Jądra, zatwierdzenia, Audyt z łańcuchem hashy, kill-switch. |
| `undo-journal` | F3 | service | inproc | C · I · F | 2907 | 16 | [undo-journal](modules/undo-journal/SPEC.md) | Dziennik cofania `fs.*` z pre-image i snapshotami zakresu; „Cofnij” jednym kliknięciem. |
| `watchdog` | F3 | service | process | C · I · F | 1855 | 15 | [watchdog](modules/watchdog/SPEC.md) | Heartbeat, restart z limitem, safe-mode, rollback, Job Objects i kill-switch poza UI. |

## Agentki i narzędzia

| Moduł | Fala | Rodzaj | Izolacja | Crate’y | Linie | Testy | SPEC | Opis |
| --- | --- | --- | --- | --- | ---: | ---: | --- | --- |
| `agent-builder` | F5 | service | inproc | C · I · F | 3118 | 17 | [agent-builder](modules/agent-builder/SPEC.md) | Kreator agentek: szkic → podgląd → test na sucho → zapis po zatwierdzeniu; autonomia nigdy L4. |
| `agent-runtime` | F3, F5 | service | inproc | C · I · F | 8345 | 62 | [agent-runtime](modules/agent-runtime/SPEC.md) | Pętla agentki z budżetami i checkpointami; v1 — równoległość, delegacja z atenuacją, Krytyczka, steering. |
| `marshal` | F5 | service | inproc | C · I · F | 3307 | 9 | [marshal](modules/marshal/SPEC.md) | Marszałek: polecenia → reguły, które tylko zawężają; nadzór postępu i raport dnia. |
| `personas` | F2 | service | inproc | C · I · F | 3821 | 32 | [personas](modules/personas/SPEC.md) | Cztery persony, katalog ról, obsady i szablony, adresowanie z polską odmianą imion. |
| `scheduler` | F5 | service | inproc | C · I · F | 7599 | 29 | [scheduler](modules/scheduler/SPEC.md) | DAG zadań, równoległe agentki z atomowym przydziałem zasobów, priorytety voice-first, trwałość. |
| `scheduler-lite` | F2 | service | inproc | C · I · F | 3417 | 15 | [scheduler-lite](modules/scheduler-lite/SPEC.md) | Zasoby wyłączne (mikrofon, głośnik), kolejka mowy, wykrywanie zakleszczeń. |
| `skills` | F5 | service | inproc | C · I · F | 3194 | 20 | [skills](modules/skills/SPEC.md) | Wersjonowane umiejętności, instalacja po przeglądzie z hashem, kwarantanna treści z zewnątrz. |
| `tools-browser` | F6 | tool | inproc | C · I · F | 1572 | 8 | [tools-browser](modules/tools-browser/SPEC.md) | Przeglądarka z profilem Alfy i CDP przez potok; każdy host przez Brokera; bez haseł; pobrania w kwarantannie. |
| `tools-clipboard` | F3 | tool | inproc | C · I · F | 1060 | 10 | [tools-clipboard](modules/tools-clipboard/SPEC.md) | Odczyt (taint) i zapis schowka z cofaniem; historia schowka — szkic. |
| `tools-common` | F3 | — | — | C | 2071 | 22 | [tools-common](modules/tools-common/SPEC.md) | Wspólny kontrakt narzędzi: manifest, `Tool`, `BrokerGate`, zasady ścieżek i treści niezaufanej. |
| `tools-fs` | F3 | tool | inproc | C · I · F | 2758 | 24 | [tools-fs](modules/tools-fs/SPEC.md) | 11 narzędzi plikowych przez Brokera i dziennik cofania; usuwanie do Kosza. |
| `tools-input` | F5–F6 | tool | inproc | C · I · F | 1694 | 16 | [tools-input](modules/tools-input/SPEC.md) | `SendInput` ze strażnikiem celów i przerwaniem przy fizycznym wejściu użytkownika. |
| `tools-media` | F6 | tool | inproc | C · I · F | 3271 | 25 | [tools-media](modules/tools-media/SPEC.md) | Nagłówki multimediów bez ffmpeg, konwersje przez sidecar ffmpeg jako nowy plik z „Cofnij”, odtwarzanie w kolejce mówienia z duckingiem. |
| `tools-net` | F6 | tool | inproc | C · I · F | 3072 | 29 | [tools-net](modules/tools-net/SPEC.md) | `net_fetch`/`net_download` tylko https do hostów publicznych, każdy host i przekierowanie przez Brokera, rebinding odrzucany, kwarantanna z SHA-256 i MOTW. |
| `tools-office` | F6 | tool | inproc | C · I · F | 1703 | 12 | [tools-office](modules/tools-office/SPEC.md) | Word/Excel przez COM: odczyt (niezaufany) i edycja kopii jako nowa wersja z „Cofnij”; makra wyłączone. |
| `tools-screen` | F6 | tool | inproc | C · I · F | 884 | 8 | [tools-screen](modules/tools-screen/SPEC.md) | Zrzuty na żądanie z maskowaniem okien Alfy/Brokera, deny-listy i pól haseł. |
| `tools-shell` | F3 | tool | process | C · I · F | 2127 | 26 | [tools-shell](modules/tools-shell/SPEC.md) | Polecenia w zakresie katalogu: snapshot, Job Object, filtrowane środowisko, „uruchom w terminalu”. |
| `tools-system` | F6 | tool | inproc | C · I · F | 2784 | 22 | [tools-system](modules/tools-system/SPEC.md) | Procesy (zakończenie ze strażnikiem celów i tożsamością), usługi przez `system.admin`, Dziennik zdarzeń, zmienne z cofaniem, stan systemu. |
| `tools-uia` | F5–F6 | tool | inproc | C · I · F | 1586 | 11 | [tools-uia](modules/tools-uia/SPEC.md) | Drzewo UIA, odczyt `TextPattern`, akcje tylko przez wzorce; wartości haseł nigdy nie wychodzą. |
| `tools-vision` | F6 | tool | inproc | C · I · F | 2609 | 20 | [tools-vision](modules/tools-vision/SPEC.md) | OCR zrzutu/pliku (`Windows.Media.Ocr`) i opis obrazu przez Router „GUI/wizja”; maskowanie przed OCR i modelem; sesja prywatna tylko lokalnie. |
| `tools-window` | F6 | tool | inproc | C · I · F | 1398 | 11 | [tools-window](modules/tools-window/SPEC.md) | Okna i monitory dla agentek oraz wspólna bramka GUI (`gui.control`). |
| `triggers` | F5 | service | inproc | C · I · F | 4676 | 19 | [triggers](modules/triggers/SPEC.md) | Wyzwalacze czasowe (cron w strefie PL z DST), zdarzeniowe i ręczne; nigdy nie uruchamiają mostu CLI. |
| `ui-terminal` | F4 | ui-panel | inproc | C · I · F | 1103 | 8 | [ui-terminal](modules/ui-terminal/SPEC.md) | Terminal ConPTY sterowany wyłącznie przez użytkownika (logowanie do CLI); treść poza logami i zdarzeniami. |

## Samonaprawa i ulepszanie

| Moduł | Fala | Rodzaj | Izolacja | Crate’y | Linie | Testy | SPEC | Opis |
| --- | --- | --- | --- | --- | ---: | ---: | --- | --- |
| `diagnostician` | F8 | service | inproc | C · I · F | 7170 | 22 | [diagnostician](modules/diagnostician/SPEC.md) | Diagnosta: katalog 24 awarii, naprawy cofalne z weryfikacją, raport „Zdrowie systemu”. |
| `evals` | F8 | service | inproc | C · I · F | 4526 | 33 | [evals](modules/evals/SPEC.md) | Harness zestawów: manifest z SHA-256, podziały dev/test/holdout, bootstrap, bramka holdoutu w Jądrze. |
| `improver` | F8 | service | inproc | C · I · F | 4022 | 18 | [improver](modules/improver/SPEC.md) | Ulepszacz R0–R2: propozycje zmian ustawień przez piaskownicę i holdout; nigdy Jądro ani progi. |
| `plugin-runtime` | F8 | service | inproc | C · I · F | 5865 | 37 | [plugin-runtime](modules/plugin-runtime/SPEC.md) | Wtyczki Wasm (wasmtime, WIT bez WASI): jedyny import `host.call` przez Brokera, paliwo, epoki, limity; zatwierdzenie z hashem. |

## Planowane (plan v6)

| Moduł | Fala | Rodzaj | Izolacja | Crate’y | Linie | Testy | SPEC | Opis |
| --- | --- | --- | --- | --- | ---: | ---: | --- | --- |
| `camera-hub` | v6-C3 | service | inproc | C · I · F | 2106 | 15 | [camera-hub](modules/camera-hub/SPEC.md) | System kamer. **v0 jest w kodzie (0.0.16): kamery USB**, kilka strumieni naraz, klatka na żądanie, podgląd, usypianie; bez nagrywania. Kamery sieciowe — dalej. |

## Kompozycja aplikacji (`app-*`)

| Moduł | Fala | Rodzaj | Izolacja | Crate’y | Linie | Testy | SPEC | Opis |
| --- | --- | --- | --- | --- | ---: | ---: | --- | --- |
| `app-agents` | — | — | — | pojedynczy | 4258 | 26 | — |  |
| `app-api` | — | — | — | pojedynczy | 7165 | 32 | — |  |
| `app-bridges` | — | — | — | pojedynczy | 839 | 6 | — |  |
| `app-broker` | — | — | — | pojedynczy | 4287 | 23 | — |  |
| `app-chat` | — | — | — | pojedynczy | 2164 | 6 | — |  |
| `app-core` | — | — | — | pojedynczy | 16 258 | 109 | — |  |
| `app-files` | — | — | — | pojedynczy | 3184 | 26 | — |  |
| `app-gui` | — | — | — | pojedynczy | 1177 | 8 | — |  |
| `app-health` | — | — | — | pojedynczy | 1482 | 6 | — |  |
| `app-logs` | — | — | — | pojedynczy | 1671 | 22 | — |  |
| `app-memory` | — | — | — | pojedynczy | 1618 | 7 | — |  |
| `app-models` | — | — | — | pojedynczy | 7543 | 46 | — |  |
| `app-modules` | — | — | — | pojedynczy | 4559 | 27 | — |  |
| `app-plugins` | — | — | — | pojedynczy | 1683 | 19 | — |  |
| `app-safety` | — | — | — | pojedynczy | 1212 | 10 | — |  |
| `app-skills` | — | — | — | pojedynczy | 880 | 1 | — |  |
| `app-store` | — | — | — | pojedynczy | 468 | 0 | — |  |
| `app-tasks` | — | — | — | pojedynczy | 3071 | 7 | — |  |
| `app-terminal` | — | — | — | pojedynczy | 441 | 2 | — |  |
| `app-updates` | — | — | — | pojedynczy | 1301 | 13 | — |  |
| `app-voice` | — | — | — | pojedynczy | 5620 | 26 | — |  |

## Biblioteki wspólne (`lib-*`)

| Moduł | Fala | Rodzaj | Izolacja | Crate’y | Linie | Testy | SPEC | Opis |
| --- | --- | --- | --- | --- | ---: | ---: | --- | --- |
| `lib-embed` | — | — | — | pojedynczy | 4572 | 44 | — |  |
| `lib-markdown` | — | — | — | pojedynczy | 1979 | 25 | — |  |
| `lib-media` | — | — | — | pojedynczy | 2049 | 17 | — |  |
| `lib-netguard` | — | — | — | pojedynczy | 649 | 7 | — |  |
| `lib-openai-compat` | — | — | — | pojedynczy | 1714 | 11 | — |  |
| `lib-smart-turn` | — | — | — | pojedynczy | 588 | 7 | — |  |
| `lib-sqlstore` | — | — | — | pojedynczy | 1321 | 30 | — |  |
| `lib-supertonic` | — | — | — | pojedynczy | 814 | 11 | — |  |

## Wzorce i spike’i

| Moduł | Fala | Rodzaj | Izolacja | Crate’y | Linie | Testy | SPEC | Opis |
| --- | --- | --- | --- | --- | ---: | ---: | --- | --- |
| `example-module` | — | service | inproc | C · I · F | 384 | 8 | — |  |
| `spike-data` | — | — | — | pojedynczy | 488 | 4 | — |  |

## Niespójności do uporządkowania

| Reguła | Liczba | Pozycje |
| --- | ---: | --- |
| SPEC w indeksie bez crate’a w `crates/` (moduły UI i powłoki są poza `crates/`) | 6 | `models`, `shell-integration`, `notify`, `ui-kit`, `ui-shell`, `ui-quick` |
| SPEC modułów planowanych, jeszcze bez kodu (plan v6 — nie niespójność) | 12 | `platform-select`, `remote-gateway`, `device-pairing`, `remote-relay`, `host-headless`, `web-panel`, `offline-client`, `platform-android`, `platform-linux`, `vision-describe`, `person-recognition`, `person-memory` |
| Moduł z trójką crate’ów bez SPEC | 1 | `example-module` |
| Crate `-impl` bez `module.toml` | 1 | `core-bus-impl` |
| Części modułów wielocrate’owych (manifest w crate’cie głównym — nie niespójność) | 6 | `platform-windows-gui-impl`, `platform-windows-kernel-impl`, `platform-windows-ocr-impl`, `platform-windows-office-impl`, `platform-windows-pty-impl`, `platform-windows-sys-impl` |
| Crate bez `README.md` | 0 | brak |
| Plik Rust powyżej 400 linii | 6 | `crates/router-impl/tests/decisions.rs` (433), `crates/router-impl/src/routing.rs` (427), `crates/app-modules/src/probe.rs` (421), `crates/accounts-hub-fake/src/hub.rs` (419), `crates/voice-tts-impl/tests/tts.rs` (412), `crates/app-models/src/data.rs` (406) |
| Crate powyżej 8000 linii w `src/` | 0 | brak |
