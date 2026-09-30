# voice-vad — SPEC (szkic v0)

## Cel
Wykrywanie mowy w ramkach po DSP: Silero VAD (MIT) lub TEN VAD na CPU (ONNX Runtime), z adaptacyjnym progiem względem szumu otoczenia; bramka dla STT (turbo halucynuje na szumie) i sygnał dla barge-in (PLAN §6.2, §6.3, §6.5).

## Fala i priorytet
F2. P0.

## Kontrakt (szkic Rust)
```rust
// voice-vad-contract — SZKIC
pub struct VadCfg { pub engine: VadEngine /* Silero | Ten */, pub threshold: f32, pub min_speech_ms: u16, pub min_silence_ms: u16, pub adaptive: bool }
pub enum VadEvent { SpeechStart { ts: Instant, prob: f32 }, SpeechEnd { ts: Instant, duration: Duration }, Prob { ts: Instant, p: f32 } }
pub trait Vad: Send + Sync {
    fn configure(&self, cfg: VadCfg) -> Result<()>;
    fn push(&self, frame: &Processed) -> Option<VadEvent>;    // 20–30 ms ramki, bez alokacji
    fn is_speech(&self) -> bool;
    fn set_noise_floor(&self, db: f32);                        // z voice-dsp
}
```
Zdarzenia: `vad.speech_start`, `vad.speech_end`, `vad.model.loaded/unloaded`.

## Zależności
`core-bus/config/log-contract`, `voice-dsp-contract` (ramki `Processed`), `model-residency-contract` (mały model na CPU, rezydentny gdy głos aktywny). Zewnętrzne: ONNX Runtime CPU, model Silero/TEN (hash, `docs/vendor/`).

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
`[voice.vad] engine = "silero"`, `threshold = 0.5`, `min_speech_ms = 100`, `min_silence_ms = 300`, `adaptive = true`, `min_threshold = 0.3`.

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
