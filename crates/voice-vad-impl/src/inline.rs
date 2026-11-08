//! Przepisanie grafu ONNX Silero przed `tract`: górny węzeł `If(sr == 16000)` zastępujemy gałęzią
//! 16 kHz (z odwzorowaniem wyjść przez `Identity`) i usuwamy wejście `sr`.
//!
//! Powód: `tract` analizuje obie gałęzie `If`, a gałąź 8 kHz nie typuje się dla okna 16 kHz; do tego
//! gałęzie Silero odwołują się do inicjalizatorów zewnętrznego grafu (przechwycenia niejawne).
//! Wariant `silero_vad_op18_ifless.onnx` ma tylko ten jeden `If` (bez zagnieżdżeń) — po przepisaniu
//! jest zwykłym grafem (Conv, Gemm-owy LSTM, Sigmoid).

use std::collections::HashSet;

use tract_onnx::pb::{GraphProto, ModelProto, NodeProto, TensorProto};

/// Częstotliwość, której gałąź wybieramy.
const TARGET_SR: i64 = 16_000;

fn int64_value(t: &TensorProto) -> Option<i64> {
    if let Some(v) = t.int64_data.first() {
        return Some(*v);
    }
    let raw: [u8; 8] = t.raw_data.get(..8)?.try_into().ok()?;
    Some(i64::from_le_bytes(raw))
}

/// Czy `cond` = `Equal(…, stała 16000)` (stała jako inicjalizator albo węzeł `Constant`).
fn compares_to_target(graph: &GraphProto, cond: &str) -> Option<bool> {
    let eq = graph
        .node
        .iter()
        .find(|n| n.op_type == "Equal" && n.output.iter().any(|o| o == cond))?;
    eq.input.iter().find_map(|name| {
        let from_init = graph
            .initializer
            .iter()
            .find(|t| &t.name == name)
            .and_then(int64_value);
        let from_const = graph
            .node
            .iter()
            .find(|n| n.op_type == "Constant" && n.output.iter().any(|o| o == name))
            .and_then(|n| {
                n.attribute
                    .iter()
                    .find_map(|a| a.t.as_ref().and_then(int64_value))
            });
        from_init.or(from_const).map(|v| v == TARGET_SR)
    })
}

/// Usuwa węzły, których wyjścia nikt nie używa (iteracyjnie).
fn prune(graph: &mut GraphProto) {
    loop {
        let used: HashSet<String> = graph
            .node
            .iter()
            .flat_map(|n| n.input.iter().cloned())
            .chain(graph.output.iter().map(|o| o.name.clone()))
            .collect();
        let before = graph.node.len();
        graph
            .node
            .retain(|n| n.output.iter().any(|o| used.contains(o)));
        if graph.node.len() == before {
            break;
        }
    }
    let used: HashSet<String> = graph
        .node
        .iter()
        .flat_map(|n| n.input.iter().cloned())
        .collect();
    graph
        .input
        .retain(|i| used.contains(&i.name) || graph.output.iter().any(|o| o.name == i.name));
}

