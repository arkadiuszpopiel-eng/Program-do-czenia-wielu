//! Budowa zabawkowego enkodera ONNX (protobuf przez `prost`, typy z `tract_onnx::pb`): osadzenia
//! tokenów i pozycji (`Slice` po długości sekwencji — jak eksporty `optimum`), `LayerNormalization`,
//! `MatMul` + `Tanh`, kontekst = średnia po masce dodana do każdego tokenu (wynik zależy od maski
//! i wypełnienia), drugie wyjście `pooled` = sam kontekst `[B, D]`.

use prost::Message as _;
use tract_onnx::pb::tensor_shape_proto::{Dimension, dimension};
use tract_onnx::pb::type_proto::{Tensor as TensorType, Value as TypeValue};
use tract_onnx::pb::{
    AttributeProto, GraphProto, ModelProto, NodeProto, OperatorSetIdProto, TensorProto,
    TensorShapeProto, TypeProto, ValueInfoProto,
};

pub(super) const FLOAT: i32 = 1;
pub(super) const INT64: i32 = 7;

/// Maksymalna długość sekwencji zabawkowego modelu (tablica pozycji).
pub const TOY_MAX_POS: usize = 64;

pub(super) fn value(name: &str, elem_type: i32, dims: &[&str]) -> ValueInfoProto {
    let dim = dims
        .iter()
        .map(|d| Dimension {
            value: Some(match d.parse::<i64>() {
                Ok(n) => dimension::Value::DimValue(n),
                Err(_) => dimension::Value::DimParam((*d).to_owned()),
            }),
            ..Default::default()
        })
        .collect();
    ValueInfoProto {
        name: name.into(),
        r#type: Some(TypeProto {
            value: Some(TypeValue::TensorType(TensorType {
                elem_type,
                shape: Some(TensorShapeProto { dim }),
            })),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub(super) fn floats(name: &str, dims: &[usize], data: Vec<f32>) -> TensorProto {
    TensorProto {
        name: name.into(),
        dims: dims
            .iter()
            .map(|d| i64::try_from(*d).unwrap_or(0))
            .collect(),
        data_type: FLOAT,
        float_data: data,
        ..Default::default()
    }
}

pub(super) fn int64s(name: &str, dims: &[i64], data: Vec<i64>) -> TensorProto {
    TensorProto {
        name: name.into(),
        dims: dims.to_vec(),
        data_type: INT64,
        int64_data: data,
        ..Default::default()
    }
}

pub(super) fn attr_int(name: &str, i: i64) -> AttributeProto {
    AttributeProto {
        name: name.into(),
        r#type: 2,
        i,
        ..Default::default()
    }
}

pub(super) fn attr_float(name: &str, f: f32) -> AttributeProto {
    AttributeProto {
        name: name.into(),
        r#type: 1,
        f,
        ..Default::default()
    }
}

pub(super) fn node(
    op: &str,
    inputs: &[&str],
    output: &str,
    attribute: Vec<AttributeProto>,
) -> NodeProto {
    NodeProto {
        op_type: op.into(),
        name: format!("{op}_{output}"),
        input: inputs.iter().map(|s| (*s).to_owned()).collect(),
        output: vec![output.into()],
        attribute,
        ..Default::default()
    }
}

/// Deterministyczne wagi z `[-scale, scale]` (generator liniowy kongruencyjny z ziarnem).
pub fn weights(seed: u64, n: usize, scale: f32) -> Vec<f32> {
    let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    (0..n)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let unit = f32::from(u16::try_from(state >> 48).unwrap_or(0)) / f32::from(u16::MAX);
            (unit * 2.0 - 1.0) * scale
        })
        .collect()
}

