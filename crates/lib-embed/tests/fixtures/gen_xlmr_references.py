"""Referencje enkodera o budowie eksportu HF XLM-R (`testkit::xlmr_like_model_bytes(1502)`) na tokenizerze
`xlmr-toy.tokenizer.json.gz`: onnxruntime 1.30 + HF `tokenizers` 0.23.2, średnia po masce + L2, każdy tekst
osobno oraz jeden wsad z wypełnieniem `<pad>` = 1 (sprawdzenie, że maska i `CumSum` pozycji działają).

Użycie: `ALFA_EMBED_DUMP_DIR=<dir> cargo test -p lib-embed --test it dump -- --ignored`, potem
`python gen_xlmr_references.py <dir> > xlmr-like.references.json` (w katalogu fixtures).
"""
import gzip, json, sys
import numpy as np
import onnxruntime as ort
from tokenizers import Tokenizer

d = sys.argv[1]
tok = Tokenizer.from_str(gzip.open("xlmr-toy.tokenizer.json.gz").read().decode())
tok.enable_truncation(max_length=64)
sess = ort.InferenceSession(f"{d}/xlmr-like.onnx", providers=["CPUExecutionProvider"])
texts = [
    "passage: Ulubiony kolor Karoliny to zielony.",
    "query: Jaki kolor lubi Karolina?",
    "passage: Zażółć gęślą jaźń — ﬁnanse ①",
    "query: ",
    "passage: " + "Spotkanie zespołu marketingu odbywa się we wtorki o 10:00. " * 6,
]
def pool(h, m):
    v = (h * m[:, None]).sum(0) / m.sum()
    return v / np.linalg.norm(v)
ids = [tok.encode(t).ids for t in texts]
single = []
for row in ids:
    h, p = sess.run(None, {"input_ids": np.array([row], np.int64), "attention_mask": np.ones((1, len(row)), np.int64)})
    single.append((pool(h[0], np.ones(len(row))), p[0] / np.linalg.norm(p[0])))
S = max(map(len, ids))
batch_ids = np.array([r + [1] * (S - len(r)) for r in ids], np.int64)
batch_mask = np.array([[1] * len(r) + [0] * (S - len(r)) for r in ids], np.int64)
hb, _ = sess.run(None, {"input_ids": batch_ids, "attention_mask": batch_mask})
max_diff = max(float(np.abs(pool(hb[i], batch_mask[i].astype(np.float32)) - single[i][0]).max()) for i in range(len(ids)))
assert max_diff < 1e-5, max_diff
r = lambda v: v.astype(np.float64).round(7).tolist()
print(json.dumps({"dims": 64, "batch_max_diff": max_diff, "cases": [
    {"text": t, "ids": i, "mean": r(m), "pooler": r(p)} for t, i, (m, p) in zip(texts, ids, single)
]}, ensure_ascii=False))
