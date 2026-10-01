# evals/F7/recall — recall@5 pamięci (ACCEPTANCE F7-02)

Próg: **recall@5 ≥ 0,85 na ≥ 200 zapytaniach PL** — obowiązuje na prawdziwym embedderze (kompozycja `app-*`:
`memory-impl` + `search-impl` + lokalny model wielojęzyczny). Na CI: sprawdzenie formatu, zgodność plików z
generatorem i wynik na `HashEmbedder` (atrapa: trygramy → 64 wym.) — **raportowany, nieblokujący**.

## Zawartość

| Plik | Opis |
|---|---|
| `synthetic/corpus.ndjson` | korpus syntetyczny (100 faktów w 7 zakresach + 20 zmyłek; fikcyjne osoby i miejsca) |
| `synthetic/queries.ndjson` | 249 zapytań: pytanie naturalne, słowa kluczowe, wariant bez polskich znaków |
| `corpus.schema.json`, `queries.schema.json` | JSON Schema linii (generowane z kodu; test snapshotu) |

Pliki `synthetic/` generuje deterministycznie `memory_impl::eval::synthetic_set()` (dane:
`crates/memory-impl/src/eval/data.rs`). Test `f7_recall` porównuje je z generatorem — zmiana zestawu = zmiana kodu
i plików naraz (`ALFA_F7_WRITE=1 cargo test -p memory-impl --test f7_recall`), a zamrożenie wymaga przeglądu
człowieka (niżej).

## Format (NDJSON, linia = obiekt; puste linie i `#…` pomijane)

```json
{"id":"f001","scope":"global","layer":"semantic","text":"Ulubiony kolor Karoliny to zielony."}
{"id":"q001","query":"Jaki kolor lubi Karolina?","expected":["f001"],"scopes":["global"],"kind":"pytanie"}
```

- `corpus`: `id` (`[A-Za-z0-9_-]`, unikalny), `scope` (`global`, `project:<id>`, `agent:<id>`, `session:<id>`),
  `layer` (`semantic`, `episodic`, `procedural`), `text`, opcjonalnie `subject`.
- `queries`: `id`, `query`, `expected` (≥ 1 identyfikator korpusu, każdy w jednym z `scopes`), `scopes` (≥ 1),
  `kind` (dowolny; raport liczy recall per rodzaj).
- Metryki: recall@5 (średni odsetek oczekiwanych w top-5), trafienie@1, MRR; wyniki per zapytanie (`results.ndjson`)
  w katalogu tymczasowym systemu (`alfa-f7-recall-<etykieta>.ndjson`).

## Korpus użytkownika (poza gitem)

Własny zestaw w tym samym formacie trzymaj w `evals/corpus/f7/` (`.gitignore`): `corpus.ndjson` + `queries.ndjson`.

```bash
ALFA_F7_CORPUS=evals/corpus/f7 cargo test -p memory-impl --test f7_recall -- --nocapture
ALFA_F7_CORPUS=evals/corpus/f7 ALFA_F7_STRICT=1 …   # wymusza ≥ 200 zapytań i próg 0,85 (prawdziwy embedder)
```

## Wynik na atrapie (CI, 2026-10-01)

| Wariant | recall@5 | trafienie@1 | MRR |
|---|---|---|---|
| `HashEmbedder` + FTS (hybryda RRF) + reranker heurystyczny | 0,964 | 0,863 | 0,907 |
| atrapa leksykalna (`memory-fake`, rdzenie PL) | 0,948 | 0,855 | 0,897 |

Zestaw syntetyczny dzieli słownictwo z faktami — wynik na atrapie to kontrola regresji, nie dowód jakości
semantycznej. Rozstrzyga korpus użytkownika i prawdziwy embedder.

## Zamrożenie

Po akceptacji zestawu przez człowieka (AGENTS.md: zmiana zestawów zamrożonych tylko za zgodą):
skopiuj `synthetic/` do `evals/acceptance/F7/recall/` i dopisz sumy do `evals/acceptance/HASHES`
(`cd evals/acceptance && find F* -type f | LC_ALL=C sort | xargs sha256sum > HASHES`).
