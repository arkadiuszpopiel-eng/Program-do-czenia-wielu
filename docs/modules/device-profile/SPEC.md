# device-profile — SPEC (szkic v0)

## Cel
Autodetekcja sprzętu przy pierwszym uruchomieniu i po zmianie (CPU, RAM, producent i model GPU, VRAM, NPU, bateria, urządzenia audio), klasa sprzętu (baseline / standard-amd / laptop-cuda / mocny), dobór profilu potoku głosu A–D i modułów rezydentnych **per maszyna**, tryb baterii, wykrywanie gry/pełnego ekranu; „Kreator sprzętu" pokazuje kompromis i pozwala nadpisać (PLAN §3.5, §6.3).

## Fala i priorytet
F1 (detekcja, id maszyny, klasa, nakładka `config/machine/<id>.toml`, onboarding). F2: profile głosu A–D, tryb baterii/termika. P0.

## Kontrakt (szkic Rust)
```rust
// device-profile-contract — SZKIC
pub struct MachineInfo { pub id: MachineId /* stabilny, z identyfikatorów sprzętu */, pub name: String, pub os: OsInfo,
    pub cpu: CpuInfo { cores, threads, l3_mb }, pub ram_mb: u32, pub gpus: Vec<GpuInfo { vendor, model, vram_mb, backends: Vec<Backend> }>,
    pub npu: Option<NpuInfo>, pub battery: Option<BatteryInfo>, pub audio: Vec<AudioDevice> }
pub enum HwClass { Baseline, StandardAmd, LaptopCuda, Strong, Unknown }
pub enum VoiceProfile { A, B, C, D }
pub struct Recommendation { pub class: HwClass, pub voice_profile: VoiceProfile, pub llm_backend: Backend, pub resident: Vec<ModuleId>,
                            pub limits: ResourceLimits, pub tradeoffs: Vec<String> }
pub trait DeviceProfile: Send + Sync {
    fn current(&self) -> MachineInfo;
    fn recommend(&self) -> Recommendation;
    fn power_state(&self) -> PowerState /* Ac | Battery { pct } */;
    fn fullscreen_active(&self) -> bool;
    fn emulate(&self, limits: Option<ResourceLimits>) -> Result<()>;   // emulacja baseline (testy)
}
```
Zdarzenia: `device.detected`, `device.changed` (hot-plug, nowa karta), `device.power.changed`, `device.fullscreen.changed`, `device.thermal.throttled`, `device.override` (użytkownik nadpisał).

## Zależności
`core-bus/config/log-contract`, `platform-windows-contract` (WMI/DXGI/zasilanie/audio/okna). Konsumenci: `model-residency`, `voice-*`, `providers-local`, `memory` (konsolidacja), `notify` (nie przeszkadzać), `transfer` (`hw_class` w manifeście).

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
- Źródło `MachineId` (hash z UUID płyty/dysku vs losowy zapisany w `%LOCALAPPDATA%`) — do ustalenia w SPEC v1.
- Detekcja NPU i jej użycie — poza v1, tylko raportowanie.
