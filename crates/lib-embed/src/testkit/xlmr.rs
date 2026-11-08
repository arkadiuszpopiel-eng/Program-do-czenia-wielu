//! Jednowarstwowy enkoder o budowie eksportu HF `XLMRobertaModel` (`torch.onnx.export`, opset 14):
//! identyfikatory pozycji z `CumSum` po masce wypełnienia (`padding_idx = 1`), `ConstantOfShape`
//! dla `token_type_ids`, rozłożona `LayerNorm` (`ReduceMean`/`Pow`/`Sqrt`), wielogłowicowa uwaga
//! z kształtami liczonymi w grafie (`Shape` → `Gather` → `Concat` → `Reshape`), maska rozszerzona
//! `(1 − m) · −10000`, `Softmax`, GELU przez `Erf`; wyjścia `last_hidden_state` i `pooler_output`.
//! Sprawdza, że `tract` wykonuje operatory i wymiary symboliczne prawdziwych eksportów E5/MiniLM.

use prost::Message as _;
use tract_onnx::pb::{
    AttributeProto, GraphProto, ModelProto, NodeProto, OperatorSetIdProto, TensorProto,
};

use super::onnx::{FLOAT, INT64, attr_int, floats, int64s, node, value, weights};

/// Wymiar enkodera.
pub const XLMR_DIMS: usize = 64;
/// Głowice uwagi.
pub const XLMR_HEADS: usize = 4;
/// Warstwa ukryta FFN.
pub const XLMR_FFN: usize = 128;
/// Tablica pozycji (`max_tokens + 2`, pozycje od `padding_idx + 1`).
pub const XLMR_MAX_POS: usize = 66;

fn attr_ints(name: &str, ints: &[i64]) -> AttributeProto {
    AttributeProto {
        name: name.into(),
        r#type: 7,
        ints: ints.to_vec(),
        ..Default::default()
    }
}

fn attr_tensor_i64(name: &str, v: i64) -> AttributeProto {
    AttributeProto {
        name: name.into(),
        r#type: 4,
        t: Some(int64s("value", &[1], vec![v])),
        ..Default::default()
    }
}

struct Graph {
    nodes: Vec<NodeProto>,
}

impl Graph {
    fn op(&mut self, op: &str, inputs: &[&str], out: &str, attrs: Vec<AttributeProto>) {
        self.nodes.push(node(op, inputs, out, attrs));
    }

    /// `LayerNorm` rozłożona jak w eksporcie opset 14.
    fn layer_norm(&mut self, x: &str, gamma: &str, beta: &str, out: &str) {
        let n = |s: &str| format!("{out}_{s}");
        self.op(
            "ReduceMean",
            &[x],
            &n("mean"),
            vec![attr_ints("axes", &[-1])],
        );
        self.op("Sub", &[x, &n("mean")], &n("d"), vec![]);
        self.op("Pow", &[&n("d"), "two"], &n("sq"), vec![]);
        self.op(
            "ReduceMean",
            &[&n("sq")],
            &n("var"),
            vec![attr_ints("axes", &[-1])],
        );
        self.op("Add", &[&n("var"), "eps"], &n("ve"), vec![]);
        self.op("Sqrt", &[&n("ve")], &n("sd"), vec![]);
        self.op("Div", &[&n("d"), &n("sd")], &n("norm"), vec![]);
        self.op("Mul", &[&n("norm"), gamma], &n("scaled"), vec![]);
        self.op("Add", &[&n("scaled"), beta], out, vec![]);
    }

    /// `x · W + b`.
    fn linear(&mut self, x: &str, w: &str, b: &str, out: &str) {
        let mm = format!("{out}_mm");
        self.op("MatMul", &[x, w], &mm, vec![]);
        self.op("Add", &[&mm, b], out, vec![]);
    }

