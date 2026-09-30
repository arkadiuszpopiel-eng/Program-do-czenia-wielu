# voice-audio — SPEC (szkic v0)

## Cel
Wejście/wyjście audio na WASAPI (crate `wasapi`): wybór urządzeń, hot-plug, wątek RT, mikser wyjścia z duckingiem, normalizacja głośności, routing per agentka, loopback jako referencja awaryjna dla AEC, earcony; autokalibracja opóźnienia pętli (PLAN §6.2, §6.8, §1.2).

## Fala i priorytet
F0: spike (a) pętla głosowa; F2: moduł. P0.

## Kontrakt (szkic Rust)
```rust
// voice-audio-contract — SZKIC
pub struct AudioDevice { pub id: DeviceId, pub name: String, pub kind: DeviceKind /* Input | Output */, pub default: bool, pub bluetooth: bool }
pub struct Frame { pub pcm: Arc<[f32]>, pub sample_rate: u32, pub channels: u8, pub ts_device: Instant }   // 10–20 ms
pub trait AudioIo: Send + Sync {
    fn devices(&self) -> Vec<AudioDevice>;
    fn open_input(&self, dev: Option<DeviceId>, cfg: StreamCfg) -> Result<InputStream>;    // SPSC bez alokacji w callbacku
    fn open_output(&self, dev: Option<DeviceId>, cfg: StreamCfg) -> Result<OutputHandle>;
    fn play(&self, src: SourceId /* Tts(PersonaId) | Earcon | Filler */, chunk: Frame) -> Result<()>;
    fn duck(&self, db: f32, attack: Duration) -> Result<()>;      // −15 dB, < 50 ms
    fn stop_all(&self) -> Result<()>;                              // twardy stop < 20 ms
    fn latency(&self) -> LoopLatency;                              // GetStreamLatency + kalibracja
    fn loopback(&self) -> Result<InputStream>;                     // referencja awaryjna AEC
}
```
Zdarzenia: `audio.device.changed` (hot-plug, domyślne), `audio.stream.started/stopped`, `audio.underrun`, `audio.exclusive_conflict`, `audio.ducked`, `audio.latency.calibrated`.

## Zależności
`core-bus/config/log-contract`, `platform-windows-contract` (urządzenia, sesja), `device-profile-contract` (nakładka: urządzenia per maszyna), `scheduler-lite-contract` (zasób wyłączny `speaker`). Zewnętrzne: `wasapi` (`docs/vendor/wasapi.md`); `cpal` odrzucony (loopback).

## Niezmienniki
- W callbacku RT: zero alokacji, zero blokad, zero IPC/Wasm, zero logowania; komunikacja przez kolejki SPSC; zdarzenia publikowane spoza wątku RT.
- Jedna agentka mówi naraz (`speaker` jako zasób wyłączny w `scheduler-lite`); mikser nie miesza dwóch strumieni TTS.
- Ducking ≤ 50 ms od sygnału; `stop_all` opróżnia kolejkę i ucisza ≤ 20 ms (wkład do kill-switch < 200 ms).
- Hot-plug bez restartu potoku; zmiana urządzenia = zdarzenie + ponowna kalibracja.
- Konflikt z aplikacją w trybie wyłącznym → stan błędu widoczny w UI, nie cicha awaria.
- Bluetooth HFP wykryty → ostrzeżenie w UI (16 kHz, +150–300 ms).

## Zdolności / uprawnienia
Dostęp do mikrofonu = zgoda systemowa Windows (brak → `ms-settings:privacy-microphone`); brak tokenów Brokera. Wskaźnik prywatności w zasobniku, gdy wejście otwarte.

## Izolacja
`inproc`, `lazy` (ładowany przy pierwszym użyciu głosu, zwalniany po bezczynności — konfigurowalnie), wątek RT o podniesionym priorytecie (MMCSS).

## Budżet zasobów
RAM ≤ 10 MB; CPU ≤ 2% jednego rdzenia baseline przy otwartym wejściu i wyjściu; opóźnienie bufora wejścia 10–20 ms; bez underrunów w 1 h.

## Konfiguracja (klucze TOML)
Per maszyna: `[machine.audio] input = "<id>"`, `output = "<id>"`, `sample_rate = 48000`, `frame_ms = 20`, `exclusive = false`, `duck_db = -15`, `idle_unload = "5m"`; wspólne: `[voice.audio] normalize = true`, `earcon_volume = 0.3`.

## Wkład do UI
Wybór urządzenia audio jednym kliknięciem (tryb głosowy), wskaźniki głośności (≤ 30 kl./s), stany błędów (brak zgody, konflikt, zmiana urządzenia — §14.4), Ustawienia → Głos → Urządzenia.

## Testy akceptacyjne
- `ACC-F2-voice-audio-01`: 1 h ciągłej pracy na desktopie i laptopie (wbudowany mikrofon/głośniki) — 0 underrunów, 0 crashy.
- `ACC-F2-voice-audio-02`: ducking ≤ 50 ms, `stop_all` ≤ 20 ms (pomiar loopbackiem, wirtualny kabel).
- `ACC-F2-voice-audio-03`: hot-plug (odłączenie słuchawek) → potok wznowiony na domyślnym urządzeniu ≤ 1 s.
- `ACC-F2-voice-audio-04`: test „brak alokacji w callbacku" (alokator liczący, `-impl`).

## Fake
`voice-audio-fake`: odtwarzanie WAV jako wejście, przechwytywanie wyjścia do bufora, wirtualny zegar, symulowane hot-plug/konflikty i opóźnienie urządzenia — podstawa deterministycznych testów całego potoku.

## Otwarte pytania
- Tryb Communications vs własna referencja AEC (spike a) — ADR (11).
- Loopback per-proces (Win10 20348+) jako zapas — do potwierdzenia na baseline Win11.
