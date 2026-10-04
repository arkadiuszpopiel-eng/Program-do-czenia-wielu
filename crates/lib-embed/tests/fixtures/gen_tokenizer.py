"""Fixture tokenizera w stylu XLM-R (`xlmr-toy.tokenizer.json.gz`) i referencje (`xlmr-toy.references.json`).

Wejście: `toy_pl.model` — SentencePiece Unigram (vocab 1500, `nmt_nfkc`) wytrenowany na docs/PLAN.md,
docs/ARCHITECTURE.md i evals/F7/recall/synthetic/*.ndjson (sentencepiece 0.2.2, seed domyślny).
Użycie: `python gen_tokenizer.py <katalog_wyjściowy>` (tokenizers 0.23.2).
"""
import json, gzip, sys
from sentencepiece import sentencepiece_model_pb2 as pb
from tokenizers import Tokenizer, models, normalizers, pre_tokenizers, processors, Regex, AddedToken

out = sys.argv[1]
m = pb.ModelProto(); m.ParseFromString(open('toy_pl.model','rb').read())
vocab = [("<s>",0.0),("<pad>",0.0),("</s>",0.0),("<unk>",0.0)] + [(p.piece, p.score) for p in m.pieces[3:]] + [("<mask>",0.0)]
tok = Tokenizer(models.Unigram(vocab, unk_id=3, byte_fallback=False))
tok.normalizer = normalizers.Sequence([normalizers.Precompiled(m.normalizer_spec.precompiled_charsmap), normalizers.Replace(Regex(" {2,}"), " ")])
tok.pre_tokenizer = pre_tokenizers.Metaspace(replacement="▁", prepend_scheme="always", split=True)
tok.post_processor = processors.TemplateProcessing(single="<s> $A </s>", pair="<s> $A </s> </s> $B </s>", special_tokens=[("<s>",0),("</s>",2)])
tok.add_special_tokens([AddedToken("<s>", normalized=False, special=True), AddedToken("<pad>", normalized=False, special=True), AddedToken("</s>", normalized=False, special=True), AddedToken("<unk>", normalized=False, special=True), AddedToken("<mask>", lstrip=True, normalized=False, special=True)])
new_json = json.loads(tok.to_str())

legacy = json.loads(tok.to_str())
legacy["normalizer"] = {"type": "Precompiled", "precompiled_charsmap": new_json["normalizer"]["normalizers"][0]["precompiled_charsmap"]}
legacy["pre_tokenizer"] = {"type": "Sequence", "pretokenizers": [{"type": "WhitespaceSplit"}, {"type": "Metaspace", "replacement": "▁", "add_prefix_space": True}]}
legacy["post_processor"] = {"type": "RobertaProcessing", "sep": ["</s>", 2], "cls": ["<s>", 0], "trim_offsets": True, "add_prefix_space": True}
legacy_tok = Tokenizer.from_str(json.dumps(legacy))

texts = [
    "", " ", "   ", "a", "Zażółć gęślą jaźń.", "ZAŻÓŁĆ GĘŚLĄ JAŹŃ", "Ulubiony kolor Karoliny to zielony.",
    "Jaki kolor lubi Karolina?", "query: Kiedy mam wizytę u dentysty?", "passage: Wizyta u dentysty jest 12 listopada o 9:00.",
    "  wiele   spacji\tTab\nnowa linia  ", " twarda spacja wąska", "Záżółć (NFD: ą ę ó)",
    "ＦＵＬＬ ｗｉｄｔｈ １２３", "ﬁnanse ﬂota ① ㎏ ™ ½", "Emoji 😀👍🏽 i flaga 🇵🇱", "中文 日本語 한국어", "Ελληνικά и русский текст",
    "hasło: Zielona-Sowa-42; e-mail: jan.kowalski@example.com", "1 234,56 zł — 3½ kg; 50%", "<s>specjalny</s> tekst <mask> koniec",
    "przed<mask>po", "a <unk> b <pad>", "Numer rejestracyjny PO 4821K, tel. +48 600 123 456",
    "Bezpieczniki są w szafce nad drzwiami wejściowymi.", "​zero‍width﻿bom", "x" * 40, "ąęłńóśźż" * 6,
    "Spotkanie zespołu marketingu odbywa się we wtorki o 10:00, a przegląd kotła gazowego jest ważny do końca października.",
]
def enc(t, text, maxlen):
    if maxlen is None:
        t.no_truncation()
    else:
        t.enable_truncation(max_length=maxlen)
    return t.encode(text).ids
cases = {"new": [], "legacy": []}
for name, t in (("new", tok), ("legacy", legacy_tok)):
    for text in texts:
        for maxlen in (None, 16):
            cases[name].append({"text": text, "max": maxlen, "ids": enc(t, text, maxlen)})
with gzip.open(out + "/xlmr-toy.tokenizer.json.gz", "wb", 9) as f:
    f.write(json.dumps(new_json, ensure_ascii=False, separators=(",", ":")).encode())
json.dump(cases, open(out + "/xlmr-toy.references.json", "w"), ensure_ascii=False, indent=None, separators=(",", ":"))
print(len(json.dumps(new_json)), new_json["normalizer"]["normalizers"][1], new_json["pre_tokenizer"], new_json["post_processor"]["type"], new_json["added_tokens"][-1])
print(cases["new"][4], cases["legacy"][4])
