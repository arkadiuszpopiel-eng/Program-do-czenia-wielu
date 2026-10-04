//! Model ONNX enkodera (BERT/XLM-R) przez `tract-onnx` (czysty Rust, ADR 0004): wejścia
//! `input_ids` + opcjonalnie `attention_mask` i `token_type_ids` (`int64 [B, S]`, wymiary
//! symboliczne — jeden plan dla wszystkich długości), jedno wyjście `f32` (`[B, S, D]` albo `[B, D]`).

use std::sync::Arc;

use tract_onnx::prelude::*;

use crate::error::EmbedError;

fn err(e: impl std::fmt::Debug) -> EmbedError {
    // `Debug` błędów `tract` zawiera łańcuch przyczyn (który węzeł, która reguła).
    EmbedError::Model(format!("{e:?}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Input {
    Ids,
    Mask,
    TypeIds,
}

/// Wyjście przebiegu: dane i kształt.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    /// Dane (kolejność wierszowa).
    pub data: Vec<f32>,
    /// Kształt (`[B, S, D]` albo `[B, D]`).
    pub shape: Vec<usize>,
}

/// Zoptymalizowany plan modelu.
pub struct OnnxModel {
    plan: Arc<TypedRunnableModel>,
    inputs: Vec<Input>,
}

impl std::fmt::Debug for OnnxModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnnxModel")
            .field("inputs", &self.inputs)
            .finish_non_exhaustive()
    }
}

impl OnnxModel {
    /// Ładuje model z bajtów (już zweryfikowanych SHA-256); `output` = nazwa wyjścia (domyślnie
    /// pierwsze). Bajty są zwalniane zaraz po dekodowaniu protobuf (szczyt RAM ≈ 2× rozmiar pliku).
    pub fn from_bytes(bytes: Vec<u8>, output: Option<&str>) -> Result<Self, EmbedError> {
        let onnx = tract_onnx::onnx();
        let mut proto = onnx.proto_model_for_read(&mut &bytes[..]).map_err(err)?;
        drop(bytes);
        // Adnotacje kształtów z eksportu (`batch_size`, `sequence_length`) kłóciłyby się z naszymi
        // symbolami — `tract` wyprowadza kształty sam (docs/vendor/tract-onnx.md).
        if let Some(graph) = proto.graph.as_mut() {
            graph.value_info.clear();
            for out in &mut graph.output {
                out.r#type = None;
            }
        }
        let mut model = onnx.model_for_proto_model(&proto).map_err(err)?;
        drop(proto);
        match output {
            Some(name) => model.select_outputs_by_name([name]).map_err(err)?,
            None => {
                let first = *model
                    .output_outlets()
                    .map_err(err)?
                    .first()
                    .ok_or_else(|| err("model bez wyjść"))?;
                model.select_output_outlets(&[first]).map_err(err)?;
            }
        }
        let names: Vec<String> = model
            .input_outlets()
            .map_err(err)?
            .iter()
            .map(|o| model.node(o.node).name.clone())
            .collect();
        let mut inputs = Vec::with_capacity(names.len());
        for name in &names {
            inputs.push(match name.as_str() {
                "input_ids" => Input::Ids,
                "attention_mask" => Input::Mask,
                "token_type_ids" => Input::TypeIds,
                other => return Err(err(format!("nieobsługiwane wejście `{other}` w {names:?}"))),
            });
        }
        if !inputs.contains(&Input::Ids) {
            return Err(err(format!("brak wejścia `input_ids` w {names:?}")));
        }
        let batch = model.symbols.sym("B");
        let seq = model.symbols.sym("S");
        for ix in 0..inputs.len() {
            let fact =
                InferenceFact::dt_shape(i64::datum_type(), tvec![batch.to_dim(), seq.to_dim()]);
            model = model.with_input_fact(ix, fact).map_err(err)?;
        }
        let plan = model
            .into_optimized()
            .map_err(err)?
            .into_runnable()
            .map_err(err)?;
        Ok(Self { plan, inputs })
    }

    /// Przebieg dla wsadu `batch × seq` (`ids`, `mask` wierszowo; `token_type_ids` = 0).
    pub fn run(
        &self,
        ids: &[i64],
        mask: &[i64],
        batch: usize,
        seq: usize,
    ) -> Result<Output, EmbedError> {
        if ids.len() != batch * seq || mask.len() != batch * seq {
            return Err(err("rozmiar wsadu niezgodny z kształtem"));
        }
        let shape = [batch, seq];
        let mut values: TVec<TValue> = TVec::new();
        for input in &self.inputs {
            let tensor = match input {
                Input::Ids => Tensor::from_shape(&shape, ids),
                Input::Mask => Tensor::from_shape(&shape, mask),
                Input::TypeIds => Tensor::zero::<i64>(&shape),
            }
            .map_err(err)?;
            values.push(tensor.into_tvalue());
        }
        let out = self.plan.run(values).map_err(err)?;
        let first = out.first().ok_or_else(|| err("brak wyjścia"))?;
        let view = first.to_plain_array_view::<f32>().map_err(err)?;
        Ok(Output {
            shape: view.shape().to_vec(),
            data: view.iter().copied().collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_is_rejected() {
        assert!(OnnxModel::from_bytes(b"to nie jest onnx".to_vec(), None).is_err());
        assert!(OnnxModel::from_bytes(Vec::new(), None).is_err());
    }
}
