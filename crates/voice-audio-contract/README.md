# voice-audio-contract

Kontrakt wejścia/wyjścia audio Voice Suite (docs/modules/voice-audio/SPEC.md, ADR 0011).

- **Typy**: `Frame` (PCM `f32`, 8–48 kHz, mono/stereo, `MediaTime` = czas z zegara urządzenia), `AudioFormat`,
  `AudioDevice`/`DeviceEvent` (hot-plug, domyślne), `StreamConfig`, `SourceId` (routing per agentka:
  `Tts(persona)`, `Filler(persona)`, `Earcon`), `Ducking` (−15 dB, rampa ≤ 50 ms), `LoopLatency`,
  `PlaybackPosition` (wyrenderowane próbki − opóźnienie wyjścia = „usłyszany prefiks”).
- **Traity**: `AudioIo` (urządzenia, `open_input/open_output/open_loopback`), `InputStream`, `OutputStream`
  (kolejka fragmentów TTS, `end_utterance`, `duck/unduck`, `stop_all` ≤ 20 ms, `position`, `drain_reference`
  = referencja AEC z czasem odtworzenia).
- **Wspólna logika RT** (te same bajty w `-impl` i `-fake`): `mixer::mixer()` → `MixerControl` (wątek sterujący)
  + `MixerRender` (wątek urządzenia: zero alokacji/blokad, kolejki SPSC `rtrb`, atomiki, seqlock postępu);
  tor głosu odtwarza jedną otwartą wypowiedź naraz (`AudioError::VoiceBusy`), tor efektów miksuje earcony;
  `capture_ring()` (przechwytywanie → ramki z ekstrapolowanym czasem); `MixerOutput` (downmix, resampling,
  normalizacja `LoudnessNormalizer`).
- **Narzędzia**: `Resampler` (okienkowany sinc/Kaiser, dowolny stosunek, strumieniowy; SNR > 60 dB, tłumienie
  pasma zaporowego > 60 dB w testach), `wav` (PCM16/24/32, float32), `synth` (sinus, szum z ziarnem, „mowa
  syntetyczna”, chirp, estymacja F0, korelacja) — do deterministycznych testów wszystkich modułów głosu.
- **Zdarzenia**: `voice.audio.device.changed`, `.stream.started/stopped`, `.underrun`, `.exclusive_conflict`,
  `.ducked/.unducked`, `.latency.calibrated`, `.playback.started/finished`, `.bluetooth_warning`.
- **Testy kontraktowe** (feature `contract-tests`): `contract_tests::run_all(factory)` — atrapa z wirtualnym
  zegarem, `-impl` na sprzęcie (`#[ignore]`).

Nowa zależność: `rtrb` 0.4.0 (MIT OR Apache-2.0).
