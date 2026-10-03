# Specyfikacje modułów (`docs/modules/<moduł>/SPEC.md`)

Każdy moduł programu Alfa ma jedną, jednostronicową specyfikację (SPEC). SPEC pisze autor modułu (model AI), akceptuje recenzent (drugi model); SPEC-i Jądra i Brokera akceptuje właściciel (PLAN §4.1). Kolejność pracy nad modułem: **SPEC → kontrakt → fake → testy → implementacja → przegląd → CI → merge** (PLAN §4.3).

Zasady:
- Moduł = trójka crate'ów `<m>-contract`, `<m>-impl`, `<m>-fake` + manifest `module.toml` (PLAN §3.2). Inne moduły zależą **tylko od `-contract`**.
- SPEC ma mieścić się w małym oknie kontekstu (cel: 40–90 linii). Szczegóły idą do ADR, `docs/vendor/` albo do kodu.
- Szkic Rust w sekcji „Kontrakt" jest **orientacyjny** — źródłem prawdy staje się crate `-contract` po jego powstaniu; SPEC się wtedy aktualizuje (DoD pkt 1).
- Nie wymyślamy decyzji spoza PLAN.md. Braki oznaczamy „do ustalenia w SPEC v1" (albo wskazujemy ADR/spike).
- ID testów akceptacyjnych: `ACC-F<fala>-<moduł>-<nr>` (np. `ACC-F1-sessions-01`). Rejestr ID i progi: `docs/ACCEPTANCE.md`; zestawy zamrażane hashem w `evals/`.

## Szablon SPEC

```markdown
# <moduł> — SPEC (szkic v0)

## Cel
Jedno–trzy zdania: co moduł robi i czego NIE robi.

## Fala i priorytet
Fala (§16.2), priorytet P0/P1/P2, wersje (v0/v1) i co wchodzi w każdą.

## Kontrakt (szkic Rust)
trait główny + 2–5 kluczowych typów + nazwy zdarzeń na magistrali (`<moduł>.<zdarzenie>`).
Oznaczony jako szkic; JSON Schema zdarzeń w `packages/schemas/`.

## Zależności
Tylko crate'y `-contract` innych modułów (+ `core-*`). Zależności zewnętrzne w `docs/vendor/`.

## Niezmienniki
Reguły, których implementacja nigdy nie łamie (testowane property-based / kontraktowo).

## Zdolności / uprawnienia
Tokeny zdolności z Brokera, jakich moduł potrzebuje (`fs.read(zakres)`, `net.egress(host)`…), lub „brak".

## Izolacja
`inproc` | `process` | `wasm` + cykl życia (`always` | `lazy` | `on-demand`) + wątek RT, jeśli dotyczy.

## Budżet zasobów
RAM / CPU / opóźnienie deklarowane w `module.toml` (wstępne; zaostrzane po pomiarach F0 na baseline).

## Konfiguracja (klucze TOML)
Klucze w `%APPDATA%\Alfa\config\*.toml` (wspólne) lub `config/machine/<id>.toml` (per maszyna), z domyślnymi.

## Wkład do UI
Panel / strona ustawień / elementy composera / zasobnik — lub „brak".

## Testy akceptacyjne
Lista ID `ACC-…` z progami (z §16.2 / ACCEPTANCE.md).

## Fake
Co udaje crate `-fake` i jak jest sterowany (fixture'y, wirtualny zegar, record/replay).

## Otwarte pytania
Punkty „do ustalenia w SPEC v1", odwołania do ADR i spike'ów.
```

## Indeks SPEC-ów (stan: październik 2026)

67 specyfikacji. Wersję i stan SPEC-a (szkic, v0, v1 „zaimplementowany”) podaje jego nagłówek; zmiany po
implementacji są w sekcjach „Zmiany po implementacji” / „Implementacja” na końcu pliku. Stan kryteriów akceptacji:
`docs/STATUS.md`; crate'y: `crates/README.md`.

### Rdzeń

