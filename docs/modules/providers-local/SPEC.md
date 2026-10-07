# providers-local — SPEC (v1, F1)

## Cel
Lokalny `ModelProvider`: wbudowany llama.cpp (Vulkan/CUDA/CPU) jako osobny proces; bez kluczy API jedyny „mózg" (MVP), z kluczami — komendy, fallback, offline. Ollama i LM Studio jako zewnętrzne endpointy przez adapter generyczny (w `providers-api`), nie tutaj. Pobieranie modelu 1,5–4,5B (oficjalny GGUF autorów; Bielik v3 — Q8_0, fala 6) w onboardingu (PLAN §1.2, §5.2, §16.2 F1).

## Fala i priorytet
F1 (chat + embeddings lokalne, pobieranie modelu, Vulkan/CUDA/CPU). P0 — wymagany w MVP.

## Kontrakt (v1)
Kontrakt = `providers-contract::ModelProvider` (ADR 0014); implementacja `crates/providers-local-impl`
(`LocalProvider`, `Sidecar`, `Downloader`, `LocalModule`). Osobny `providers-local-contract` nie jest potrzebny w F1
(UI/onboarding używa modułu z kompozycji); do rozważenia, gdy inne moduły będą sterować pobieraniem.
- **Protokół sidecara (rozstrzygnięte):** `llama-server` (OpenAI-compatible HTTP) na `127.0.0.1`, losowy port i losowy
  `--api-key` (192 bity, tylko w pamięci, redagowany w logach) per uruchomienie; nigdy `0.0.0.0`. Strumień przez wspólny
  silnik `lib-openai-compat` (ten sam co `providers-api`). Job Object / brak tokenu egress — przez `platform-windows` (F2).
- **Cykl życia:** start na żądanie (jeden naraz), zdrowie `GET /health`, `Started` od razu po przyjęciu żądania (zimny
  start modelu nie jest milczeniem dla Routera), restart po awarii z limitem (`max_restarts` w `restart_window`),
  fallback GPU → CPU (`-ngl 0`), zwolnienie po bezczynności, `kill_on_drop`.
- **Argumenty z profilu urządzenia:** backend (Vulkan/CUDA/CPU = osobne pliki `llama-server`; bateria lub słaby sprzęt
  → CPU), `-ngl` z dzierżawy `model-residency` (GPU → wszystkie warstwy, CPU → 0) albo proporcjonalnie do budżetu VRAM,
  `-c`, `--threads` (rdzenie fizyczne), `--alias`, `-np 1`, `--jinja` (narzędzia).
- **Modele:** `models.toml` (GGUF, URL HF, rozmiar, SHA-256, kwant, warstwy, kontekst, szacunki VRAM/RAM, licencja);
  walidacja: `.gguf`, bez kwantów IQ, https. Domyślny: Bielik 4.5B v3.0 Instruct Q8_0 (fala 6; wcześniej Q4_K_M — brak w oficjalnym repozytorium).
- **Pobieranie:** wznawianie HTTP Range z pliku `.part` (≤ 3 automatyczne wznowienia), SHA-256; zły hash → błąd
  i usunięcie `.part`. Hash nieznany w manifeście (`sha256 = ""`) → zapis przy pierwszym pobraniu (`<plik>.sha256`,
  zaufanie przy pierwszym użyciu) + ostrzeżenie w logach; model bez zapisanego hasha = niezainstalowany.
- Bez pobranego modelu dostawca jest `Unconfigured` (Router go pomija).

Zdarzenia: `local.model.download.progress/finished/failed`, `local.model.loaded/unloaded`, `local.backend.fallback`
(GPU → CPU), `local.sidecar.crashed` — bez treści rozmowy i bez klucza.

## Zależności
`providers-contract` (ModelProvider), `lib-openai-compat` (silnik HTTP/SSE), `core-bus/registry-contract`, `model-residency-contract` (dzierżawa VRAM/RAM; bez zarządcy — budżet z `device-profile`), `device-profile-contract` (backend, VRAM, rdzenie), `platform-windows-contract` (Job Object — F2). Zewnętrzne: llama.cpp (wersja przypięta, `docs/vendor/llama-cpp.md`).

## Niezmienniki
- Model tylko GGUF z hashem (łańcuch dostaw, PLAN §8.7); nieznany hash = brak ładowania.
- Bez kwantów IQ (crashe RDNA3/Vulkan); 8B tylko gdy STT idzie na CPU (rezydencja).
- Crash sidecara nie zabija aplikacji; automatyczny fallback backendu (Vulkan → CPU) z ograniczoną liczbą prób i zdarzeniem.
- Brak Pythona; brak dostępu sidecara do sieci (Job Object / brak tokenu egress) — dane nie wychodzą.
- Tabela cen: koszt = 0 PLN, ale zużycie tokenów raportowane do `cost-meter` (statystyki).

