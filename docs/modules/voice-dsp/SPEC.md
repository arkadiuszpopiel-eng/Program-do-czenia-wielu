# voice-dsp — SPEC (szkic v0)

## Cel
Przetwarzanie sygnału mikrofonu: AEC z **własnym strumieniem TTS jako referencją** (awaryjnie loopback lub tryb Communications), redukcja szumu (RNNoise; DeepFilterNet3 opcja), AGC, adaptacja do szumu otoczenia, tryb szeptu; pewność AEC jako sygnał dla filtrowania „przerwań" (PLAN §6.2, §6.8, §12.4).

## Fala i priorytet
F0: spike (a) porównanie referencji AEC; F2: moduł. P0.

## Kontrakt (szkic Rust)
```rust
// voice-dsp-contract — SZKIC
pub struct DspCfg { pub aec: AecMode /* OwnReference | Loopback | Communications | Off */, pub ns: NsMode /* RnNoise | DeepFilter | Off */,
                    pub agc: bool, pub whisper_mode: bool }
pub struct Processed { pub frame: Frame, pub echo_residual_db: f32, pub noise_floor_db: f32, pub aec_confidence: f32, pub speech_likely: bool }
pub trait Dsp: Send + Sync {
    fn configure(&self, cfg: DspCfg) -> Result<()>;
    fn push_reference(&self, tts_frame: &Frame, play_ts: Instant);   // to, co właśnie gra (z voice-audio)
    fn process(&self, mic: &Frame) -> Processed;                      // w wątku RT lub tuż za nim, bez alokacji
    fn calibrate(&self) -> Result<Calibration /* opóźnienie pętli, tłumienie echa */>;
    fn stats(&self) -> DspStats;
}
```
Zdarzenia: `dsp.calibrated`, `dsp.echo_high` (echo nieusuwalne → agresywniejsze progi barge-in / słuchawki), `dsp.noise.changed`, `dsp.mode.fallback` (OwnReference → Loopback → Communications).

## Zależności
`core-bus/config/log-contract`, `voice-audio-contract` (ramki, opóźnienie, loopback). Zewnętrzne: crate AEC (`aec3`/`sonora` — kandydaci), RNNoise (`docs/vendor/`).

## Niezmienniki
- Referencja = własny strumień TTS z dokładnym znacznikiem czasu odtworzenia; loopback tylko jako fallback (zdarzenie).
- Brak alokacji/blokad w `process` (ścieżka RT); ramki 10–20 ms, 16 kHz wewnętrznie (resampling na wejściu), wyjście dla VAD/STT 16 kHz mono.
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
- Wybór crate AEC (`aec3` vs `sonora`) i licencja — ADR (11) po spike (a).
- DeepFilterNet3 na baseline (CPU) — pomiar; domyślnie RNNoise.