| Moduł                                    | Fala  | Opis                                                                                                                                  |
| ---------------------------------------- | ----- | ------------------------------------------------------------------------------------------------------------------------------------- |
| [`core-bus`](core-bus/SPEC.md)           | F0    | Magistrala typowanych zdarzeń między modułami, UI i procesami potomnymi.                                                              |
| [`core-registry`](core-registry/SPEC.md) | F0    | Rejestr manifestów `module.toml`, graf zależności, cykl życia i health-check modułów.                                                 |
| [`core-config`](core-config/SPEC.md)     | F0    | Konfiguracja TOML warstwowa (wspólna, maszyna, sesja, agentka) z JSON Schema i przeładowaniem; klucze `kernel.*` tylko przez Brokera. |
| [`core-log`](core-log/SPEC.md)           | F0–F1 | Trwały zapis strumieni zdarzeń (NDJSON, rotacja, retencja, redakcja) i audyt `pre-broker` z łańcuchem SHA-256.                        |

### Platforma i system

| Moduł                                            | Fala       | Opis                                                                                                                                 |
| ------------------------------------------------ | ---------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| [`platform-windows`](platform-windows/SPEC.md)   | F1, F5, F6 | Jedyne miejsce z windows-rs za `SystemPort`: pliki, procesy, schowek, okna, skróty, UIA, wejście, zrzuty, ConPTY, sygnały systemowe. |
| [`platform-apps`](platform-apps/SPEC.md)         | F6         | Porty Office COM, izolowanej przeglądarki (CDP przez potok, filtr egressu) i rejestru tylko do odczytu z deny-listą sekretów. |
| [`device-profile`](device-profile/SPEC.md)       | F1         | Autodetekcja sprzętu, klasa maszyny, rekomendacja profilu głosu A–D i budżetów, tryb baterii i gry.                                  |
| [`updater`](updater/SPEC.md)                     | F1, F3     | Stały launcher, wersje obok siebie, podpis minisign, rollback i ochrona przed pętlą awarii.                                          |
| [`shell-integration`](shell-integration/SPEC.md) | F1         | Zasobnik, skróty globalne, protokół `alfa://`, jedna instancja, dialogi plików — w powłoce Tauri.                                    |
| [`notify`](notify/SPEC.md)                       | F1         | Powiadomienia Windows i w aplikacji, tryb „nie przeszkadzać” — w `app-core`.                                                         |

### Dane

| Moduł                                                  | Fala   | Opis                                                                                                          |
| ------------------------------------------------------ | ------ | ------------------------------------------------------------------------------------------------------------- |
| [`sessions`](sessions/SPEC.md)                         | F1     | Sesje z osobną szyfrowaną bazą, historia append-only jako drzewo gałęzi, usłyszany prefiks, crypto-shredding. |
| [`search`](search/SPEC.md)                             | F1     | Wyszukiwanie FTS5 i wektorowe w bazie sesji, hybryda RRF; „szukaj wszędzie” tylko dla właściciela.            |
| [`memory`](memory/SPEC.md)                             | F1, F7 | Pamięć w 4 warstwach i 4 zakresach z proweniencją, wersjami faktów, Inspektorem i kaskadowym `forget`.        |
| [`memory-consolidation`](memory-consolidation/SPEC.md) | F7     | Strażniczka pamięci: nocna konsolidacja z budżetem, dziennikiem i cofaniem, nie na baterii ani w grze.        |
| [`artifacts`](artifacts/SPEC.md)                       | F1     | Rejestr plików oddanych przez agentki: wersje, podgląd, diff, akcje jako intencje.                            |
| [`transfer`](transfer/SPEC.md)                         | F1, F7 | Paczki `.alfa`: eksport i import z podglądem, trybami, szyfrowaniem hasłem, snapshotem i kopiami z rotacją.   |

### Modele, mosty i zgodność

