# Alfa — architektura

> Dokument pochodny od `docs/PLAN.md` (§3, §5.1, §8, §13). Plan jest źródłem prawdy; ten dokument go nie zmienia, tylko rozwija. Wartości oznaczone „do ustalenia w F0" nie są rozstrzygnięte w planie.

## 1. Zasady, które kształtują architekturę

| # | Zasada (plan §2) | Skutek architektoniczny |
|---|---|---|
| 1 | Lekkość | moduł nieużywany nie kosztuje RAM/CPU: ładowanie na żądanie, zwalnianie po bezczynności |
| 2 | Mikrojądro | jądro to kilka MB bez logiki domenowej; wszystko inne to moduł z manifestem |
| 3 | Zbudowane dla AI | małe moduły mieszczące się w oknie kontekstu, jeden kontrakt na moduł, `-fake` do testów bez sprzętu |
| 4 | Wszystko jest zdarzeniem | magistrala + append-only dziennik: logi, postępy, cofanie, replay, samodoskonalenie |
| 5 | Przerywalność wszędzie | anulowanie w kontraktach (`stream`, `cancel`), kill-switch poza UI |
| 6 | Najmniejsze uprawnienia | tokeny zdolności, potomek ≤ rodzic, reguły tylko zawężają |
| 7 | Jądro bezpieczeństwa poza zasięgiem agentów | Broker jako osobny proces na osobnym koncie |
| 9 | Fallback na każdej ścieżce | Router, łańcuchy silników głosu, CPU jako zapas GPU |

## 2. Schemat

```
┌────────────────── UI (Tauri 2 · WebView2 · Svelte 5) ──────────────────┐
│ Powierzchnia │ Panele chowane: Sesje·Agentki·Oś czasu·Pliki·Pamięć·Ekran·Głos │ Ustawienia│
└───────────────▲────────────────────────────────────────────────────────┘
                │ typowane komendy + strumień zdarzeń (minimalne capabilities per okno)
┌───────────────┴────────────── JĄDRO „Alfa Core" (Rust, ≈ kilka MB) ────────────────┐
│ Magistrala zdarzeń · Rejestr modułów · Konfiguracja · Klient Brokera · Log-writer   │
│ (bez logiki domenowej; wszystko poniżej to moduły z manifestem)                     │
└──▲────────────────────────▲────────────────────────▲──────────────────────▲───────┘
   │ in-proc (feature flag) │ osobny proces (stdio/pipe)│ Wasm (wasmtime+WIT)   │
┌──┴───────────┐ ┌──────────┴────────┐ ┌────────────────┴───┐ ┌───────────────┴────┐
│ Krytyczne:   │ │ Ciężkie/awaryjne: │ │ Wtyczki AI,        │ │ Zewnętrzne:        │
│ voice-audio, │ │ STT/TTS/LLM local,│ │ umiejętności       │ │ serwery MCP, CLI   │
│ scheduler,   │ │ przeglądarka,     │ │ (sandbox, limity)  │ │ (mosty, „opaque    │
│ memory, router│ │ UIA-helper        │ │                    │ │ worker")           │
└──────────────┘ └───────────────────┘ └────────────────────┘ └────────────────────┘
        ┌──────────────────────────────────────────────────────────────┐
        │ BROKER (usługa w tle, osobne konto): tokeny zdolności,       │  + BROKER-UI (okno
        │ writer audytu, kill-switch, polityki Jądra, operacje z UAC   │  zatwierdzeń w Twojej
        └──────────────────────────────────────────────────────────────┘  sesji, wyższy poziom
        + WATCHDOG (osobny proces: restart, safe-mode, rollback, Job Objects)  integralności)
```

### 2.1 Jądro „Alfa Core"

