# tract-onnx — Silero VAD w czystym Rust (`voice-vad-impl`)

- Wersja: **`tract-onnx` 0.23.8** (MIT OR Apache-2.0; ciągnie `tract-core/-hir/-nnef/-linalg` 0.23.8).
  Docs: https://docs.rs/tract-onnx/0.23.8 · zweryfikowano z prawdziwym modelem 2026-10-01.
- Model: **`silero_vad_op18_ifless.onnx`** z pakietu PyPI `silero-vad` 6.2.3 (MIT),
  SHA-256 `7671cd04b004e9076da0d4a7b1a5aec36adf161c39230c1cb94a4fd5db6bbd28`. Poza repo.

## Używane API
```rust
use tract_onnx::prelude::*;
let onnx = tract_onnx::onnx();
let mut proto = onnx.proto_model_for_path(path)?;          // tract_onnx::pb::ModelProto (prost) — można przepisać graf
let mut model = onnx.model_for_proto_model(&proto)?;       // InferenceModel
let names: Vec<String> = model.input_outlets()?.iter().map(|o| model.node(o.node).name.clone()).collect();
model = model.with_input_fact(i, f32::fact([1, 576]).into())?;      // input: 64 kontekstu + 512 próbek
model = model.with_input_fact(s, f32::fact([2, 1, 128]).into())?;   // state
let plan = model.into_optimized()?.into_runnable()?;                // Arc<TypedRunnableModel>
let x = Tensor::from_shape(&[1, 576], &samples)?;
let out = plan.run(tvec![x.into_tvalue(), state.clone().into_tvalue()])?;
let p: f32 = *out[0].to_plain_array_view::<f32>()?.iter().next().unwrap_or(&0.0);   // 0.23: bez `as_slice`
let state = out[1].clone().into_tensor();
```

## Pułapki
- `tract` analizuje **obie** gałęzie `If`; eksporty Silero mają `If(sr == 16000)` (gałąź 8 kHz nie typuje się dla
  okna 16 kHz), a `silero_vad.onnx` / `_16k_op15` — zagnieżdżone `If` LSTM. Wariant „ifless” ma jeden górny `If`:
  zastępujemy go gałęzią 16 kHz (`voice-vad-impl/src/inline.rs`), usuwamy wejście `sr`.
- Adnotacje kształtów z eksportu używają symbolu `batch` → konflikt z konkretnym `[1, 576]`; czyścimy `value_info`
  i typy wyjść grafu — `tract` wyprowadza kształty sam.
- Błędy `tract` (`anyhow`) — `Debug` (`{e:?}`) pokazuje łańcuch przyczyn (który węzeł, która reguła).
- `ort` odrzucony: pobiera binaria ONNX Runtime przy budowie (zablokowane w CI, łańcuch dostaw).
