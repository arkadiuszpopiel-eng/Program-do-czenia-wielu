# voice-dsp-contract

Kontrakt DSP mikrofonu (docs/modules/voice-dsp/SPEC.md): trait `Dsp` (instancja na strumień: `configure`,
`push_reference` — ramki z `OutputStream::drain_reference`, `process` → ramki 10 ms 16 kHz mono `Processed`,
`calibrate`, `stats`, `take_events`, `reset`).

`Processed` niesie: ramkę po AEC → NS → AGC, `aec_confidence` (0–1, 1 gdy agentka milczy), `erle_db`,
`echo_residual_db`, `noise_floor_db` (dla progu VAD), `speech_prob`/`speech_likely`, `reference_active`.
`DspCfg`: `AecMode` (`OwnReference` domyślnie / `Loopback` / `Communications` / `Off`), `NsMode`, AGC, tryb szeptu,
`reference_margin_ms` (wyprzedzenie referencji — przyczynowość AEC przy niedokładnych znacznikach).
Wspólne: `NoiseFloorTracker`, `calibration_signal()` (chirp 300→3500 Hz). Zdarzenia `voice.dsp.*`
(`calibrated`, `echo_high`, `noise.changed`, `mode.fallback`, `headphones`). Testy kontraktowe pod `contract-tests`.

`fbank` (F5): cechy log-mel w stylu Kaldi (`FbankCfg::kaldi(n_mels)` — okno Poveya 25 ms / przesunięcie 10 ms,
preemfaza 0,97, skala int16, FFT 512, opcjonalne CMN), `Fbank::compute` i strumieniowe `FbankStream` — wspólne wejście
modeli `voice-wake-impl` (KWS `log_mel`) i `voice-speaker-impl` (embedding mówcy).
