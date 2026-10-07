# voice-vad-contract

Kontrakt wykrywania mowy (docs/modules/voice-vad/SPEC.md): trait `Vad` (instancja na strumień: `configure`,
`push(&Frame 16 kHz mono dowolnej długości) -> Vec<VadEvent>`, `push_processed(&Processed)`, `is_speech`,
`last_prob`, `set_noise_floor`, `reset`).

Wspólne dla `-impl` i `-fake`: **`VadMachine`** — próg z histerezą (`threshold`, `hysteresis`), minimalne czasy
mowy/ciszy, próg adaptacyjny (+0,2 przy szumie −60 → −30 dBFS, zawsze w `[min_threshold, max_threshold]`);
`EnergyDetector` (prawdopodobieństwo z energii względem szumu). Domyślnie `min_speech_ms = 30` (decyzja ≤ 60 ms,
ACC-F2-voice-vad-02), `min_silence_ms = 300`. Zdarzenia `voice.vad.speech_start/speech_end`, `voice.vad.model.loaded/unloaded`.
Testy kontraktowe (`contract-tests`): segmenty mowy syntetycznej (start ≤ 60 ms), szum bez mowy, konfiguracja.