| Moduł                                        | Fala   | Opis                                                                                                       |
| -------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------- |
| [`accounts-hub`](accounts-hub/SPEC.md)       | F1     | Katalog dostawców, konta i klucze w Menedżerze poświadczeń, kreator, wykrywanie mostów CLI.                |
| [`providers-api`](providers-api/SPEC.md)     | F1     | Adaptery `ModelProvider` dla Anthropic, OpenAI i endpointów zgodnych; strumień z anulowaniem.              |
| [`providers-local`](providers-local/SPEC.md) | F1     | llama.cpp jako sidecar `llama-server`, pobieranie modelu z SHA-256, fallback GPU → CPU.                    |
| [`router`](router/SPEC.md)                   | F1     | Wybór trasy wg klasy zadania i ograniczeń, fallback ≤ 2 s, circuit breaker, decyzja z uzasadnieniem.       |
| [`cost-meter`](cost-meter/SPEC.md)           | F1     | Koszty w PLN po kursie NBP, limit miesięczny i budżet tła.                                                 |
| [`model-residency`](model-residency/SPEC.md) | F2     | Zarządca RAM/VRAM: dzierżawy modeli, wymiana wg priorytetów, tryb gry i baterii.                           |
| [`compliance`](compliance/SPEC.md)           | F1, F4 | Rejestr zgodności tras, tagi prywatności i jurysdykcji, deny-listy Jądra z normalizacją ścieżek Windows.   |
| [`agent-backends`](agent-backends/SPEC.md)   | F4     | Mosty do oficjalnych CLI (Claude Code, Codex) jako „opaque worker” w worktree, bez dostępu do poświadczeń. |
| [`mcp`](mcp/SPEC.md)                         | F4, F6 | Klient MCP z odciskiem opisów narzędzi i serwer MCP Alfy (v0: schowek, okna; v1: UIA, zrzuty, rejestr) bez TCP. |

### Głos

| Moduł                                        | Fala   | Opis                                                                                                          |
| -------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------- |
| [`voice-audio`](voice-audio/SPEC.md)         | F2     | WASAPI: urządzenia, wątek RT bez alokacji, mikser z duckingiem, licznik próbek, referencja AEC.               |
| [`voice-dsp`](voice-dsp/SPEC.md)             | F2     | AEC3 z własną referencją TTS, redukcja szumu, AGC; fbank log-mel dla słów wywoławczych i mówcy.               |
| [`voice-vad`](voice-vad/SPEC.md)             | F2     | Silero VAD przez `tract-onnx` z progiem adaptacyjnym — bramka dla STT i sygnał przerwania.                    |
| [`voice-stt`](voice-stt/SPEC.md)             | F2     | whisper.cpp jako sidecar `whisper-server`: transkrypt częściowy i końcowy, hotwords, fallback GPU → CPU.      |
| [`voice-tts`](voice-tts/SPEC.md)             | F2     | Pocket TTS i Piper, głosy v0 (wysokość, tempo), łańcuch zapasowy per agentka, cache fraz.                     |
| [`voice-turn`](voice-turn/SPEC.md)           | F2     | Wykrywanie końca tury z polityką cierpliwości wobec hezytacji.                                                |
| [`voice-cmd`](voice-cmd/SPEC.md)             | F2     | Szybkie komendy głosowe PL/EN bez LLM („stop”, „czekaj”, „przełącz na Deltę”…).                               |
| [`voice-dialog`](voice-dialog/SPEC.md)       | F2     | Automat rozmowy: ducking i twardy stop, backchannel, usłyszany prefiks, 6 klas intencji przerwania.           |
| [`voice-persona`](voice-persona/SPEC.md)     | F2     | Normalizator PL do mowy, słownik wymowy, chunker i planista stylu per silnik.                                 |
| [`voice-wake`](voice-wake/SPEC.md)           | F2, F5 | PTT, przełącznik, adresowanie po imieniu; v1 — słowa wywoławcze „Hej …” lokalnie, domyślnie wyłączone.        |
| [`voice-pipeline`](voice-pipeline/SPEC.md)   | F2     | Runtime potoku: mikrofon → DSP → VAD → STT → dialog → TTS → wyjście, z przerywaniem i runnerem zestawu F2.    |
| [`voice-speaker`](voice-speaker/SPEC.md)     | F5     | Weryfikacja właściciela (ECAPA), profil szyfrowany; ryzyko głosem bez weryfikacji → potwierdzenie nie-głosem. |
| [`voice-dictation`](voice-dictation/SPEC.md) | F5     | Dyktowanie do okna docelowego z normalizacją odwrotną PL; nigdy do okien Alfy ani pól haseł.                  |
| [`voice-readaloud`](voice-readaloud/SPEC.md) | F5     | Czytanie zaznaczenia lub okna przez UIA `TextPattern` głosem agentki; treść niezaufana.                       |
| [`voice-s2s`](voice-s2s/SPEC.md)             | F5     | Kontrakt trybu speech-to-speech w chmurze z obcinaniem odpowiedzi po stronie dostawcy (adapter później).      |