| Składnik | Crate | Odpowiedzialność |
|---|---|---|
| Magistrala zdarzeń | `core-bus` | publikacja/subskrypcja typowanych zdarzeń, grupowanie zdarzeń do UI (batch co klatkę) |
| Rejestr modułów | `core-registry` | wczytywanie manifestów, rozwiązywanie kontraktów, cykl życia (`lazy`/`on-demand`/`always`), health-check, budżety |
| Konfiguracja | `core-config` | pliki TOML z JSON Schema, przeładowanie na żywo, warstwa wspólna + nakładka per maszyna |
| Log-writer | `core-log` | zapis strumieni innych niż Audyt (§7 tego dokumentu) |
| Klient Brokera | część `core-*` | kanał do usługi Brokera: żądanie tokenów, przekazanie próśb o zatwierdzenie, odbiór kill-switcha |
| Aktualizator | `updater` | launcher, wersje side-by-side, rollback (ADR 7) |

Jądro nie zawiera logiki domenowej (rozmowa, głos, narzędzia). Nie zawiera też polityk bezpieczeństwa — te trzyma Broker.

## 3. Moduł = jednostka wszystkiego

### 3.1 Trójka crate'ów

| Crate | Zawartość | Kto od niego zależy |
|---|---|---|
| `<m>-contract` | trait(y), typy, zdarzenia, JSON Schema zdarzeń | wszyscy inni (jedyna dozwolona zależność między modułami) |
| `<m>-impl` | implementacja | tylko rejestr modułów / kompozycja builda |
| `<m>-fake` | atrapa deterministyczna do testów bez sprzętu i bez sieci | testy innych modułów, symulatory (§4.5 planu) |

Test kontraktowy uruchamia ten sam zestaw przypadków przeciwko `-impl` i `-fake`; rozjazd zachowań jest błędem.

### 3.2 Manifest `module.toml`

Pola wymagane przez plan (§3.2): `id`, `version`, `kind`, kontrakty dostarczane/wymagane, żądane zdolności, budżet zasobów, cykl życia, izolacja, schemat konfiguracji, wkład do UI, health-check. Przykład (nazwy pól są propozycją; schemat manifestu powstaje w F0, pkt 2 §4.5a):

```toml
id = "voice-stt"
version = "0.1.0"
kind = "voice-engine"          # service | tool | provider | voice-engine | agent-pack | ui-panel

[contracts]
provides = ["voice-stt-contract@1"]
requires = ["core-bus-contract@1", "voice-audio-contract@1", "device-profile-contract@1"]

[capabilities]
requested = ["fs.read(%LOCALAPPDATA%/Alfa/models/**)", "gpu.compute"]

[budget]
ram_mb = 1536                  # z modelem large-v3-turbo Q5_0
cpu_pct_idle = 0
vram_mb = 2560

[lifecycle]
mode = "on-demand"             # lazy | on-demand | always
unload_after_idle_s = 300

[isolation]
mode = "process"               # inproc | process | wasm
transport = "stdio-jsonrpc"

[config]
schema = "schemas/voice-stt.config.schema.json"

[ui]
settings_page = "ui/settings/voice-stt"
panel = ""

[health]
check = "ping"
interval_s = 15
```

Budżet z manifestu jest monitorowany w runtime; przekroczenie → ostrzeżenie lub zwolnienie modułu (§3.4 planu).

### 3.3 Izolacja wg potrzeby

| Tryb | Kiedy | Mechanizm | Przykłady |
|---|---|---|---|
| `inproc` | ścieżki RT i krytyczne dla opóźnień | natywny crate za feature flagą; wątek RT; **bez Wasm/IPC w callbacku audio** | `voice-audio`, `voice-dsp`, `voice-vad`, `voice-turn`, `voice-dialog`, `scheduler`, `memory`, `router` |
| `process` | ciężkie lub awaryjne | osobny proces, JSON-RPC po stdio lub named pipe; AppContainer + Job Object dla niezaufanych | `voice-stt`, `voice-tts`, `providers-local` (llama.cpp), `tools-browser`, helper UIA |
| `wasm` | kod generowany przez AI | wasmtime + własny WIT, cel `wasip2`, bez importów WASI, limity epoch/fuel/pamięci | wtyczki, umiejętności (ADR 12) |
| zewnętrzne | procesy spoza Alfy | serwery MCP, oficjalne CLI jako „opaque worker" (§8.5 planu) | Claude Code, Codex |

