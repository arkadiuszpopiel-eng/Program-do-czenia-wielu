# voice-wake-impl

Implementacja `voice-wake` (docs/modules/voice-wake/SPEC.md).

**v0 (F2):** `WakeService` na `HotkeyPort` (`platform-windows-impl`: `RegisterHotKey` + `WH_KEYBOARD_LL` zgłaszający
puszczenie PTT) — rejestracja/wyrejestrowanie skrótów (niepoprawna konfiguracja nie psuje poprzedniej),
`ProcessPort::foreground_is_elevated` przy `pump` (okno administratora → komunikat w UI), `MicArbiter` — mikrofon jako
zasób wyłączny `scheduler-lite` (`Holder::User`, `Priority::UserSpeech`) na czas słuchania, `VoiceWakeModule` publikuje
`voice.wake.*`. Reakcja PTT ≤ 10 ms (ACC-F2-voice-wake-01) — pomiar na runnerze.

**v1 (F5) — słowa wywoławcze „Hej Alfa/Beta/Gama/Delta” i imiona z Kreatora** (domyślnie wyłączone, tylko lokalnie):

- `kws::load_scorer(<model>.kws.json)` → `Box<dyn KeywordScorer>` dla `WakeWordListener` z kontraktu (bufor ~2 s,
  bramka energii, detektor z histerezą i oknem odporności). Inferencja przez **`tract-onnx` 0.23.8** (czysty Rust, jak
  `voice-vad-impl`); każdy plik ONNX ma **SHA-256 w manifeście** — inny plik nie jest ładowany.
- Format manifestu `alfa-kws-v1` (`src/kws/manifest.rs`), dwa rodzaje modelu:
  - `log_mel` — klasyfikator na cechach log-mel Kaldi liczonych w Rust (`voice-dsp-contract::fbank`, 25/10 ms, FFT 512,
    `n_mels` 8–128): wejście `[1, frames, n_mels]` (`btf`), `[1, n_mels, frames]` (`bft`) lub `[1, 1, frames, n_mels]`
    (`b1tf`), wyjście `[1, K]` (wynik na etykietę; `sigmoid` / `softmax` z `background_index` / `none`), krok co
    `step_frames` ramek. Przeznaczony dla własnego treningu frazy PL (mała sieć CRNN/DS-CNN eksportowana do ONNX).
  - `openwakeword` — potok openWakeWord: `melspectrogram.onnx` (`[1, 1760]` → `[1,1,F,32]`, potem `x/10+2`),
    `embedding_model.onnx` (`[1,76,32,1]` → 96 cech co 80 ms), klasyfikator frazy (`[1,16,96]` → `[1,1]`) — po jednym
    na etykietę.
- `eval` + bin **`alfa-wake-eval`** (`check`, `run`, `freeze`, `schema`) — FAR/dzień na nagraniach tła i FRR na
  pozytywach, przegląd progów 0,30–0,95, rekomendacja progu; format w `evals/F5/voice/README.md`.

## Skąd wziąć model (nie ma go w repo)

| Element | Skąd | Licencja |
|---|---|---|
| `melspectrogram.onnx`, `embedding_model.onnx` (openWakeWord) | wydanie `v0.5.1`: `https://github.com/dscripka/openWakeWord/releases/download/v0.5.1/melspectrogram.onnx` (i `embedding_model.onnx`) — to samo pobiera `openwakeword.utils.download_models()`; PyPI `pyopen-wakeword` / `wyoming-openwakeword` mają je tylko jako `.tflite` (konwersja `tf2onnx`) | kod openWakeWord Apache-2.0; model embeddingu pochodzi z Google `speech_embedding` (Apache-2.0) — **zweryfikować przed dystrybucją** |
| Klasyfikator „Hej Alfa/Beta/Gama/Delta” (PL) | **własny trening** (openWakeWord nie ma fraz PL): notatnik treningowy openWakeWord na mowie syntetycznej (np. Piper `pl_PL`, Pocket TTS PL) + pozytywy właściciela z korpusu (typ 10), negatywy z nagrań tła PL | wynik treningu — własny; dane syntetyczne zgodnie z licencją głosów TTS; gotowe angielskie modele openWakeWord to CC BY-NC-SA 4.0 (niekomercyjne) — nie używać |
| Klasyfikator `log_mel` | własny trening (PyTorch → `torch.onnx.export`, opset ≤ 17, stały kształt wejścia) | własny |

Modele sherpa-onnx KWS (zipformer-transducer z grafem słów kluczowych) wymagają dekodowania transducera — poza v1
(manifest `kind` jest rozszerzalny).

Test z prawdziwym modelem: `ALFA_KWS_MODEL=…/model.kws.json [ALFA_KWS_WAV=…/hej-alfa.wav] cargo test -p voice-wake-impl
--test kws_onnx real_model -- --ignored --nocapture`. Pomiar F5-05/06 (maszyna z korpusem):
`cargo run -p voice-wake-impl --bin alfa-wake-eval -- run evals/corpus/f5/wake.ndjson --audio-root evals/corpus --model …
--split test --out wyniki.ndjson`.