## Zdolności / uprawnienia
`fs.read/write(%LOCALAPPDATA%\Alfa\models\**)`; `net.egress(host)` tylko dla pobierania modeli z hostów z katalogu (na czas pobierania).

## Izolacja
`process` (JSON-RPC po stdio/named pipe; Job Object), `on-demand` z `idle_unload` (domyślnie 10 min, z nakładki maszyny).

## Budżet zasobów
Baseline: model 3–4,5B Q4 ≈ 2,5–3,5 GB VRAM (Vulkan) obok STT 1–2,5 GB; RAM sidecara ≤ 1 GB poza wagami; tok/s i TTFT mierzone w spike (h) — cele do ustalenia po F0 (korekta GPU ×2,2 dla baseline).

## Konfiguracja (klucze TOML)
Wspólne: `[providers.local] default_model = "bielik-4.5b-v3.0-instruct-q8_0"`, `ctx = 8192`, `min_ctx = 4096`, `stt_reserve_mb = 1500`. Per maszyna: `[providers.local.machine] backend = "auto" | "vulkan" | "cuda" | "cpu"`, `gpu_layers = "auto"`, `idle_unload = "10m"`, `max_vram_mb`.
W aplikacji (Ustawienia → Modele i silniki, nakładka maszyny, od następnego uruchomienia; `app_modules::route::engine_settings`):
`engines.llm.backend` (`auto` | `cuda` | `vulkan` | `cpu` → `BackendChoice`), `engines.llm.context` (`auto` | `4096` | `2048` → `ctx`),
`engines.llm.threads` (0 = rdzenie fizyczne), `engines.llm.idle_unload_min` (1–120 min); wartość spoza zakresu = domyślna.

## Wkład do UI
Onboarding: pobieranie modelu (postęp, wznawianie); Ustawienia → Modele i dostawcy → Lokalne (modele, backend, benchmark); stany GPU OOM / przerwane pobieranie (§14.4).

## Testy akceptacyjne
- `ACC-F1-providers-local-01`: lokalny llama.cpp na żywo — rozmowa na modelu 3–4,5B na runnerze desktop (Vulkan) i laptop (CUDA), 0 crashy w 1 h.
- `ACC-F1-providers-local-02`: pobieranie z przerwaniem sieci → wznowienie, hash zgodny.
- `ACC-F1-providers-local-03`: wymuszony błąd Vulkan → fallback CPU ≤ 5 s, rozmowa trwa.
- `ACC-F3-providers-local-04`: eval narzędzi fs/shell na lokalnym modelu ≥ próg ustalony w F0.

## Fake
`providers-local-fake`: udaje sidecar (skryptowane odpowiedzi, tok/s z wirtualnym zegarem), pobieranie z lokalnych fixture'ów, symulacja OOM/crash.

## Otwarte pytania
- Próg jakości tool-use po polsku na Bielik 4.5B — pomiar F3.
- SHA-256 i dokładny rozmiar GGUF Bielika — wpisać do `models.toml` po pierwszym pobraniu (pełna weryfikacja łańcucha dostaw).
- Nasłuch TCP na `127.0.0.1` z kluczem zamiast named pipe (llama-server nie obsługuje pipe) — ryzyko ograniczone (localhost,
  losowy port i klucz per uruchomienie); Job Object bez sieci dla sidecara — F2 przez `platform-windows`.