Odrzucone: DLL przez `abi_stable` (ADR 2). Włączanie/wyłączanie w runtime: hot dla `process`/`wasm`, aktywacja flagą dla `inproc`. Feature flags wybierają skład builda; build minimalny = sam czat.

Wątki: UIA na dedykowanym wątku MTA; windows-rs/COM zamknięte w jednym crate `platform-windows` za traitem `SystemPort` (jedna wersja windows-rs w całym workspace).

## 4. Katalog modułów (§3.3 planu)

| Grupa | Moduł | Rola (jedno zdanie) |
|---|---|---|
| Jądro | `core-bus` | Magistrala typowanych zdarzeń, jedyny kanał między modułami i do UI. |
| Jądro | `core-registry` | Rejestr manifestów, cykl życia i health-check modułów. |
| Jądro | `core-config` | Konfiguracja TOML z JSON Schema, warstwowa (wspólna + per maszyna), przeładowanie na żywo. |
| Jądro | `core-log` | Writer strumieni logów innych niż Audyt (NDJSON + indeks SQLite). |
| Jądro | `updater` | Launcher, wersje side-by-side, aktualizacje podpisane minisign, rollback. |
| Bezpieczeństwo | `safety-broker` | Usługa na osobnym koncie: tokeny zdolności, zatwierdzenia, writer Audytu, polityki Jądra, kill-switch. |
| Bezpieczeństwo | `broker-ui` | Małe natywne okno zatwierdzeń w sesji użytkownika, na wyższym poziomie integralności. |
| Bezpieczeństwo | `watchdog` | Heartbeat, restart modułów, safe-mode po N awariach, rollback, Job Objects. |
| Bezpieczeństwo | `risk-classifier` | Ocena akcji: odwracalność, zakres, wpływ zewnętrzny, destrukcyjność, pewność STT. |
| Bezpieczeństwo | `undo-journal` | Dziennik cofania dla `fs.*`, snapshoty zakresu dla shella. |
| Bezpieczeństwo | `compliance` | Rejestr tras abonamentowych ze statusem, datą weryfikacji i wyłącznikiem. |
| Modele | `accounts-hub` | Konta i klucze (Credential Manager), katalog dostawców, kreator dodawania bez restartu. |
| Modele | `providers-local` | Wbudowany llama.cpp (Vulkan/CUDA/CPU) i adapter do zewnętrznych endpointów lokalnych. |
| Modele | `providers-api` | Adaptery API: Anthropic, OpenAI, adapter generyczny „endpoint zgodny z OpenAI/Anthropic". |
| Modele | `agent-backends` | Mosty CLI (Claude Code, Codex, później Grok Build, Kimi Code, `agy`) jako `AgentBackend`. |
| Modele | `router` | Kierowanie zadań według klasy, tagów prywatności/jurysdykcji, budżetu i opóźnienia; fallback, circuit breaker. |
| Modele | `cost-meter` | Koszty, limity (PLN, kurs NBP), wskaźniki zużycia okien planów. |
| Modele | `model-residency` | Zarządca RAM/VRAM: które modele są rezydentne, wymiana, wykrywanie pełnego ekranu. |
| Agentki | `agent-runtime` | Pętla plan → działanie → obserwacja → weryfikacja, checkpointy, budżety, steering. |
| Agentki | `personas` | Persony Alfa/Beta/Gama/Delta (imię, głos, charakter) i obsada ról. |
| Agentki | `scheduler` | Deterministyczny szeregowacz z Lock Managerem zasobów wyłącznych (`scheduler-lite` w F2). |
| Agentki | `marshal` | Marszałek: tłumaczy polecenia naturalne na deklaratywne reguły, które tylko zawężają. |
| Agentki | `agent-builder` | Kreator agentek: opis → manifest → test w piaskownicy → biblioteka. |
| Agentki | `triggers` | Harmonogram i wyzwalacze (plik, czas, skrót, fokus okna, schowek, e-mail). |
| Głos | `voice-*` | Voice Suite (~16 modułów, §6.2 planu): audio, dsp, vad, turn, stt, speaker, wake, dialog, tts, persona, cmd, dictation, readaloud, lab, s2s, transcribe. |
| System | `platform-windows` | Jedyny crate z windows-rs/COM, za traitem `SystemPort`. |
| System | `tools-fs` | Pliki i foldery z dziennikiem cofania (usuwanie domyślnie do Kosza). |
| System | `tools-shell` | PowerShell/cmd/ConPTY/WSL2/SSH ze snapshotem zakresu. |
| System | `tools-clipboard` | Schowek z historią i deny-listą. |
| System | `tools-window` | Okna, pulpity wirtualne, fokus. |
| System | `tools-uia` | UI Automation: drzewo, wzorce, ocena jakości drzewa. |
| System | `tools-vision` | Zrzuty per monitor (DPI), OCR. |
| System | `tools-input` | SendInput (mysz, klawiatura, dotyk, pióro) z weryfikacją po akcji. |
| System | `tools-browser` | Przeglądarka przez CDP/Playwright z własnym profilem i deny-listą. |
| System | `tools-office` | Office COM (Word/Excel w P1). |
| System | `tools-system` | Procesy, usługi, rejestr, ustawienia, zasilanie, sieć. |
| System | `tools-net` | HTTP/WebSocket, pobieranie z wznawianiem, egress-allowlista. |
| System | `tools-media` | Kamera, mikrofon, odtwarzanie, ffmpeg. |
| System | `mcp` | Klient MCP i serwer MCP Alfy (stdio-proxy / named pipe z ACL). |
| System | `shell-integration` | Zasobnik, toasty, handler protokołu, „Wyślij do", pasek zadań. |
| System | `notify` | Powiadomienia natywne i w aplikacji, tryb „nie przeszkadzać". |
| Dane | `memory` | Cztery warstwy pamięci z zakresami (sesja/projekt/globalna/agentka) i proweniencją. |
| Dane | `sessions` | Sesje: polityka modeli, agentki, zakres pamięci, katalog roboczy, tag prywatności. |
| Dane | `search` | FTS5 + wektory (sqlite-vec) w szyfrowanej bazie per sesja. |
| Dane | `artifacts` | Pliki oddawane przez agentki: karty, wersje, podgląd. |
| Dane | `transfer` | Import/eksport paczek `.alfa`, kopie zapasowe jako zaplanowany eksport. |
| Sprzęt | `device-profile` | Autodetekcja sprzętu, profil potoku A–D per maszyna, tryb baterii. |
| Samo-ulepszanie | `diagnostician` | Klasteryzacja błędów → hipoteza → odtworzenie → Propozycja zmiany. |
| Samo-ulepszanie | `improver` | Ulepszacz (idle/noc, modele lokalne) z bramką ewaluacyjną w Jądrze. |
| Samo-ulepszanie | `evals` | Zadania wzorcowe, zamrożone zestawy akceptacyjne, ukryty holdout. |
| Samo-ulepszanie | `plugin-runtime` | Uruchamianie wtyczek Wasm (wasmtime + WIT) z limitami. |
| UI | `ui-shell` | Okno główne, panele, tryb skupienia, wiele okien na wspólnym środowisku WebView2. |
| UI | `ui-kit` | Tokeny designu, komponenty, Storybook (jedno źródło prawdy §14.3). |
| UI | `ui-quick` | Szybkie pytanie, pigułka głosowa, menu zasobnika. |
| UI | `ui-terminal` | Wbudowany terminal ConPTY do logowania w CLI (użytkownik loguje się sam). |

