# voice-dsp — SPEC (v0, zaimplementowany w F2)

## Cel
Przetwarzanie sygnału mikrofonu: AEC z **własnym strumieniem TTS jako referencją** (awaryjnie loopback lub tryb Communications), redukcja szumu (RNNoise; DeepFilterNet3 opcja), AGC, adaptacja do szumu otoczenia, tryb szeptu; pewność AEC jako sygnał dla filtrowania „przerwań" (PLAN §6.2, §6.8, §12.4).

## Fala i priorytet
F0: spike (a) porównanie referencji AEC; F2: moduł. P0.

## Kontrakt (Rust, `voice-dsp-contract`)
```rust
pub struct DspCfg { pub aec: AecMode /* OwnReference | Loopback | Communications | Off */, pub ns: NsMode /* RnNoise | DeepFilter | Off */,
                    pub agc: bool, pub agc_target_db: f32, pub whisper_mode: bool, pub reference_margin_ms: u32 }
pub struct Processed { pub frame: Frame /* 10 ms, 16 kHz mono */, pub echo_residual_db: f32, pub erle_db: f32, pub noise_floor_db: f32,
                       pub aec_confidence: f32, pub speech_prob: f32, pub speech_likely: bool, pub reference_active: bool }
pub trait Dsp: Send {                                      // jedna instancja na strumień, wątek przetwarzania (nie callback RT)
    fn configure(&mut self, cfg: DspCfg) -> Result<(), DspError>;
    fn push_reference(&mut self, frame: &Frame);          // OutputStream::drain_reference (czas odtworzenia)
    fn process(&mut self, mic: &Frame) -> Result<Vec<Processed>, DspError>;
    fn calibrate(&mut self, played: &[f32], recorded: &[f32], rate: u32) -> Result<Calibration, DspError>;
    fn stats(&self) -> DspStats;  fn take_events(&mut self) -> Vec<DspEvent>;  fn reset(&mut self);
}
```
Zdarzenia: `voice.dsp.calibrated`, `voice.dsp.echo_high` (echo nieusuwalne → agresywniejsze progi barge-in / słuchawki), `voice.dsp.noise.changed`, `voice.dsp.mode.fallback`, `voice.dsp.headphones`.

## Zależności
`core-bus/config/log-contract`, `voice-audio-contract` (ramki, opóźnienie, loopback). Zewnętrzne: `sonora` 0.2 (AEC3), `nnnoiseless` 0.5 (RNNoise), `rustfft` 6.4 (`docs/vendor/sonora.md`).

## Niezmienniki
- Referencja = własny strumień TTS z dokładnym znacznikiem czasu odtworzenia; loopback tylko jako fallback (zdarzenie).
- Brak blokad w `process`; wątek przetwarzania tuż za kolejką SPSC (AEC3 może alokować — nie w callbacku RT); bloki 10 ms, 48 kHz wewnętrznie (RNNoise; resampling na wejściu), wyjście dla VAD/STT 16 kHz mono.
- `aec_confidence` towarzyszy każdej ramce; `voice-dialog` używa jej do potwierdzania barge-in, `improver` do odfiltrowywania fałszywych „przerwań" jako sygnału.
- Tryb Communications nie jest domyślny (tłumi inne strumienie o 80%).
- Słuchawki wykryte (echo ≈ 0) → informacja dla `voice-dialog` (agresywniejsze progi).

## Zdolności / uprawnienia
Brak.

## Izolacja
`inproc`, `lazy` (razem z `voice-audio`).

## Budżet zasobów
CPU ≤ 5% jednego rdzenia baseline (AEC + RNNoise, 16 kHz); RAM ≤ 20 MB; opóźnienie dodane ≤ 10 ms.

## Konfiguracja (klucze TOML)
`[voice.dsp] aec = "own_reference"`, `ns = "rnnoise"`, `agc = true`, `whisper_mode = false`, `auto_calibrate = true`; per maszyna: `[machine.audio.dsp] headphones = "auto"`.

## Wkład do UI
Ustawienia → Głos → Urządzenia (kalibracja, wskaźnik echa/szumu), diagnostyka w Voice Lab.

## Testy akceptacyjne
- `ACC-F0-voice-dsp-01`: spike (a) — tabela: własna referencja vs loopback vs Communications (tłumienie echa dB, opóźnienie) na desktopie i laptopie (wbudowane głośniki/mikrofon).
- `ACC-F2-voice-dsp-02`: fałszywe przerwania ≤ 1/godz. przy 1 h TTS przez głośniki laptopa + tło TV bez mowy właściciela (wspólnie z `voice-dialog`).
- `ACC-F2-voice-dsp-03`: deterministyczny test na nagraniach (`voice-audio-fake`): echo residual poniżej progu na ≥ 95% ramek z korpusu.

## Fake
`voice-dsp-fake`: przepuszcza ramki z fixture'ów i zwraca zaprogramowane `aec_confidence`/`speech_likely` (skrypt po czasie) — do testów `voice-dialog`.

## Otwarte pytania
- Porównanie trzech wariantów referencji na sprzęcie (spike a, ACC-F0-voice-dsp-01) — wymaga desktopu i laptopa.
- DeepFilterNet3 na baseline (CPU) — pomiar; domyślnie RNNoise.

## Decyzje v0 (F2)
- AEC: `sonora` 0.2 (czysto-rustowy port WebRTC AEC3, BSD-3-Clause); wynik testu syntetycznego: ERLE 49 dB, mowa bliska zachowana (poziom −0,2 dB, obwiednia 0,997). NS: `nnnoiseless` (RNNoise). AGC własny. Kalibracja: korelacja FFT (`rustfft`).
- Przetwarzanie 48 kHz w blokach 10 ms (wymóg RNNoise), wyjście 16 kHz; opóźnienie dodane ≈ 9 ms (AEC3).
- `process` może alokować (AEC3) — dlatego DSP działa w wątku przetwarzania tuż za kolejką SPSC, nie w callbacku urządzenia.
- DeepFilterNet3 w v0: zastępczo RNNoise + `voice.dsp.mode.fallback`.

## Zmiany F5 (addytywne)
- `voice_dsp_contract::fbank` — log-mel Kaldi (okno Poveya 25/10 ms, preemfaza 0,97, skala int16, FFT 512, `n_mels` 8–128, CMN): `Fbank::compute`, strumieniowe `FbankStream`. Wejście modeli KWS (`voice-wake` v1, rodzaj `log_mel`) i embeddingu mówcy (`voice-speaker`). Parametry jak domyślne `kaldi-native-fbank` (sherpa-onnx/WeSpeaker); testy: FFT vs DFT naiwna, pik tonu we właściwym paśmie, strumień = wsad, CMN. Zgodność numeryczna z referencją Kaldi — do potwierdzenia testem z prawdziwym modelem (`#[ignore]`, README `voice-speaker-impl`).
