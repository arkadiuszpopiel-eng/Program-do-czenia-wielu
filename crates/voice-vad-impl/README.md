# voice-vad-impl

Implementacja `voice-vad`: **Silero VAD (ONNX, MIT) przez `tract-onnx` 0.23.8** (czysty Rust, bez binariów ONNX
Runtime). `ort` odrzucony: pobiera ORT z sieci przy budowie (zablokowane / łańcuch dostaw), wymaga DLL.

- `tract` nie typuje grafów Silero z operatorami `If` (gałąź 8 kHz nie pasuje do okna 16 kHz; zagnieżdżone `If`
  LSTM w `silero_vad.onnx` i `_16k_op15`). Rozwiązanie: wariant **`silero_vad_op18_ifless.onnx`** (pakiet PyPI
  `silero-vad` 6.2.3) + przepisanie grafu przed `tract` (`inline.rs`): górny `If(sr == 16000)` → gałąź 16 kHz,
  usunięcie wejścia `sr` i adnotacji kształtów z symbolem `batch`.
- Model tylko ze znanym SHA-256 (`KNOWN_MODELS`; `HashPolicy::AllowUnverified` dla Voice Lab).
- Okna 512 próbek + 64 próbki kontekstu, stan RNN `[2,1,128]`; błąd modelu → detektor energii (zdarzenie).
- `VoiceVadModule::with_residency` — dzierżawa w `model-residency` (CPU, 30 MB, priorytet głosu).

Model **nie jest w repo**. Skąd wziąć: `pip download silero-vad==6.2.3 --no-deps`, rozpakować wheel (zip),
plik `silero_vad/data/silero_vad_op18_ifless.onnx` (SHA-256 `7671cd04…bd28`). Test z prawdziwym modelem i mową:

```
ALFA_SILERO_VAD=…/silero_vad_op18_ifless.onnx ALFA_SPEECH_WAV=…/jfk.wav \
  cargo test -p voice-vad-impl --test vad silero_real_model -- --ignored --nocapture
```
(`jfk.wav` — próbka whisper.cpp `samples/jfk.wav`, np. z sdist PyPI `pywhispercpp`). Wynik lokalny (2026-10-01):
mowa 11 s wykryta (start 1344 ms po 1 s ciszy, koniec 12 000 ms, 4 segmenty na pauzach), szum biały 0 zdarzeń,
406 okien w 0,58 s w buildzie debug.