## 5. Budżety lekkości (§3.4 planu; wstępne, ostateczne po pomiarze w F0)

| Wskaźnik | Cel wstępny |
|---|---|
| Instalacja bazowa (jądro + UI, bez modeli) | ≤ 40 MB; modele/moduły pobierane na żądanie |
| Zimny start do zasobnika / do okna | ≤ 1 s / ≤ 1,5 s |
| Otwarcie okna z zasobnika | ciepłe ≤ 150 ms, zimne ≤ 1 s |
| Idle bez głosu: jądro | ≤ 40 MB Private Working Set, CPU ≈ 0 |
| Idle z oknem | suma drzewa procesów (WebView2 dominuje) — cel wyznaczony w F0 (spike f) |
| Moduły głosowe | ładowane przy pierwszym użyciu, zwalniane po bezczynności |
| RAM z załadowanym głosem | ~2,5–4 GB łącznie, zapas ≥ 7 GB z 16 GB — do zmierzenia w F0 |
| VRAM (8 GB baseline) | pulpit 0,5–1 GB + STT 1–2,5 GB; LLM 3–4B Q4 obok STT; 8B tylko przy STT na CPU |
| Budżet per moduł | deklarowany w manifeście, monitorowany; przekroczenie → ostrzeżenie/zwolnienie |
| Binaria | LTO, strip; opt-level pod RT tylko tam, gdzie trzeba |