/// Zastępuje górny `If` zależny od `sr` gałęzią 16 kHz. Zwraca `Ok(true)`, gdy przepisano.
pub fn inline_sample_rate_if(model: &mut ModelProto) -> Result<bool, String> {
    let graph = model.graph.as_mut().ok_or("model bez grafu")?;
    let Some(pos) = graph.node.iter().position(|n| n.op_type == "If") else {
        return Ok(false);
    };
    let node = graph.node[pos].clone();
    let cond = node.input.first().ok_or("If bez warunku")?;
    let is_then =
        compares_to_target(graph, cond).ok_or("warunek If nie jest porównaniem sr z 16000")?;
    let branch_name = if is_then {
        "then_branch"
    } else {
        "else_branch"
    };
    let branch = node
        .attribute
        .iter()
        .find(|a| a.name == branch_name)
        .and_then(|a| a.g.clone())
        .ok_or("brak gałęzi If")?;
    if branch.node.iter().any(|n| n.op_type == "If") {
        return Err(
            "gałąź 16 kHz zawiera zagnieżdżone If — użyj silero_vad_op18_ifless.onnx".into(),
        );
    }
    if branch.output.len() != node.output.len() {
        return Err("liczba wyjść gałęzi ≠ liczba wyjść If".into());
    }
    let mut nodes: Vec<NodeProto> = graph.node[..pos].to_vec();
    nodes.extend(branch.node);
    for (i, (from, to)) in branch.output.iter().zip(&node.output).enumerate() {
        nodes.push(NodeProto {
            input: vec![from.name.clone()],
            output: vec![to.clone()],
            name: format!("alfa_inline_identity_{i}"),
            op_type: "Identity".into(),
            ..NodeProto::default()
        });
    }
    nodes.extend(graph.node[pos + 1..].iter().cloned());
    graph.node = nodes;
    graph.initializer.extend(branch.initializer);
    // Adnotacje kształtów z eksportu używają symbolu `batch`, który koliduje z konkretnym
    // kształtem wejścia [1, 576] — `tract` wyprowadzi kształty sam.
    graph.value_info.clear();
    for out in &mut graph.output {
        out.r#type = None;
    }
    prune(graph);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tract_onnx::pb::{AttributeProto, ValueInfoProto};

    fn vi(name: &str) -> ValueInfoProto {
        ValueInfoProto {
            name: name.into(),
            ..ValueInfoProto::default()
        }
    }

    fn node(op: &str, inputs: &[&str], outputs: &[&str]) -> NodeProto {
        NodeProto {
            input: inputs.iter().map(|s| (*s).into()).collect(),
            output: outputs.iter().map(|s| (*s).into()).collect(),
            op_type: op.into(),
            ..NodeProto::default()
        }
    }

    fn graph(branch_out: &str, inner: Vec<NodeProto>) -> GraphProto {
        GraphProto {
            node: inner,
            output: vec![vi(branch_out)],
            ..GraphProto::default()
        }
    }

    #[test]
    fn replaces_if_with_16k_branch_and_drops_sr() {
        let sixteen = TensorProto {
            name: "c".into(),
            int64_data: vec![16_000],
            ..TensorProto::default()
        };
        let then_g = graph("t_out", vec![node("Relu", &["input"], &["t_out"])]);
        let else_g = graph("e_out", vec![node("Sigmoid", &["input"], &["e_out"])]);
        let mut iff = node("If", &["cond"], &["output"]);
        iff.attribute = vec![
            AttributeProto {
                name: "then_branch".into(),
                g: Some(then_g),
                ..AttributeProto::default()
            },
            AttributeProto {
                name: "else_branch".into(),
                g: Some(else_g),
                ..AttributeProto::default()
            },
        ];
        let mut model = ModelProto {
            graph: Some(GraphProto {
                node: vec![node("Equal", &["sr", "c"], &["cond"]), iff],
                initializer: vec![sixteen],
                input: vec![vi("input"), vi("sr")],
                output: vec![vi("output")],
                ..GraphProto::default()
            }),
            ..ModelProto::default()
        };
        assert_eq!(inline_sample_rate_if(&mut model), Ok(true));
        let g = model.graph.unwrap();
        let ops: Vec<&str> = g.node.iter().map(|n| n.op_type.as_str()).collect();
        assert_eq!(ops, vec!["Relu", "Identity"]);
        assert_eq!(
            g.input.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            vec!["input"]
        );
        let mut plain = ModelProto {
            graph: Some(GraphProto::default()),
            ..ModelProto::default()
        };
        assert_eq!(inline_sample_rate_if(&mut plain), Ok(false));
        assert!(inline_sample_rate_if(&mut ModelProto::default()).is_err());
        let raw = TensorProto {
            raw_data: 16_000i64.to_le_bytes().to_vec(),
            ..TensorProto::default()
        };
        assert_eq!(int64_value(&raw), Some(16_000));
    }
}
