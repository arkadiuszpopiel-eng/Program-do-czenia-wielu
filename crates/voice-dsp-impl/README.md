# voice-dsp-impl

Implementacja `voice-dsp`. Potok 48 kHz, bloki 10 ms: referencja (oś czasu odtworzenia) → **AEC3** → **RNNoise**
→ AGC → 16 kHz.

- **AEC: `sonora` 0.2.0** — czysto-rustowy port WebRTC Audio Processing (AEC3), BSD-3-Clause. Wybrany zamiast
  `aec3` 0.4 (młodszy, mniej użytkowników) i zamiast własnego PBFDAF (niepotrzebny). Referencja = własny strumień
  TTS (`push_reference` z czasem odtworzenia), czytana z wyprzedzeniem `reference_margin_ms` minus skalibrowane
  opóźnienie pętli; resztę opóźnienia (np. 40 ms akustyki) estymuje AEC3. Filtr górnoprzepustowy AEC3 włączony
  (domyślny WebRTC).
- **NS: `nnnoiseless` 0.5.2** (RNNoise w Rust, BSD-3-Clause, bez domyślnych feature'ów) + jego VAD jako `speech_prob`.
- **AGC**: własny (`agc::Agc`), cel −20 dBFS, +20 dB (tryb szeptu +30 dB), −20 dB, bramkowany mową, ogranicznik −1 dBFS.
- **Kalibracja pętli**: korelacja wzajemna FFT (`rustfft` 6.4.1) nagrania z `calibration_signal` (±1 ms w testach,
  także end-to-end przez `voice-audio-fake`).
- Metryki: ERLE (wygładzony), pewność AEC = ERLE/20 dB, słuchawki (mikrofon ≥ 30 dB ciszej niż referencja przez
  90% z 2 s), `echo_high` (ERLE < 6 dB po 2 s), zmiany szumu ≥ 6 dB.

Wyniki (`cargo test -p voice-dsp-impl --test aec -- --nocapture`): echo = wyjście opóźnione o 40 ms (nieznane DSP)
przez „pokój” 25 ms + szum: **ERLE 49 dB** po zbieżności (próg ≥ 15 dB), mowa bliska: Δpoziom −0,2 dB,
korelacja obwiedni 0,997; podwójna rozmowa: korelacja obwiedni z mową bliską 0,89 (bez AEC 0,64). Korelacja próbek
mowy bliskiej to ~0,72 z powodu przesunięcia fazy HPF (bez HPF 0,99) — VAD/STT nie są na nią czułe.
RNNoise na szumie białym −40 dBFS: −8,7 dB, mowa wykryta w 70/70 ramek, 0/90 fałszywych.

DSP działa w wątku przetwarzania tuż za kolejką SPSC (nie w callbacku RT; `sonora` może alokować).
Do zrobienia na sprzęcie: ACC-F0-voice-dsp-01 (trzy warianty referencji), ACC-F2-voice-dsp-02/03.