Wszystkie budżety i kryteria DoD mierzy się na **baseline** (emulowanym); mocniejsze maszyny mogą je tylko poprawiać.

## 6. Klasy sprzętu (§3.5 planu)

| Klasa | Sprzęt | Ścieżka GPU | Co daje |
|---|---|---|---|
| Baseline (minimum) | Ryzen 5 5600 · RX 7600 8 GB (RDNA3) · 16 GB · Win11 | Vulkan, bez CUDA | profile głosu A/B/C, lokalny LLM 3–4B; wszystkie DoD i budżety; brak fizycznie → emulacja limitów (6 rdzeni, 16 GB, VRAM 8 GB, korekta GPU ×2,2) |
| Desktop (Standard-AMD) | Ryzen 7 5700X3D · RX 9070 XT 16 GB (RDNA4) · 32 GB | Vulkan | whisper large-v3 pełny, LLM 8–14B Q4; główna maszyna dev i runner AMD; ROCm do sprawdzenia w F0 |
| Laptop (Laptop-CUDA) | i7-13700H · RTX 4050 6 GB · 16 GB | CUDA | whisper CUDA turbo Q5, LLM 3–4B; ciasny VRAM → rezydencja na przemian; tryb baterii, termika |
| Przyszły „Mocny" | NVIDIA ≥ 12–16 GB | CUDA | pełny lokalny stos (profil D) |

Mechanizmy: `device-profile` (autodetekcja, profil per maszyna, Kreator sprzętu), konfiguracja warstwowa z nakładką `config/machine/<id>.toml`, `model-residency` (limity VRAM/RAM, wymiana modeli, wykrywanie pełnego ekranu). Luka: RDNA3 nie jest pokryty fizycznie — fallback CPU i przypięte sterowniki.

## 7. Układ repo (§3.6 planu)

```
apps/desktop/         Tauri 2 shell + ui/ (Svelte 5)
crates/<moduł>-contract|-impl|-fake   (wg katalogu §4)
crates/core-*         jądro
sidecars/             ciężkie moduły jako osobne procesy (whisper.cpp, TTS…)
plugins/              źródła wtyczek Wasm + WIT
packages/ui-kit/      tokeny designu, komponenty, Storybook
packages/schemas/     JSON Schema zdarzeń i konfiguracji
providers-catalog/    deklaratywny katalog dostawców (§5.6 planu)
evals/                zadania wzorcowe, zamrożone zestawy akceptacyjne (hash), korpus własny (poza gitem), holdout
docs/                 PLAN.md, ARCHITECTURE.md, AI_WORKFLOW.md, ACCEPTANCE.md, VOICE.md, PERSONAS.md, UI.md,
                      THREAT_MODEL.md, ADR/, modules/<m>/SPEC.md, formats/, vendor/, compliance/
AGENTS.md  CLAUDE.md  README.md
```

