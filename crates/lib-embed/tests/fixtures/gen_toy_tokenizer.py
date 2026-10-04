"""Tokenizer zabawkowy `src/testkit/toy-tokenizer.json` (Unigram 2000 na małych literach, NFKC + Lowercase).

Wejście: `corpus.txt` (jak w gen_tokenizer.py). sentencepiece 0.2.2, tokenizers 0.23.2.
"""
import json, unicodedata
import sentencepiece as spm
from sentencepiece import sentencepiece_model_pb2 as pb
from tokenizers import Tokenizer, models, normalizers, pre_tokenizers, processors, Regex, AddedToken
text = unicodedata.normalize("NFKC", open("corpus.txt", encoding="utf-8").read()).lower()
open("corpus_lower.txt", "w", encoding="utf-8").write(text)
spm.SentencePieceTrainer.train(input="corpus_lower.txt", model_prefix="toy_lower", vocab_size=2000, model_type="unigram",
    normalization_rule_name="identity", character_coverage=0.9995, input_sentence_size=100000, shuffle_input_sentence=False,
    num_threads=1, minloglevel=2)
m = pb.ModelProto(); m.ParseFromString(open("toy_lower.model", "rb").read())
vocab = [("<s>",0.0),("<pad>",0.0),("</s>",0.0),("<unk>",0.0)] + [(p.piece, round(p.score, 4)) for p in m.pieces[3:]] + [("<mask>",0.0)]
tok = Tokenizer(models.Unigram(vocab, unk_id=3, byte_fallback=False))
tok.normalizer = normalizers.Sequence([normalizers.NFKC(), normalizers.Lowercase(), normalizers.Replace(Regex(" {2,}"), " ")])
tok.pre_tokenizer = pre_tokenizers.Metaspace(replacement="▁", prepend_scheme="always", split=True)
tok.post_processor = processors.TemplateProcessing(single="<s> $A </s>", special_tokens=[("<s>",0),("</s>",2)])
tok.add_special_tokens([AddedToken(t, normalized=False, special=True) for t in ["<s>", "<pad>", "</s>", "<unk>"]] + [AddedToken("<mask>", lstrip=True, normalized=False, special=True)])
j = json.loads(tok.to_str())
for k in ("decoder",):
    j[k] = None
s = json.dumps(j, ensure_ascii=False, separators=(",", ":"))
open("toy-tokenizer.json", "w", encoding="utf-8").write(s + "\n")
print(len(s.encode()), len(vocab))
print(tok.encode("Jaki kolor lubi Karolina?").tokens)
