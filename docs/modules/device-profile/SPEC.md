# device-profile — SPEC (szkic v0)

## Cel
Autodetekcja sprzętu przy pierwszym uruchomieniu i po zmianie (CPU, RAM, producent i model GPU, VRAM, NPU, bateria, urządzenia audio), klasa sprzętu (baseline / standard-amd / laptop-cuda / mocny / zintegrowana), dobór profilu potoku głosu A–D i modułów rezydentnych **per maszyna**, tryb baterii, wykrywanie gry/pełnego ekranu; „Kreator sprzętu" pokazuje kompromis i pozwala nadpisać (PLAN §3.5, §6.3).

## Fala i priorytet
F1 (detekcja, id maszyny, klasa, nakładka `config/machine/<id>.toml`, onboarding). F2: profile głosu A–D, tryb baterii/termika. P0.

## Kontrakt (stan F1 — źródło prawdy: `crates/device-profile-contract`)
```rust
pub struct Profile { machine_id: MachineId /* 32 hex, SHA-256 */, os, cpu: CpuInfo { physical_cores, logical_cores, l3_cache_mb },
    ram_mb, gpus: Vec<GpuInfo { vendor, vram_mb, backends }>, npu: Option<NpuInfo>, battery: Option<BatteryInfo>,
    power: PowerState /* Ac | Battery { percent } | Unknown */, audio: Option<Vec<AudioDevice>>, emulation: Option<Emulation> }
pub enum HwClass { Baseline, StandardAmd, LaptopCuda, Strong, Integrated, Unknown }   pub enum VoiceProfile { A, B, C, D }
pub fn classify(&Profile) -> HwClass; pub fn recommend(&Profile) -> Recommendation;   // czyste, wspólne dla impl/fake
pub fn apply_overlay(&Profile, &MachineOverlay) -> Recommendation;                     // nadpisania użytkownika
impl Profile { pub fn emulate_baseline(&self) -> Profile }  // 6c/12t, 16 GB, VRAM 8 GB + korekta GPU ×2,2 / CPU +25%
pub struct Recommendation { class, voice_profile, voice_variant /* Amd16|Cuda|Full */, stt_backend, stt_model,
    llm_backend, local_llm, heavy_local_tts, residency: ResidencyBudget { vram_mb, ram_mb, desktop_reserve_mb,
    stt_tts_exclusive }, power_saving, emulation, tradeoffs: Vec<String> }
pub trait DeviceProfile { fn current(&self) -> Profile; fn recommend(&self) -> Recommendation; fn power_state(&self) -> PowerState;
    fn fullscreen_active(&self) -> bool; fn emulate(&self, Option<ResourceLimits>) -> Result<()>; fn refresh(&self) -> Result<bool>; }
```
Zdarzenia (`DeviceEvent`): `device.detected`, `device.changed` (hot-plug, nowa karta), `device.power.changed`, `device.fullscreen.changed`, `device.thermal.throttled` (F2), `device.override` (użytkownik nadpisał).
Klasy (2026-10-17): karta ≥ 20 GB + ≥ 32 GB RAM → Mocny (NVIDIA już od 12 GB — ciężki TTS na CUDA); inna niż NVIDIA ≥ 12 GB + ≥ 24 GB RAM → Standard-AMD (nazwa historyczna: każda karta Vulkan); NVIDIA ≥ 6 GB + 16 GB, gdy nie jest Mocny (także 12–24 GB przy RAM < 32 GB) → Laptop-CUDA (etykieta „karta NVIDIA (CUDA)”); AMD/Intel ≥ 6 GB + 16 GB → Baseline; sama karta zintegrowana z małą rezerwą + ≥ 16 GB RAM → Integrated (mały LLM na karcie przez Vulkan w części RAM, STT na CPU); reszta Unknown (rozmowa przez API; karta z własną pulą ≥ 3,5 GB liczy STT). Liczba wątków nie zmienia klasy — poniżej 8 wątków kompromis w rekomendacji. „Zintegrowana” — flaga sterownika (DXCore `IsIntegrated`), a bez niej < 1 GB własnej pamięci. Karta zintegrowana z rezerwą z BIOS-u ≥ 6 GB (`UMA_POOL_MIN_MB`, `GpuInfo::has_memory_pool`; np. Ryzen AI Max, Radeon 890M z 16 GB) ma własną pulę — klasy, pakiety i pomiar liczą ją jak kartę z tą pamięcią. Główna karta — z własną pulą przed zintegrowaną z małą rezerwą. Głos na karcie (DirectML) — od karty z własną pulą ≥ 12 GB (`GPU_TTS_MIN_VRAM_MB`, ADR 0028). Na baterii: bez lokalnego LLM, D → B.