## Zmiany — menedżer modeli (2026-10-04)
- Pobieranie GGUF i sidecara `llama-server` w aplikacji przejął menedżer `app-models` (SPEC `models`): ten sam
  manifest `models.toml`, katalog `%LOCALAPPDATA%\Alfa\models`, plik + rekord `<plik>.sha256` (rozpoznawany przez
  `providers_local_impl::installed`), ale plik bez przypiętego SHA-256 instaluje się **dopiero po jawnej zgodzie
  w UI** (karta z policzonym hashem i licencją) — także w onboardingu. `llama-server` (Vulkan/CPU) z archiwum ZIP
  wydania llama.cpp do `sidecars\llama-<backend>\` (bezpieczne rozpakowanie; wydanie i hash do przypięcia).
- `Downloader` i komendy `models_local_*` zostają dla zgodności (zapisują hash przy pierwszym pobraniu bez karty
  zgody) — UI już ich nie używa; wyłączenie — decyzja człowieka.


## Zmiany — fala 6 (2026-10-06)
- `LocalConfig::server_candidates`: kandydaci na plik `llama-server` per backend, sprawdzani przy **każdym** starcie
  sidecara (`LocalConfig::server`; pierwszy istniejący wygrywa, kandydat zastępczy — ostrzeżenie w dzienniku). Kompozycja
  (`app_modules::route::local::candidates`): własna kompilacja → `sidecars/llama/` → zastępcze. Serwer pobrany w
  Ustawieniach po starcie aplikacji działa od następnej wiadomości (test `tests/install_after_start.rs`).
- Próba profilu laptopa właściciela na atrapach (`tests/laptop.rs`): Bielik 4.5B z manifestu na kompilacji CUDA z
  `-ngl` = wszystkie warstwy i `--threads 14`, STT obok w budżecie 6 GB VRAM, ciężki TTS wyklucza STT, na baterii —
  kompilacja CPU bez warstw GPU, bez kompilacji CUDA — Vulkan na GPU.

## Zmiany — fala 6, laptop 6 GB: modele Q8_0, KV cache i STT obok LLM (2026-10-06)
- **Modele tylko z oficjalnych repozytoriów** (`speakleash/*-GGUF`, apache-2.0, bez bramki): wpis Q4_K_M wskazywał
  nieistniejący plik (HTTP 404 na runnerze) — Q4_K_M Bielika v3 jest wyłącznie u osób trzecich (decyzja zaufania dla
  człowieka). Manifest: **Bielik 4.5B v3.0 Instruct Q8_0** (`bielik-4.5b-v3.0-instruct-q8_0`, 4826 MiB, domyślny,
  narzędzia) i **Bielik 1.5B v3.0 Instruct Q8_0** (`bielik-1.5b-v3.0-instruct-q8_0`, 1620 MiB, `tools = false` —
  model 1,5B zbyt zawodnie wywołuje narzędzia; Router nie wybierze go do zadań agentek, rozmowa i głos działają).
- **KV cache w rozliczeniu VRAM/RAM.** `vram_mb`/`ram_mb` = wagi + stały narzut, **bez** KV; nowe wymagane pole
  `kv_mb_per_1k_ctx` (KV f16 na 1024 tokeny, > 0). `vram_mb` = plik GGUF + 550 MB (`GPU_OVERHEAD_MB`: kontekst
  CUDA/Vulkan ≈ 300 + bufory obliczeń ≈ 250), `ram_mb` = plik + ≈ 750. KV: warstwy × głowice KV × wymiar głowy × 2
  (K, V) × 2 B. Architektura z raportu technicznego Bielik v3 (bez sieci w sesji nie dało się odczytać GGUF —
  **do potwierdzenia** z metadanych `qwen2.*` albo z wpisu `llama_kv_cache … MiB` w logu `llama-server`):
  4.5B = Qwen2.5-3B (GQA 16/2, głowa 128) pogłębiony do 60 warstw → 60 MiB/1k; 1.5B = Qwen2.5-1.5B (GQA 12/2) do
  prawdopodobnie 32 warstw → 32 MiB/1k.

  | Model | `vram_mb` | `ram_mb` | KV/1k | `-c 8192`: VRAM całości | RAM na CPU |
  | --- | --- | --- | --- | --- | --- |
  | 4.5B Q8_0 | 5400 | 5600 | 60 | 5880 | 6080 |
  | 1.5B Q8_0 | 2200 | 2400 | 32 | 2456 | 2656 |
- **Częściowe odciążenie z dzierżawą.** `-ngl` = `layers_for(budżet VRAM − rezerwa STT, ctx)`: całość, gdy się mieści,
  inaczej (budżet − 550) / (VRAM całości − 550) × warstwy. Dzierżawa `model-residency` zgłasza VRAM dla tych warstw
  (`ModelEntry::vram_for`) i RAM procesu z wagami/KV warstw na CPU (`ram_for`); dzierżawa na CPU → `-ngl 0`. Rezerwa
  STT (`stt_reserve_mb` = 1500, tyle co dzierżawa `voice-stt`) — gdy profil urządzenia przewiduje STT na GPU.
- **Kontekst** (`LocalConfig::ctx_for`, `min_ctx` = 4096): na GPU zmniejszany (połowienie do `min_ctx`) **tylko
  wtedy, gdy pozwala to zmieścić model w całości** obok rezerwy STT; gdy i tak będzie częściowe odciążenie — pełny
  `ctx` (KV warstwy to ułamek jej wag: 4096 zamiast 8192 dałoby 4.5B Q8_0 na laptopie 1–2 warstwy). Na CPU — pełny.
  Silnik i `max_tokens` (połowa) z kontekstu uruchomienia.
- **Laptop RTX 4050 6 GB (budżet zarządcy 5153 MB), zasilacz:** 4.5B Q8_0 nie mieści się w całości nawet przy 4096
  (5640 + 1500 > 5153) → `-c 8192`, **34 z 60 warstw na karcie: 3570 MB VRAM**, 26 warstw (wagi + KV) w RAM —
  dzierżawa 3146 MB RAM; whisper CUDA obok: 3570 + 1500 = 5070 MB ≤ 5153. Rozmowa tekstowa i głosowa — ten sam
  układ (rezerwa na STT od początku, bez przeładowań). Bez rezerwy byłoby 51 warstw (5080 MB) i STT na CPU.
  1.5B Q8_0: w całości (2456 MB) + STT (1500 MB). Bateria: LLM na CPU (6080 MB RAM), STT CUDA. Baseline 8 GB
  (7408 MB): 4.5B Q8_0 w całości obok STT (5880 + 1500 = 7380). *Kompromis:* rezerwa na STT spowalnia rozmowę
  tekstową 4.5B na laptopie (34 zamiast 51 warstw na GPU), ale whisper turbo na CPU byłby za wolny do rozmowy.
  *Do decyzji człowieka:* rekomendacja 1.5B w onboardingu dla kart ≤ 6 GB (szybciej, całość na GPU, ale słabsza
  jakość po polsku i bez narzędzi) — logika onboardingu bez zmian.
- **Dzierżawa odświeżana przy użyciu:** w trakcie żądania oznaczona `in_use` (nie zwalnia jej zadanie tła zarządcy
  ani równy priorytet), koniec ostatniego żądania odświeża licznik bezczynności. Wcześniej dzierżawa LLM nie była
  odświeżana i zarządca odbierał ją co `idle_unload` (10 min) **od załadowania** — przeładowanie modelu w trakcie
  rozmowy.
- Testy: `tests/config.rs` (KV, narzut, `layers_for`/`vram_for`/`ram_for`, `ctx_for`, układ na laptopie),
  `tests/laptop.rs` (34/60 warstw, dzierżawa 3570 MB, STT obok, 1.5B w całości, bateria, Vulkan),
  `tests/laptop_voice.rs` (kolejne tury głosowe przez 36 min czasu zarządcy: jedno uruchomienie `llama-server`,
  LLM nie wypierany, STT na CPU, gdy się nie mieści), `tests/download.rs` (oficjalne repozytoria, Q8_0).
- **Okno rozmowy w kontekście uruchomienia** (`window::fit`, kodek `LlamaCodec`): czat wysyła całą gałąź historii
  (append-only), a `llama-server` odrzuca prompt dłuższy niż `-c`. Do serwera idzie prompt systemowy, narzędzia i najnowsze
  tury mieszczące się w `ctx − rezerwa na odpowiedź (min(max_tokens, ctx/4)) − 256`; szacunek tokenów zawyżony (bajty
  UTF-8 / 3 + 8 na wiadomość). Okno zaczyna się od tury użytkownika bez wyników narzędzi — para „wywołanie → wynik” nie
  jest rozrywana; gdy w budżecie nie ma takiej tury (długa seria narzędzi), okno sięga do najbliższej wcześniejszej.
  Ostatnia wiadomość zostaje zawsze; historia w sesji nietknięta; log `info` z liczbą pominiętych wiadomości (bez treści).
  Testy: `tests/window.rs`.
- **Wyjście `llama-server` w dzienniku Alfy:** wiersze stderr klasyfikowane (`process::classify_line`) — błędy i brak
  pamięci (`error`, `failed`, `out of memory`…) jako `warn`, architektura modelu, KV cache, odciążenie warstw i urządzenie
  jako `info` (cel `providers_local_impl::llama_server`, zapisywane domyślnie — potwierdzenie szacunków VRAM i diagnoza
  OOM na sprzęcie); reszta `debug` pod celem `llama_server` (poza Alfą — tylko po jawnym `ALFA_LOG=…,llama_server=debug`,
  bo wiersze żądań mogą nieść treść). Wcześniej całe wyjście szło na `debug` pod celem obcym, obcinanym do `warn`.
- **Architektura potwierdzona na runnerze CI** (`rehearsal.yml`, log `llama-server` b6710): 4.5B — 60 warstw, 2 głowice KV,
  `n_ctx_train` 8192 (manifest miał `ctx = 32768` — poprawione; dłuższe `-c` niż trening pogarsza odpowiedzi), 4,76 mld
  parametrów, KV 480 MiB przy `-c 8192`; 1.5B — 32 warstwy, 2 głowice KV, `n_ctx_train` 8192, KV 256 MiB. CPU 4 vCPU:
  4.5B 6,8 tok/s, 1.5B 21 tok/s; żądanie z narzędziem (4.5B, `--jinja`) — wywołanie narzędzia.
