# sonora (AEC3), nnnoiseless (RNNoise), rustfft — `voice-dsp-impl`

- **`sonora` 0.2.0** (BSD-3-Clause; podcrate'y `sonora-aec3/-ns/-agc2/-common-audio/-simd` 0.2.0, BSD-3-Clause):
  czysto-rustowy port WebRTC Audio Processing. Bez feature `examples` (ciągnie `cpal`, zakazany w `deny.toml`).
- **`nnnoiseless` 0.5.2** (BSD-3-Clause): RNNoise w Rust; `default-features = false` (domyślne = CLI + `dasp`).
- **`rustfft` 6.4.1** (MIT OR Apache-2.0).
- Zweryfikowano kompilacją i testami 2026-10-01 (`cargo test -p voice-dsp-impl`).

## Używane API
```rust
use sonora::{AudioProcessing, Config, StreamConfig};
use sonora::config::EchoCanceller;
let mut apm = AudioProcessing::builder()
    .config(Config { echo_canceller: Some(EchoCanceller::default()), ..Config::default() })
    .capture_config(StreamConfig::new(48_000, 1)).render_config(StreamConfig::new(48_000, 1)).build();
apm.process_render_f32(&[&reference_10ms[..]], &mut [&mut sink[..]])?;   // najpierw render (referencja)
apm.set_stream_delay_ms(margin_ms as i32);                                 // podpowiedź; AEC3 estymuje resztę
apm.process_capture_f32(&[&mic_10ms[..]], &mut [&mut out[..]])?;           // bloki 10 ms, deinterleaved
apm.statistics();                                                          // erle/erl/delay (Option)

let mut ns = nnnoiseless::DenoiseState::new();                             // Box<DenoiseState>
let vad_prob = ns.process_frame(&mut out480, &in480);                      // 480 próbek @ 48 kHz, skala i16 (×32768)

let mut planner = rustfft::FftPlanner::<f32>::new();
let fwd = planner.plan_fft_forward(n); fwd.process(&mut buf_complex);       // buf: Vec<Complex32>
```

## Pułapki
- `EchoCanceller::default()` włącza filtr górnoprzepustowy: faza niskich harmonicznych się przesuwa
  (korelacja próbek ~0,72 przy identycznym widmie) — oceniaj poziom/obwiednię, nie przebieg.
- Referencja musi **wyprzedzać** echo w mikrofonie (przyczynowość) — stąd `reference_margin_ms`.
- AEC3 może alokować w `process_*` — uruchamiamy w wątku przetwarzania, nie w callbacku urządzenia.
- RNNoise: pierwsza ramka ma artefakty narastania; wyjście opóźnione o jedną ramkę (10 ms).