    /// Projekcja na głowice: `[B, S, D]` → `[B, H, S, Dh]` (albo `[B, H, Dh, S]` dla kluczy).
    fn heads(&mut self, x: &str, w: &str, b: &str, perm: &[i64], out: &str) {
        let (lin, shaped) = (format!("{out}_lin"), format!("{out}_r"));
        self.linear(x, w, b, &lin);
        self.op("Reshape", &[&lin, "shp4"], &shaped, vec![]);
        self.op("Transpose", &[&shaped], out, vec![attr_ints("perm", perm)]);
    }
}

fn embeddings(g: &mut Graph) {
    g.op("Equal", &["input_ids", "pad"], "eq", vec![]);
    g.op("Not", &["eq"], "ne", vec![]);
    g.op("Cast", &["ne"], "m32", vec![attr_int("to", 6)]);
    g.op("CumSum", &["m32", "axis1"], "cs", vec![]);
    g.op("Mul", &["cs", "m32"], "inc", vec![]);
    g.op("Cast", &["inc"], "inc64", vec![attr_int("to", 7)]);
    g.op("Add", &["inc64", "pad"], "pos_ids", vec![]);
    g.op("Gather", &["E", "input_ids"], "we", vec![]);
    g.op("Gather", &["P", "pos_ids"], "pe", vec![]);
    g.op("Shape", &["input_ids"], "ids_shape", vec![]);
    let zero = vec![attr_tensor_i64("value", 0)];
    g.op("ConstantOfShape", &["ids_shape"], "type_ids", zero);
    g.op("Gather", &["T", "type_ids"], "te", vec![]);
    g.op("Add", &["we", "pe"], "e1", vec![]);
    g.op("Add", &["e1", "te"], "e2", vec![]);
    g.layer_norm("e2", "ln_e_g", "ln_e_b", "h0");
}

/// Rozmiary enkodera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XlmrShape {
    /// Słownik.
    pub vocab: usize,
    /// Wymiar.
    pub dims: usize,
    /// Głowice uwagi.
    pub heads: usize,
    /// Warstwa ukryta FFN.
    pub ffn: usize,
    /// Warstwy.
    pub layers: usize,
    /// Tablica pozycji.
    pub max_pos: usize,
}

impl XlmrShape {
    /// Mały model testów zgodności (1 warstwa, 64 wym.).
    pub fn tiny(vocab: usize) -> Self {
        Self {
            vocab,
            dims: XLMR_DIMS,
            heads: XLMR_HEADS,
            ffn: XLMR_FFN,
            layers: 1,
            max_pos: XLMR_MAX_POS,
        }
    }

    /// Kształt `multilingual-e5-small` (losowe wagi — pomiar czasu i RAM bez pobierania modelu).
    pub fn e5_small() -> Self {
        Self {
            vocab: 250_002,
            dims: 384,
            heads: 12,
            ffn: 1536,
            layers: 12,
            max_pos: 514,
        }
    }
}

/// Maska rozszerzona i kształty głowic (wspólne dla warstw).
fn shared(g: &mut Graph) {
    g.op("Unsqueeze", &["attention_mask", "ax12"], "m4", vec![]);
    g.op("Cast", &["m4"], "m4f", vec![attr_int("to", 1)]);
    g.op("Sub", &["one", "m4f"], "inv", vec![]);
    g.op("Mul", &["inv", "neg"], "ext", vec![]);
    g.op("Shape", &["h0"], "hshape", vec![]);
    g.op("Gather", &["hshape", "idx0"], "bsz", vec![]);
    g.op("Gather", &["hshape", "idx1"], "seq", vec![]);
    g.op("Unsqueeze", &["bsz", "ax0"], "bsz1", vec![]);
    g.op("Unsqueeze", &["seq", "ax0"], "seq1", vec![]);
    let axis0 = || vec![attr_int("axis", 0)];
    let parts = ["bsz1", "seq1", "nheads", "dhead"];
    g.op("Concat", &parts, "shp4", axis0());
    g.op("Concat", &["bsz1", "seq1", "dmodel"], "shp3", axis0());
}