## 8. Kontrakty modeli i backendów (§5.1 planu)

| Kontrakt | Jednostka | Metody | Kto implementuje |
|---|---|---|---|
| `ModelProvider` | tokeny | `capabilities()`, `stream(req)` z anulowaniem, `health()`, `cost()` | klasa A (lokalne: llama.cpp, Ollama/LM Studio przez adapter), klasa B (API: Anthropic, OpenAI, adapter generyczny), klasa D (własne) |
| `AgentBackend` | zadania | `submit_task`, `events`, `approve`, `steer`, `cancel`, `resume` | klasa C (mosty CLI jako „opaque worker"), klasa D (własne) |

Rodzaje `ModelProvider`: chat, STT, TTS, embeddings, wizja/OCR, S2S. Router kieruje **zadania** (nie żądania tokenowe) według klasy zadania × ograniczeń (tag prywatności i jurysdykcji, budżet, opóźnienie, możliwości), z fallbackiem i circuit breakerem. Mosty mają zimny start rzędu sekund → nigdy na ścieżce głosu. Mosty nie widzą tokenów CLI (deny-lista ścieżek poświadczeń, ETW w testach F4). Szczegóły: ADR 5, ADR 14, ADR 15.

## 9. Broker, Broker-UI, Watchdog (§8 planu)

| Proces | Gdzie działa | Rola | Czego nie robi |
|---|---|---|---|
| `safety-broker` (usługa) | tło, osobne konto Windows, sesja 0 | wydaje tokeny zdolności (`fs.read/write(zakres)`, `shell.exec`, `gui.control(app)`, `net.egress(host)`, `secrets.read`, `system.admin`; TTL; potomek ≤ rodzic), prowadzi zatwierdzenia, jedyny writer Audytu, trzyma polityki Jądra, obsługuje kill-switch, uruchamia operacje elewowane przez UAC | nie ma UI (sesja 0 nie pokazuje okien); nie jest wąskim gardłem logów |
| `broker-ui` | sesja użytkownika, wyższy poziom integralności niż agentki | okno zatwierdzeń: kliknięcie/klawisz z wejścia niewstrzykniętego (UIPI odrzuca SendInput z procesów agentek); opcjonalnie Windows Hello | nie renderuje treści LLM w WebView |
| `watchdog` | osobny proces | heartbeat, restart modułów, safe-mode po N awariach, rollback, zabijanie drzew przez Job Objects, obsługa kill-switcha < 200 ms | nie zależy od UI |

Jądro bezpieczeństwa (agent nie zmienia): silnik uprawnień, audyt, watchdog, aktualizator, kill-switch, tagi prywatności, budżety, egress-allowlista, klasyfikator ryzyka, polityka taint, bramka ewaluacyjna, deny-listy z §1.3 planu. Zakaz `gui.control` wobec procesów Alfy, Brokera i helpera `uiAccess`. Narzędzia wykonujące dla ≤ L3: restricted token / low-integrity / AppContainer. Szczegóły: ADR 3, ADR 15.

Kolejność w czasie: w F1 Audyt tymczasowo pisze `core-log` z oznaczeniem `pre-broker`; w F3 Broker przejmuje strumień z nowym łańcuchem hashy.

## 10. Przepływ zdarzeń i logów (§13 planu)

Zdarzenie: `{id, ts, sesja, agentka, przebieg, span, rodzaj, poziom, payload_ref, koszt, hash_prev}`. Zapis: append-only NDJSON + indeks SQLite; payloady szyfrowane kluczem per sesja (crypto-shredding — usunięcie klucza sesji kasuje treść bez łamania łańcucha). Schematy wersjonowane w `packages/schemas/` z upcasterami i testami migracji.

```
moduł ──publish──▶ core-bus ──┬──▶ subskrybenci (router, scheduler, agent-runtime, UI przez batch co klatkę)
                              ├──▶ core-log ──▶ strumienie: Wywołania modeli · Narzędzia i GUI · Głos · Diagnostyka
                              └──▶ klient Brokera ──▶ safety-broker ──▶ strumień Audyt (łańcuch hashy po digestach,
                                                                          pliki append-only przez ACL, kotwica głowy poza zasięgiem agentów)
```

| Strumień | Writer | Zawartość | Retencja |
|---|---|---|---|
| Audyt | Broker | akcje wrażliwe, decyzje uprawnień, zmiany konfiguracji i samo-zmiany; zdarzenia mostów „niezależnie niezweryfikowane" | limity dysku (do ustalenia w F0) |
| Wywołania modeli | `core-log` | dostawca, model, tokeny, koszt, opóźnienie, prompt/odpowiedź (redakcja przełączalna) | limity dysku |
| Narzędzia i GUI | `core-log` | krok + zrzut + migawka UIA, deny-lista | domyślnie 7 dni, szyfrowane |
| Głos | `core-log` | opóźnienia etapów p50/p95, fałszywe przerwania, dokładność prefiksu, echo, WER | limity dysku |
| Diagnostyka | `core-log` | błędy, moduły, watchdog | limity dysku |

Z tych samych zdarzeń powstają: drzewo postępów (Plan → kroki → status), kapsuła aktywności, Oś czasu/Replay, narracja Mówczyni (generowana ze zdarzeń, nie z pamięci modelu), sygnały dla Ulepszacza, paczka diagnostyczna. Poziomy: TRACE … AUDIT.

## 11. Zasady zależności

| Reguła | Egzekucja |
|---|---|
| Moduł zależy od innego modułu **tylko przez `<m>-contract`**; nigdy od `-impl` | sprawdzanie grafu zależności w CI (`cargo-deny` + własny skrypt), `dependency-cruiser` dla TS |
| Jądro (`core-*`) nie zależy od żadnego modułu domenowego | graf zależności w CI |
| windows-rs/COM tylko w `platform-windows`; inne moduły przez `SystemPort` | `cargo-deny` (ban crate poza allowlistą) |
| Kontrakty Rust → typy TS generowane (`tauri-specta`/`ts-rs`), JSON Schema dla zdarzeń, WIT dla Wasm | `git diff --exit-code` na plikach generowanych (ADR 13) |
| Plik ≤ ~300–400 linii, crate ≤ ~5–8 tys. linii | clippy `too_many_lines`, skrypt CI (heurystyka) |
| Zero `TODO/FIXME/unimplemented!/unwrap()` w ścieżkach produkcyjnych | lint |
| Kolejność pracy nad modułem | SPEC → kontrakt → fake → testy → implementacja → przegląd drugiego modelu → CI → merge |

## 12. Otwarte punkty (do ustalenia w F0)

| Punkt | Spike / miejsce |
|---|---|
| Dokładny schemat manifestu `module.toml` i nazwy pól | F0 pkt 2 (§4.5a planu) |
| Wartości ostateczne budżetów §5 i sumy RAM drzewa WebView2 (1 i 3 okna) | spike (f) |
| Zgodność SQLCipher + sqlite-vec + FTS5 w jednej bazie | spike (i), ADR 8 |
| Uruchomienie Broker-UI na wyższym poziomie integralności z usługi + test odrzucenia SendInput | spike (k), ADR 3 |
| Historia append-only vs bloki myślenia Anthropic | spike (g) odroczony do klucza Anthropic, ADR 6 (Tymczasowy) |
| Progi pokrycia testami (wstępnie ≥ 85% linii, ≥ 70% gałęzi) | F0 |
| Snap Layouts, Mica, pisownia PL w WebView2, toasty z AUMID przez launcher | spike (j) |
| Limity dysku strumieni logów | F0 |
