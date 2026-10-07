# voice-vad — SPEC (v0, zaimplementowany w F2)

## Cel
Wykrywanie mowy w ramkach po DSP: Silero VAD (MIT) lub TEN VAD na CPU (ONNX przez `tract`, czysty Rust), z adaptacyjnym progiem względem szumu otoczenia; bramka dla STT (turbo halucynuje na szumie) i sygnał dla barge-in (PLAN §6.2, §6.3, §6.5).

## Fala i priorytet
F2. P0.

## Kontrakt (Rust, `voice-vad-contract`)
```rust
pub struct VadCfg { pub engine: VadEngine /* Silero | Ten | Energy */, pub threshold: f32, pub hysteresis: f32,
                    pub min_speech_ms: u16 /* 30 */, pub min_silence_ms: u16 /* 300 */, pub adaptive: bool, pub min_threshold: f32, pub max_threshold: f32 }
pub enum VadEvent { SpeechStart { ts: MediaTime, prob: f32 }, SpeechEnd { ts: MediaTime, duration: Duration } }
pub trait Vad: Send {                                      // instancja na strumień
    fn configure(&mut self, cfg: VadCfg) -> Result<(), VadError>;
    fn push(&mut self, frame: &Frame) -> Result<Vec<VadEvent>, VadError>;   // 16 kHz mono, dowolna długość
    fn push_processed(&mut self, p: &Processed) -> Result<Vec<VadEvent>, VadError>;
    fn is_speech(&self) -> bool;  fn last_prob(&self) -> f32;  fn set_noise_floor(&mut self, db: f32);  fn reset(&mut self);
}
// Wspólne: VadMachine (histereza, min. czasy, próg adaptacyjny w [min, max]), EnergyDetector.
```
Zdarzenia: `voice.vad.speech_start`, `voice.vad.speech_end`, `voice.vad.model.loaded/unloaded`.

## Zależności
`core-bus/config/log-contract`, `voice-dsp-contract` (ramki `Processed`), `model-residency-contract` (mały model na CPU, rezydentny gdy głos aktywny). Zewnętrzne: `tract-onnx` 0.23 (`docs/vendor/tract-onnx.md`), model Silero (hash; poza repo).

## Niezmienniki
- Działa wyłącznie na CPU; model z hashem (ONNX).
- Opóźnienie decyzji `SpeechStart` ≤ 60 ms od początku mowy (ramka + model); `SpeechEnd` wg `min_silence_ms` (koniec tury decyduje `voice-turn`, nie VAD).
- Brak alokacji w `push`; stan resetowany przy zmianie urządzenia.
- Próg adaptacyjny nigdy poniżej minimum z konfiguracji (ochrona przed „mową z szumu").
- Podczas `Speaking` (agentka mówi) VAD działa na sygnale po AEC — wykrycie mowy tu uruchamia ducking w `voice-dialog`.

## Zdolności / uprawnienia
Brak.

## Izolacja
`inproc`, `lazy` (z potokiem głosu).

## Budżet zasobów
CPU ≤ 2% jednego rdzenia baseline; RAM ≤ 30 MB (model ≈ 2 MB + runtime); opóźnienie ≤ 60 ms.

## Konfiguracja (klucze TOML)
`[voice.vad] engine = "silero"`, `threshold = 0.5`, `hysteresis = 0.15`, `min_speech_ms = 30`, `min_silence_ms = 300`, `adaptive = true`, `min_threshold = 0.3`, `max_threshold = 0.8`.

## Wkład do UI
Stan „słyszy Cię" w trybie głosowym i pigułce; Ustawienia → Głos → Tury i barge-in (czułość).

## Testy akceptacyjne
- `ACC-F2-voice-vad-01`: na korpusie własnym (zestaw zamrożony, cisza/szum/głośniki/słuchawki): precision/recall segmentów mowy ≥ 95% / ≥ 95% (progi do potwierdzenia w ACCEPTANCE).
- `ACC-F2-voice-vad-02`: opóźnienie `SpeechStart` ≤ 60 ms p95 (test z wirtualnym zegarem na WAV).
- `ACC-F2-voice-vad-03`: 1 h tła TV bez mowy właściciela → segmenty „mowy" nie prowadzą do fałszywych przerwań > 1/h (z `voice-dialog`, `voice-dsp`).

## Fake
`voice-vad-fake`: zdarzenia mowy z pliku adnotacji (czas start/stop) zamiast modelu — deterministyczny potok.

## Otwarte pytania
- Silero vs TEN VAD (jakość na PL, licencja) — Voice Lab w F2; do ustalenia w SPEC v1.

## Decyzje v0 (F2)
- Silero przez `tract-onnx` (czysty Rust); `ort` odrzucony (pobieranie binariów ORT przy budowie). `tract` nie obsługuje `If` w eksportach Silero — używamy `silero_vad_op18_ifless.onnx` (silero-vad 6.2.3, SHA-256 `7671cd04…bd28`) i przepisujemy górny `If(sr)` na gałąź 16 kHz przed `tract`. Model poza repo; test z prawdziwym modelem i mową `#[ignore]` (`ALFA_SILERO_VAD`, `ALFA_SPEECH_WAV`) — zweryfikowany lokalnie.
- `min_speech_ms` domyślnie 30 (nie 100): wymóg „SpeechStart ≤ 60 ms” (ACC-F2-voice-vad-02); krótkie zakłócenia odcina potwierdzenie barge-in w `voice-dialog`.
- Atrapa: VAD energetyczny albo skrypt przedziałów.
