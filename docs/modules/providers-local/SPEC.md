# providers-local — SPEC (szkic v0)

## Cel
Lokalny `ModelProvider`: wbudowany llama.cpp (Vulkan/CUDA/CPU) jako osobny proces; bez kluczy API jedyny „mózg" (MVP), z kluczami — komendy, fallback, offline. Ollama i LM Studio jako zewnętrzne endpointy przez adapter generyczny (w `providers-api`), nie tutaj. Pobieranie modelu 3–4,5B Q4_K_M w onboardingu (PLAN §1.2, §5.2, §16.2 F1).

## Fala i priorytet
F1 (chat + embeddings lokalne, pobieranie modelu, Vulkan/CUDA/CPU). P0 — wymagany w MVP.

## Kontrakt (szkic Rust)
```rust
// providers-local-contract — SZKIC (implementuje ModelProvider z providers-api-contract / providers-common)
pub struct LocalModel { pub id: ModelId, pub file: PathBuf, pub sha256: Hash, pub params_b: f32, pub quant: Quant /* Q4_K_M | Q5_0 | ... (bez IQ) */,
                        pub ctx: u32, pub vram_mb_est: u32, pub ram_mb_est: u32, pub license: String }
pub enum Backend { Vulkan, Cuda, Cpu }
pub trait LocalProvider: ModelProvider {
    fn models(&self) -> Vec<LocalModel>;
    fn download(&self, spec: ModelSpec, cancel: CancelToken) -> BoxStream<DownloadProgress>;  // wznawialne, hash po pobraniu
    fn load(&self, id: ModelId, backend: Backend, budget: ResidencyLease) -> Result<()>;
    fn unload(&self, id: ModelId) -> Result<()>;
    fn bench(&self, id: ModelId) -> Result<Bench /* tok/s, ttft */>;
}
```
Zdarzenia: `local.model.download.progress/finished/failed`, `local.model.loaded/unloaded`, `local.backend.fallback` (Vulkan → CPU), `local.sidecar.crashed`.

## Zależności
`providers-api-contract` (ModelProvider), `core-bus/config/log-contract`, `model-residency-contract` (lease VRAM/RAM — F2; w F1 prosty limit), `device-profile-contract` (backend), `platform-windows-contract` (spawn sidecar, Job Object). Zewnętrzne: llama.cpp (wersja przypięta, `docs/vendor/llama-cpp.md`).

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
- Wybór modelu domyślnego (Bielik 4.5B vs inne) i próg jakości tool-use po polsku — pomiar F3; do ustalenia w SPEC v1.
- Protokół sidecara: własny JSON-RPC vs `llama-server` HTTP na localhost (ryzyko: nasłuch TCP) — do ustalenia w SPEC v1 (preferencja: pipe).