### Jądro bezpieczeństwa

| Moduł                                        | Fala | Opis                                                                                                        |
| -------------------------------------------- | ---- | ----------------------------------------------------------------------------------------------------------- |
| [`safety-broker`](safety-broker/SPEC.md)     | F3   | Tokeny zdolności, poziomy L0–L4, twarde blokady Jądra, zatwierdzenia, Audyt z łańcuchem hashy, kill-switch. |
| [`broker-ui`](broker-ui/SPEC.md)             | F3   | Natywne okno zatwierdzeń na wyższym poziomie integralności; decyzje tylko z fizycznego wejścia.             |
| [`watchdog`](watchdog/SPEC.md)               | F3   | Heartbeat, restart z limitem, safe-mode, rollback, Job Objects i kill-switch poza UI.                       |
| [`risk-classifier`](risk-classifier/SPEC.md) | F3   | Deterministyczna ocena ryzyka akcji: odwracalność, zakres, egress, głos, taint, trifecta.                   |
| [`undo-journal`](undo-journal/SPEC.md)       | F3   | Dziennik cofania `fs.*` z pre-image i snapshotami zakresu; „Cofnij” jednym kliknięciem.                     |

### Agentki i narzędzia

| Moduł                                        | Fala   | Opis                                                                                                       |
| -------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------- |
| [`agent-runtime`](agent-runtime/SPEC.md)     | F3, F5 | Pętla agentki z budżetami i checkpointami; v1 — równoległość, delegacja z atenuacją, Krytyczka, steering.  |
| [`personas`](personas/SPEC.md)               | F2     | Cztery persony, katalog ról, obsady i szablony, adresowanie z polską odmianą imion.                        |
| [`scheduler-lite`](scheduler-lite/SPEC.md)   | F2     | Zasoby wyłączne (mikrofon, głośnik), kolejka mowy, wykrywanie zakleszczeń.                                 |
| [`scheduler`](scheduler/SPEC.md)             | F5     | DAG zadań, równoległe agentki z atomowym przydziałem zasobów, priorytety voice-first, trwałość.            |
| [`triggers`](triggers/SPEC.md)               | F5     | Wyzwalacze czasowe (cron w strefie PL z DST), zdarzeniowe i ręczne; nigdy nie uruchamiają mostu CLI.       |
| [`marshal`](marshal/SPEC.md)                 | F5     | Marszałek: polecenia → reguły, które tylko zawężają; nadzór postępu i raport dnia.                         |
| [`agent-builder`](agent-builder/SPEC.md)     | F5     | Kreator agentek: szkic → podgląd → test na sucho → zapis po zatwierdzeniu; autonomia nigdy L4.             |
| [`skills`](skills/SPEC.md)                   | F5     | Wersjonowane umiejętności, instalacja po przeglądzie z hashem, kwarantanna treści z zewnątrz.              |
| [`tools-common`](tools-common/SPEC.md)       | F3     | Wspólny kontrakt narzędzi: manifest, `Tool`, `BrokerGate`, zasady ścieżek i treści niezaufanej.            |
| [`tools-fs`](tools-fs/SPEC.md)               | F3     | 11 narzędzi plikowych przez Brokera i dziennik cofania; usuwanie do Kosza.                                 |
| [`tools-shell`](tools-shell/SPEC.md)         | F3     | Polecenia w zakresie katalogu: snapshot, Job Object, filtrowane środowisko, „uruchom w terminalu”.         |
| [`tools-clipboard`](tools-clipboard/SPEC.md) | F3     | Odczyt (taint) i zapis schowka z cofaniem; historia schowka — szkic.                                       |
| [`tools-window`](tools-window/SPEC.md)       | F6     | Okna i monitory dla agentek oraz wspólna bramka GUI (`gui.control`).                                       |
| [`tools-uia`](tools-uia/SPEC.md)             | F5–F6  | Drzewo UIA, odczyt `TextPattern`, akcje tylko przez wzorce; wartości haseł nigdy nie wychodzą.             |
| [`tools-input`](tools-input/SPEC.md)         | F5–F6  | `SendInput` ze strażnikiem celów i przerwaniem przy fizycznym wejściu użytkownika.                         |
| [`tools-screen`](tools-screen/SPEC.md)       | F6     | Zrzuty na żądanie z maskowaniem okien Alfy/Brokera, deny-listy i pól haseł.                                |
| [`tools-office`](tools-office/SPEC.md)       | F6     | Word/Excel przez COM: odczyt (niezaufany) i edycja kopii jako nowa wersja z „Cofnij”; makra wyłączone.     |
| [`tools-browser`](tools-browser/SPEC.md)     | F6     | Przeglądarka z profilem Alfy i CDP przez potok; każdy host przez Brokera; bez haseł; pobrania w kwarantannie. |
| [`ui-terminal`](ui-terminal/SPEC.md)         | F4     | Terminal ConPTY sterowany wyłącznie przez użytkownika (logowanie do CLI); treść poza logami i zdarzeniami. |

