//! Wspólne: ładowanie grafu ONNX przez `tract-onnx` (czysty Rust, bez binariów ONNX Runtime;
//! ADR 0004 jak `voice-vad-impl`) ze stałym kształtem wejścia i uruchomienie → płaskie wyjście `f32`.

use std::sync::Arc;

use tract_onnx::prelude::*;
use voice_wake_contract::WakeError;

pub(crate) fn err(e: impl std::fmt::Debug) -> WakeError {
    // `Debug` błędów `tract` zawiera łańcuch przyczyn (który węzeł i dlaczego).
    WakeError::Model(format!("{e:?}"))
}

/// Zoptymalizowany graf z jednym wejściem `f32` o stałym kształcie.
pub(crate) struct Plan {
    plan: Arc<TypedRunnableModel>,
    shape: Vec<usize>,
}

impl Plan {
    /// Ładuje graf z bajtów (już zweryfikowanych hashem) i ustala kształt wejścia nr 0.
    pub(crate) fn load(bytes: &[u8], shape: &[usize]) -> Result<Self, WakeError> {
        let model = tract_onnx::onnx()
            .model_for_read(&mut &bytes[..])
            .map_err(err)?
            .with_input_fact(0, f32::fact(shape).into())
            .map_err(err)?;
        let plan = model
            .into_optimized()
            .map_err(err)?
            .into_runnable()
            .map_err(err)?;
        Ok(Self {
            plan,
            shape: shape.to_vec(),
        })
    }

    /// Uruchamia graf; zwraca pierwsze wyjście spłaszczone.
    pub(crate) fn run(&self, data: &[f32]) -> Result<Vec<f32>, WakeError> {
        let x = Tensor::from_shape(&self.shape, data).map_err(err)?;
        let out = self.plan.run(tvec!(x.into_tvalue())).map_err(err)?;
        let first = out.first().ok_or_else(|| err("brak wyjścia modelu"))?;
        let view = first.to_plain_array_view::<f32>().map_err(err)?;
        Ok(view.iter().copied().collect())
    }
}

/// Sigmoid.
pub(crate) fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Softmax (stabilny numerycznie).
pub(crate) fn softmax(x: &[f32]) -> Vec<f32> {
    let max = x.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let e: Vec<f32> = x.iter().map(|v| (v - max).exp()).collect();
    let sum: f32 = e.iter().sum::<f32>().max(f32::MIN_POSITIVE);
    e.iter().map(|v| v / sum).collect()
}