## Zależności
`core-bus/registry-contract`, `platform-contract` (`HardwarePort`: DXGI/DXCore/zasilanie/MMDevice/rejestr na Windows; `WindowPort` — pełny ekran). Poza Windows sonda `sysinfo` (CPU/RAM). Konsumenci: `model-residency`, `voice-*`, `providers-local`, `memory` (konsolidacja), `notify` (nie przeszkadzać), `transfer` (`hw_class` w manifeście).

## Niezmienniki
- `MachineId` stabilny między uruchomieniami i wersjami; zmiana sprzętu (GPU/RAM) = nowa detekcja, ten sam id (nakładka pyta o ponowny dobór).
- Nakładka per maszyna zawiera tylko rzeczy fizyczne (urządzenia audio, profil, rezydencja, limity) — nigdy agentek, reguł, uprawnień.
- Nadpisanie użytkownika ma pierwszeństwo nad rekomendacją, ale UI pokazuje kompromis (np. „D-CUDA: STT i ciężki TTS nie naraz").
- Emulacja baseline (6 rdzeni, 16 GB, VRAM 8 GB, korekta GPU ×2,2) dostępna w buildzie testowym i w CI sprzętowym.
- Detekcja nie wysyła niczego poza maszynę (brak telemetrii).

## Zdolności / uprawnienia
Brak (odczyt systemu przez `SystemPort`).

## Izolacja
`inproc`, `always` (lekki nasłuch zdarzeń zasilania/pełnego ekranu; sama detekcja `on-demand`).

## Budżet zasobów
Pełna detekcja ≤ 500 ms (raz); RAM ≤ 1 MB; nasłuch zdarzeń bez pollingu.

## Konfiguracja (klucze TOML)
Nakładka `config/machine/<id>.toml`: `[machine] name, hw_class_override`, `[machine.voice] profile = "auto" | "A" | "B" | "C" | "D"`, `[machine.audio] input, output`, `[machine.resident] modules = [...]`, `[machine.limits] ram_mb, vram_mb, cpu_threads`, `[machine.battery] reduce_local_models = true`, `[machine.gaming] detect_fullscreen = true`.

## Wkład do UI
Onboarding „pomiar sprzętu", Ustawienia → Urządzenia (profil per maszyna, tryb baterii), Kreator sprzętu z kompromisami, chip profilu w pasku.

## Testy akceptacyjne
- `ACC-F1-device-profile-01`: detekcja na runnerach desktop (RDNA4/Vulkan) i laptop (RTX 4050/CUDA) daje oczekiwane klasy i backendy; fixture'y dla baseline.
- `ACC-F1-device-profile-02`: `MachineId` stabilny po restarcie i aktualizacji (10/10).
- `ACC-F2-device-profile-03`: przejście na baterię → zdarzenie ≤ 2 s; pełny ekran → zdarzenie ≤ 1 s.

## Fake
`device-profile-fake`: profile z fixture'ów (baseline, desktop, laptop), sterowane zdarzenia zasilania/pełnego ekranu/hot-plug.

## Otwarte pytania
- Ustalone (F1): `MachineId` = SHA-256 z separacją domeny nad `MachineGuid` (Windows) / `/etc/machine-id`, a bez nich nad losowym UUID zapisanym raz w katalogu stanu.
- Detekcja NPU: DXCore (adapter „generic ML” bez grafiki) — tylko raportowanie; użycie poza v1.
- Nasłuch zmian bez pollingu (`WM_POWERBROADCAST`, zdarzenia okien) — F2; w F1 `poll_changes()` wołane przez jądro.
- Ustalone (0.0.11): każda karta ma `id` (numer adaptera), `integrated` i `shared_mb`; `Profile::light_gpu()` zwraca kartę zintegrowaną do lekkich zadań tylko wtedy, gdy obok jest osobna karta główna. Dwie identyczne karty nie zlewają się w jedną.