### Samonaprawa i ulepszanie

| Moduł                                    | Fala | Opis                                                                                                  |
| ---------------------------------------- | ---- | ----------------------------------------------------------------------------------------------------- |
| [`diagnostician`](diagnostician/SPEC.md) | F8   | Diagnosta: katalog 24 awarii, naprawy cofalne z weryfikacją, raport „Zdrowie systemu”.                |
| [`improver`](improver/SPEC.md)           | F8   | Ulepszacz R0–R2: propozycje zmian ustawień przez piaskownicę i holdout; nigdy Jądro ani progi.        |
| [`evals`](evals/SPEC.md)                 | F8   | Harness zestawów: manifest z SHA-256, podziały dev/test/holdout, bootstrap, bramka holdoutu w Jądrze. |
| [`plugin-runtime`](plugin-runtime/SPEC.md) | F8 | Wtyczki Wasm (wasmtime, WIT bez WASI): jedyny import `host.call` przez Brokera, paliwo, epoki, limity; zatwierdzenie z hashem. |

### UI (TypeScript)

| Moduł                          | Fala   | Opis                                                                                        |
| ------------------------------ | ------ | ------------------------------------------------------------------------------------------- |
| [`ui-kit`](ui-kit/SPEC.md)     | F0–F1  | Tokeny designu i komponenty Svelte 5 ze Storybookiem, makietami i axe (`packages/ui-kit`).  |
| [`ui-shell`](ui-shell/SPEC.md) | F1–F8  | Okno główne: pasek tytułu, rozmowa, panele, Ustawienia, paleta, skróty (`apps/desktop/ui`). |
| [`ui-quick`](ui-quick/SPEC.md) | F1, F5 | Szybkie pytanie i pigułka głosowa jako lekkie okna; menu zasobnika natywne.                 |

### Moduły z planu bez SPEC-a

`tools-vision`, `tools-system`, `tools-net`, `tools-media`
(F6), `voice-lab` i `voice-transcribe` — SPEC powstaje przed falą, w której moduł jest budowany. Korzeń kompozycji
`app-*` i biblioteki `lib-*` nie mają SPEC-ów (opis w `crates/README.md` i `docs/ARCHITECTURE.md` §13).
