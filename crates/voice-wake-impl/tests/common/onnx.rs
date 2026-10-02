//! Budowa małych grafów ONNX w testach (bez pobierania modeli): wejście/wyjście `f32`,
//! inicjalizatory, węzły z atrybutami; zapis do pliku z SHA-256 dla manifestu.

#![allow(dead_code)]

use prost::Message as _;
use tract_onnx::pb::tensor_shape_proto::{Dimension, dimension};
use tract_onnx::pb::type_proto::{Tensor as TensorType, Value as TypeValue};
use tract_onnx::pb::{
    AttributeProto, GraphProto, ModelProto, NodeProto, OperatorSetIdProto, TensorProto,
    TensorShapeProto, TypeProto, ValueInfoProto,
};

const FLOAT: i32 = 1;
const INT64: i32 = 7;

pub fn value(name: &str, dims: &[i64]) -> ValueInfoProto {
    ValueInfoProto {
        name: name.into(),
        r#type: Some(TypeProto {
            value: Some(TypeValue::TensorType(TensorType {
                elem_type: FLOAT,
                shape: Some(TensorShapeProto {
                    dim: dims
                        .iter()
                        .map(|d| Dimension {
                            value: Some(dimension::Value::DimValue(*d)),
                            ..Default::default()
                        })
                        .collect(),
                }),
            })),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn floats(name: &str, dims: &[i64], data: Vec<f32>) -> TensorProto {
    TensorProto {
        name: name.into(),
        dims: dims.to_vec(),
        data_type: FLOAT,
        float_data: data,
        ..Default::default()
    }
}

pub fn int64s(name: &str, data: Vec<i64>) -> TensorProto {
    TensorProto {
        name: name.into(),
        dims: vec![data.len() as i64],
        data_type: INT64,
        int64_data: data,
        ..Default::default()
    }
}

pub fn ints_attr(name: &str, v: &[i64]) -> AttributeProto {
    AttributeProto {
        name: name.into(),
        r#type: 7,
        ints: v.to_vec(),
        ..Default::default()
    }
}

pub fn int_attr(name: &str, v: i64) -> AttributeProto {
    AttributeProto {
        name: name.into(),
        r#type: 2,
        i: v,
        ..Default::default()
    }
}

pub fn node(op: &str, inputs: &[&str], output: &str, attrs: Vec<AttributeProto>) -> NodeProto {
    NodeProto {
        op_type: op.into(),
        name: format!("{op}_{output}"),
        input: inputs.iter().map(|s| (*s).to_owned()).collect(),
        output: vec![output.into()],
        attribute: attrs,
        ..Default::default()
    }
}

pub fn model(
    input: ValueInfoProto,
    output: ValueInfoProto,
    nodes: Vec<NodeProto>,
    init: Vec<TensorProto>,
) -> Vec<u8> {
    ModelProto {
        ir_version: 8,
        opset_import: vec![OperatorSetIdProto {
            domain: String::new(),
            version: 13,
        }],
        producer_name: "alfa-test".into(),
        graph: Some(GraphProto {
            name: "g".into(),
            node: nodes,
            initializer: init,
            input: vec![input],
            output: vec![output],
            ..Default::default()
        }),
        ..Default::default()
    }
    .encode_to_vec()
}