/// Bajty modelu: słownik `vocab`, wymiar `dims`, opcjonalne wejście `token_type_ids`.
pub fn toy_model_bytes(vocab: usize, dims: usize, type_ids: bool) -> Vec<u8> {
    let mut init = vec![
        floats("E", &[vocab, dims], weights(1, vocab * dims, 1.0)),
        floats(
            "P",
            &[TOY_MAX_POS, dims],
            weights(2, TOY_MAX_POS * dims, 0.1),
        ),
        floats("W", &[dims, dims], weights(3, dims * dims, 0.5)),
        floats("bias", &[dims], weights(4, dims, 0.1)),
        floats("gamma", &[dims], vec![1.0; dims]),
        floats("beta", &[dims], vec![0.0; dims]),
        floats("half", &[], vec![0.5]),
        int64s("one", &[], vec![1]),
        int64s("zero1", &[1], vec![0]),
        int64s("axis0", &[1], vec![0]),
        int64s("axis1", &[1], vec![1]),
        int64s("axis2", &[1], vec![2]),
    ];
    let mut nodes = vec![
        node("Gather", &["E", "input_ids"], "tok", vec![]),
        node("Shape", &["input_ids"], "shape", vec![]),
        node(
            "Gather",
            &["shape", "one"],
            "seqlen",
            vec![attr_int("axis", 0)],
        ),
        node("Unsqueeze", &["seqlen", "axis0"], "seqlen1", vec![]),
        node("Slice", &["P", "zero1", "seqlen1", "axis0"], "pos", vec![]),
        node("Add", &["tok", "pos"], "h0", vec![]),
    ];
    let mut inputs = vec![
        value("input_ids", INT64, &["batch_size", "sequence_length"]),
        value("attention_mask", INT64, &["batch_size", "sequence_length"]),
    ];
    let mut embedded = "h0";
    if type_ids {
        init.push(floats("T", &[2, dims], weights(5, 2 * dims, 0.3)));
        nodes.push(node("Gather", &["T", "token_type_ids"], "ty", vec![]));
        nodes.push(node("Add", &["h0", "ty"], "h0t", vec![]));
        inputs.push(value(
            "token_type_ids",
            INT64,
            &["batch_size", "sequence_length"],
        ));
        embedded = "h0t";
    }
    nodes.extend([
        node(
            "LayerNormalization",
            &[embedded, "gamma", "beta"],
            "ln",
            vec![attr_int("axis", -1), attr_float("epsilon", 1e-5)],
        ),
        node("MatMul", &["ln", "W"], "mm", vec![]),
        node("Add", &["mm", "bias"], "h1", vec![]),
        node("Tanh", &["h1"], "h2", vec![]),
        node(
            "Cast",
            &["attention_mask"],
            "maskf",
            vec![attr_int("to", 1)],
        ),
        node("Unsqueeze", &["maskf", "axis2"], "m3", vec![]),
        node("Mul", &["h2", "m3"], "hm", vec![]),
        node(
            "ReduceSum",
            &["hm", "axis1"],
            "sum",
            vec![attr_int("keepdims", 1)],
        ),
        node(
            "ReduceSum",
            &["m3", "axis1"],
            "cnt",
            vec![attr_int("keepdims", 1)],
        ),
        node("Div", &["sum", "cnt"], "ctx", vec![]),
        node("Mul", &["ctx", "half"], "ctxh", vec![]),
        node("Add", &["h2", "ctxh"], "last_hidden_state", vec![]),
        node("Squeeze", &["ctx", "axis1"], "pooled", vec![]),
    ]);
    let dims_s = dims.to_string();
    ModelProto {
        ir_version: 8,
        opset_import: vec![OperatorSetIdProto {
            domain: String::new(),
            version: 17,
        }],
        producer_name: "alfa-lib-embed-testkit".into(),
        graph: Some(GraphProto {
            name: "toy-encoder".into(),
            node: nodes,
            initializer: init,
            input: inputs,
            output: vec![
                value(
                    "last_hidden_state",
                    FLOAT,
                    &["batch_size", "sequence_length", &dims_s],
                ),
                value("pooled", FLOAT, &["batch_size", &dims_s]),
            ],
            ..Default::default()
        }),
        ..Default::default()
    }
    .encode_to_vec()
}