/// Warstwa enkodera: uwaga + FFN (nazwy z prefiksem `l<i>_`).
fn layer(g: &mut Graph, l: usize, input: &str, output: &str) {
    let n = |s: &str| format!("l{l}_{s}");
    g.heads(input, &n("Wq"), &n("bq"), &[0, 2, 1, 3], &n("q"));
    g.heads(input, &n("Wk"), &n("bk"), &[0, 2, 3, 1], &n("k"));
    g.heads(input, &n("Wv"), &n("bv"), &[0, 2, 1, 3], &n("v"));
    g.op("MatMul", &[&n("q"), &n("k")], &n("sc"), vec![]);
    g.op("Mul", &[&n("sc"), "scale"], &n("scs"), vec![]);
    g.op("Add", &[&n("scs"), "ext"], &n("scm"), vec![]);
    let softmax = vec![attr_int("axis", -1)];
    g.op("Softmax", &[&n("scm")], &n("probs"), softmax);
    g.op("MatMul", &[&n("probs"), &n("v")], &n("ctx"), vec![]);
    let perm = vec![attr_ints("perm", &[0, 2, 1, 3])];
    g.op("Transpose", &[&n("ctx")], &n("ctxt"), perm);
    g.op("Reshape", &[&n("ctxt"), "shp3"], &n("ctxr"), vec![]);
    g.linear(&n("ctxr"), &n("Wo"), &n("bo"), &n("ao"));
    g.op("Add", &[&n("ao"), input], &n("r1"), vec![]);
    g.layer_norm(&n("r1"), &n("ln1_g"), &n("ln1_b"), &n("h1"));
    g.linear(&n("h1"), &n("W1"), &n("b1"), &n("f1"));
    g.op("Div", &[&n("f1"), "sqrt2"], &n("gd"), vec![]);
    g.op("Erf", &[&n("gd")], &n("ge"), vec![]);
    g.op("Add", &[&n("ge"), "one"], &n("ga"), vec![]);
    g.op("Mul", &[&n("f1"), &n("ga")], &n("gm"), vec![]);
    g.op("Mul", &[&n("gm"), "half"], &n("gelu"), vec![]);
    g.linear(&n("gelu"), &n("W2"), &n("b2"), &n("f2"));
    g.op("Add", &[&n("f2"), &n("h1")], &n("r2"), vec![]);
    g.layer_norm(&n("r2"), &n("ln2_g"), &n("ln2_b"), output);
}

fn pooler(g: &mut Graph) {
    let first = vec![attr_int("axis", 1)];
    g.op("Gather", &["last_hidden_state", "idx0"], "first", first);
    g.linear("first", "Wp", "bp", "pool_lin");
    g.op("Tanh", &["pool_lin"], "pooler_output", vec![]);
}

/// Wagi warstwy `l` (ziarna warstwy 0 jak w pierwszej wersji — referencje bez zmian).
fn layer_weights(s: &XlmrShape, l: usize) -> Vec<TensorProto> {
    let (d, f) = (s.dims, s.ffn);
    let base = 1000 * l as u64;
    let n = |x: &str| format!("l{l}_{x}");
    let spec: [(&str, u64, Vec<usize>, f32); 12] = [
        ("Wq", 104, vec![d, d], 0.25),
        ("Wk", 105, vec![d, d], 0.25),
        ("Wv", 106, vec![d, d], 0.25),
        ("Wo", 107, vec![d, d], 0.25),
        ("W1", 108, vec![d, f], 0.2),
        ("W2", 109, vec![f, d], 0.2),
        ("bq", 111, vec![d], 0.05),
        ("bk", 112, vec![d], 0.05),
        ("bv", 113, vec![d], 0.05),
        ("bo", 114, vec![d], 0.05),
        ("b1", 115, vec![f], 0.05),
        ("b2", 116, vec![d], 0.05),
    ];
    let mut out: Vec<TensorProto> = spec
        .into_iter()
        .map(|(name, seed, dims, scale)| {
            floats(
                &n(name),
                &dims,
                weights(base + seed, dims.iter().product(), scale),
            )
        })
        .collect();
    for ln in ["ln1", "ln2"] {
        out.push(floats(&n(&format!("{ln}_g")), &[d], vec![1.0; d]));
        out.push(floats(&n(&format!("{ln}_b")), &[d], vec![0.0; d]));
    }
    out
}

