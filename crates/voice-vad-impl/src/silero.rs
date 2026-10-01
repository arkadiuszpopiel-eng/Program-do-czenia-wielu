//! Silero VAD (ONNX, MIT) przez `tract-onnx` (czysty Rust — bez binariów ONNX Runtime):
//! okna 512 próbek @ 16 kHz + 64 próbki kontekstu z poprzedniego okna (jak w `OnnxWrapper` v5/v6),
//! stan RNN `[2, 1, 128]` przenoszony między oknami. Model tylko ze znanym hashem SHA-256.

use std::path::Path;
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tract_onnx::prelude::*;
use voice_vad_contract::{SILERO_WINDOW, VadError};

/// Kontekst doklejany przed oknem (16 kHz).
pub const CONTEXT: usize = 64;

/// Znane modele (pakiet PyPI `silero-vad` 6.2.3, `silero_vad/data/`): (nazwa, SHA-256).
/// Obsługiwany przez `tract`: wariant „ifless” (po przepisaniu górnego `If`, patrz `inline`);
/// `silero_vad.onnx` i `silero_vad_16k_op15.onnx` mają zagnieżdżone `If` (LSTM z PyTorch).
pub const KNOWN_MODELS: [(&str, &str); 1] = [(
    "silero_vad_op18_ifless.onnx (6.2.3)",
    "7671cd04b004e9076da0d4a7b1a5aec36adf161c39230c1cb94a4fd5db6bbd28",
)];

/// Polityka weryfikacji pliku modelu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashPolicy {
    /// Tylko modele z [`KNOWN_MODELS`] (produkcja).
    KnownOnly,
    /// Dowolny plik (Voice Lab, nowe wersje przed dopisaniem hasha).
    AllowUnverified,
}

fn err(e: impl std::fmt::Debug) -> VadError {
    // `Debug` błędów `tract` zawiera cały łańcuch przyczyn (który węzeł i dlaczego).
    VadError::Model(format!("{e:?}"))
}

/// SHA-256 pliku (hex).
pub fn sha256_file(path: &Path) -> Result<String, VadError> {
    let bytes = std::fs::read(path).map_err(|e| err(format!("{}: {e}", path.display())))?;
    Ok(Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Załadowany model ze stanem strumienia.
pub struct SileroModel {
    plan: Arc<TypedRunnableModel>,
    order: [Option<usize>; 3],
    state: Tensor,
    context: [f32; CONTEXT],
    input: Vec<f32>,
    name: String,
}

impl std::fmt::Debug for SileroModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SileroModel")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl SileroModel {
    /// Ładuje i optymalizuje model (sprawdzając hash wg `policy`).
    pub fn load(path: &Path, policy: HashPolicy) -> Result<Self, VadError> {
        let hash = sha256_file(path)?;
        let known = KNOWN_MODELS.iter().find(|(_, h)| *h == hash);
        if known.is_none() && policy == HashPolicy::KnownOnly {
            return Err(err(format!(
                "nieznany hash modelu {hash} ({})",
                path.display()
            )));
        }
        let onnx = tract_onnx::onnx();
        let mut proto = onnx.proto_model_for_path(path).map_err(err)?;
        crate::inline::inline_sample_rate_if(&mut proto).map_err(err)?;
        let mut model = onnx.model_for_proto_model(&proto).map_err(err)?;
        // Kolejność wejść różni się między wariantami eksportu — mapujemy po nazwach.
        let names: Vec<String> = model
            .input_outlets()
            .map_err(err)?
            .iter()
            .map(|o| model.node(o.node).name.clone())
            .collect();
        let idx = |n: &str| {
            names
                .iter()
                .position(|x| x == n)
                .ok_or_else(|| err(format!("brak wejścia `{n}` w {names:?}")))
        };
        let (input_ix, state_ix) = (idx("input")?, idx("state")?);
        let order = [Some(input_ix), Some(state_ix), idx("sr").ok()];
        model = model
            .with_input_fact(input_ix, f32::fact([1, SILERO_WINDOW + CONTEXT]).into())
            .map_err(err)?
            .with_input_fact(state_ix, f32::fact([2, 1, 128]).into())
            .map_err(err)?;
        if let Some(sr) = order[2] {
            model = model
                .with_input_fact(sr, InferenceFact::from(tensor0(16_000i64)))
                .map_err(err)?;
        }
        let plan = model
            .into_optimized()
            .map_err(err)?
            .into_runnable()
            .map_err(err)?;
        Ok(Self {
            plan,
            order,
            state: Tensor::zero::<f32>(&[2, 1, 128]).map_err(err)?,
            context: [0.0; CONTEXT],
            input: Vec::with_capacity(SILERO_WINDOW + CONTEXT),
            name: known.map_or_else(
                || "silero_vad (niezweryfikowany)".to_owned(),
                |(n, _)| (*n).to_owned(),
            ),
        })
    }

    /// Nazwa (z listy znanych modeli).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Prawdopodobieństwo mowy w oknie 512 próbek.
    pub fn infer(&mut self, window: &[f32]) -> Result<f32, VadError> {
        if window.len() != SILERO_WINDOW {
            return Err(err(format!("okno {} ≠ {SILERO_WINDOW}", window.len())));
        }
        self.input.clear();
        self.input.extend_from_slice(&self.context);
        self.input.extend_from_slice(window);
        self.context
            .copy_from_slice(&window[SILERO_WINDOW - CONTEXT..]);
        let x = Tensor::from_shape(&[1, SILERO_WINDOW + CONTEXT], &self.input).map_err(err)?;
        let sr = tensor0(16_000i64);
        let mut inputs: [Option<TValue>; 3] = [None, None, None];
        let values = [
            x.into_tvalue(),
            self.state.clone().into_tvalue(),
            sr.into_tvalue(),
        ];
        for (slot, value) in self.order.iter().zip(values) {
            if let Some(i) = slot {
                inputs[*i] = Some(value);
            }
        }
        let inputs: TVec<TValue> = inputs.into_iter().flatten().collect();
        let out = self.plan.run(inputs).map_err(err)?;
        let prob: f32 = out
            .first()
            .and_then(|t| {
                t.to_plain_array_view::<f32>()
                    .ok()
                    .and_then(|v| v.iter().next().copied())
            })
            .ok_or_else(|| err("brak wyjścia `output`"))?;
        if let Some(state) = out.get(1) {
            self.state = state.clone().into_tensor();
        }
        Ok(prob.clamp(0.0, 1.0))
    }

    /// Reset stanu RNN i kontekstu.
    pub fn reset(&mut self) {
        if let Ok(z) = Tensor::zero::<f32>(&[2, 1, 128]) {
            self.state = z;
        }
        self.context = [0.0; CONTEXT];
    }
}
