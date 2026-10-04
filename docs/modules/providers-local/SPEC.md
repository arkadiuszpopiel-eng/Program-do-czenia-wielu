# providers-local — SPEC (v1, F1)

## Cel
Lokalny `ModelProvider`: wbudowany llama.cpp (Vulkan/CUDA/CPU) jako osobny proces; bez kluczy API jedyny „mózg" (MVP), z kluczami — komendy, fallback, offline. Ollama i LM Studio jako zewnętrzne endpointy przez adapter generyczny (w `providers-api`), nie tutaj. Pobieranie modelu 3–4,5B Q4_K_M w onboardingu (PLAN §1.2, §5.2, §16.2 F1).

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
  walidacja: `.gguf`, bez kwantów IQ, https. Domyślny: Bielik 4.5B v3.0 Instruct Q4_K_M.
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
Wspólne: `[providers.local] default_model = "bielik-4.5b-q4_k_m"` (do potwierdzenia), `ctx = 8192`. Per maszyna: `[providers.local.machine] backend = "auto" | "vulkan" | "cuda" | "cpu"`, `gpu_layers = "auto"`, `idle_unload = "10m"`, `max_vram_mb`.

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