/// Bajty modelu o podanym kształcie (wagi deterministyczne).
pub fn xlmr_model_bytes(s: XlmrShape) -> Vec<u8> {
    let d = s.dims;
    let to_i64 = |n: usize| i64::try_from(n).unwrap_or(0);
    let w = |name: &str, seed: u64, dims: &[usize], scale: f32| {
        floats(name, dims, weights(seed, dims.iter().product(), scale))
    };
    let mut init = vec![
        w("E", 101, &[s.vocab, d], 0.5),
        w("P", 102, &[s.max_pos, d], 0.2),
        w("T", 103, &[1, d], 0.1),
        w("Wp", 110, &[d, d], 0.2),
        w("bp", 117, &[d], 0.05),
        floats("ln_e_g", &[d], vec![1.0; d]),
        floats("ln_e_b", &[d], vec![0.0; d]),
    ];
    for l in 0..s.layers {
        init.extend(layer_weights(&s, l));
    }
    let scalar = |name: &str, v: f32| floats(name, &[], vec![v]);
    let head_dim = (d / s.heads.max(1)) as f32;
    init.extend([
        scalar("one", 1.0),
        scalar("neg", -10_000.0),
        scalar("half", 0.5),
        scalar("two", 2.0),
        scalar("eps", 1e-5),
        scalar("sqrt2", std::f32::consts::SQRT_2),
        scalar("scale", 1.0 / head_dim.sqrt()),
        int64s("pad", &[], vec![1]),
        int64s("axis1", &[], vec![1]),
        int64s("idx0", &[], vec![0]),
        int64s("idx1", &[], vec![1]),
        int64s("ax0", &[1], vec![0]),
        int64s("ax12", &[2], vec![1, 2]),
        int64s("nheads", &[1], vec![to_i64(s.heads)]),
        int64s("dhead", &[1], vec![to_i64(d / s.heads.max(1))]),
        int64s("dmodel", &[1], vec![to_i64(d)]),
    ]);
    let mut g = Graph { nodes: Vec::new() };
    embeddings(&mut g);
    shared(&mut g);
    let mut input = "h0".to_owned();
    for l in 0..s.layers {
        let output = if l + 1 == s.layers {
            "last_hidden_state".to_owned()
        } else {
            format!("l{l}_out")
        };
        layer(&mut g, l, &input, &output);
        input = output;
    }
    pooler(&mut g);
    let ds = d.to_string();
    ModelProto {
        ir_version: 7,
        opset_import: vec![OperatorSetIdProto {
            domain: String::new(),
            version: 14,
        }],
        producer_name: "alfa-lib-embed-testkit-xlmr".into(),
        graph: Some(GraphProto {
            name: "xlmr-like".into(),
            node: g.nodes,
            initializer: init,
            input: vec![
                value("input_ids", INT64, &["batch_size", "sequence_length"]),
                value("attention_mask", INT64, &["batch_size", "sequence_length"]),
            ],
            output: vec![
                value(
                    "last_hidden_state",
                    FLOAT,
                    &["batch_size", "sequence_length", &ds],
                ),
                value("pooler_output", FLOAT, &["batch_size", &ds]),
            ],
            ..Default::default()
        }),
        ..Default::default()
    }
    .encode_to_vec()
}

/// Mały model zgodności (1 warstwa, 64 wym.) dla słownika `vocab`.
pub fn xlmr_like_model_bytes(vocab: usize) -> Vec<u8> {
    xlmr_model_bytes(XlmrShape::tiny(vocab))
}
