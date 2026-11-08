"""Referencje zabawkowego modelu `lib-embed` (testkit): HF `tokenizers` 0.23.2 + onnxruntime 1.30.

Użycie: najpierw `ALFA_EMBED_DUMP_DIR=<dir> cargo test -p lib-embed --features testkit --test dump -- --ignored`,
potem `python gen_references.py <dir> > toy-model.references.json`. Wektor = średnia stanów po masce
(albo wyjście `pooled`), normalizacja L2; każdy tekst liczony osobno (wsad 1, bez wypełnienia).
"""
import json, sys
import numpy as np
import onnxruntime as ort
from tokenizers import Tokenizer

d = sys.argv[1]
tok = Tokenizer.from_file(f"{d}/tokenizer.json")
tok.enable_truncation(max_length=64)
texts = [
    "passage: Ulubiony kolor Karoliny to zielony.",
    "query: Jaki kolor lubi Karolina?",
    "passage: Paszport leży w górnej szufladzie biurka.",
    "query: ",
    "passage: " + "Spotkanie zespołu marketingu odbywa się we wtorki o 10:00. " * 8,
]
def run(model, ids, output, types):
    sess = ort.InferenceSession(f"{d}/{model}", providers=["CPUExecutionProvider"])
    feeds = {"input_ids": np.array([ids], dtype=np.int64), "attention_mask": np.ones((1, len(ids)), dtype=np.int64)}
    if types:
        feeds["token_type_ids"] = np.zeros((1, len(ids)), dtype=np.int64)
    out = sess.run([output], feeds)[0]
    v = out[0].mean(axis=0) if out.ndim == 3 else out[0]
    return (v / np.linalg.norm(v)).astype(np.float64).round(7).tolist()
cases = []
for t in texts:
    ids = tok.encode(t).ids
    cases.append({
        "text": t, "ids": ids,
        "mean": run("model.onnx", ids, "last_hidden_state", False),
        "pooled": run("model.onnx", ids, "pooled", False),
        "mean_types": run("model-types.onnx", ids, "last_hidden_state", True),
    })
print(json.dumps({"dims": 32, "cases": cases}, ensure_ascii=False))
