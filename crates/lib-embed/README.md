# lib-embed

Wspólna biblioteka (`lib-*`, bez logiki modułu): **lokalny embedder tekstu ONNX** dla `search` (PLAN §10,
ADR 0004, ACCEPTANCE F7-02). Zależy tylko od `search-contract` (trait `Embedder`) i `model-residency-contract`
(dzierżawa RAM) — moduły (`memory-impl` w testach, `app-*` w kompozycji) mogą od niej zależeć bez łamania
`scripts/check-deps.sh`.

| Element | Opis |
|---|---|
| `EmbedManifest` (`embed.json`, `alfa-embed-v1`) | pliki ONNX i `tokenizer.json` z **SHA-256** (plik o innym hashu nie jest ładowany), `dims` (= wymiar `vec0`), `max_tokens`, `pooling` (`mean`/`cls`/`pooled`), `query_prefix`/`passage_prefix` (E5), `output`, `batch_size`, `ram_mb`, `idle_unload_s`; `model_id()` = `id@<12 hex>` z odcisku plików i ustawień (każda zmiana → przebudowa wektorów w `search`) |
| `TextTokenizer` | własny tokenizer `tokenizer.json` dla **SentencePiece Unigram** (XLM-R: `multilingual-e5-*`, `paraphrase-multilingual-*`): tokeny dodane (`lstrip`/`rstrip`), normalizatory `Precompiled` (Darts + grafemy), `Replace`, `Strip`, `NFC/NFD/NFKC/NFKD`, `Lowercase`; pre-tokenizery `Metaspace` (`prepend_scheme` i dawne `add_prefix_space`), `WhitespaceSplit`; Viterbi z `fuse_unk`; `TemplateProcessing`/`RobertaProcessing`/`BertProcessing`; obcięcie jak HF. Nieznany element → błąd ładowania (nigdy cicha, inna tokenizacja). |
| `OnnxModel` | `tract-onnx` 0.23.8 (czysty Rust): wejścia `input_ids` + opcjonalnie `attention_mask`, `token_type_ids` (zera), wymiary symboliczne `B×S` (jeden plan), adnotacje kształtów z eksportu czyszczone, wybór wyjścia po nazwie |
| `Engine` | tokenizacja → wsady ≤ `batch_size` posortowane po długości (mniej wypełnienia) → pooling z maską → L2; kontrola kształtu wyjścia |
| `OnnxEmbedder` | `search_contract::Embedder`: **wątek tła** `alfa-embedder` (jedyny właściciel modelu), ładowanie leniwe albo `preload()`, zwolnienie po bezczynności / `unload()` / odebraniu dzierżawy, dzierżawa `model-residency` (`ModelRole::Embedder`, CPU, `Priority::Conversation`, `cpu_ram_mb = ram_mb`) **przed** ładowaniem; błąd ładowania pamiętany `retry_after` (domyślnie 30 s); panika w `tract` → błąd + wyładowanie; wywołanie dzielone na zlecenia ≤ 64 tekstów (zapytanie nie czeka za całą reindeksacją); tekst przycinany do `max_tokens × 64` B |
| `catalog` | znane modele: `multilingual-e5-small` (domyślny), `paraphrase-multilingual-minilm-l12-v2` — URL, rozmiar, licencja, ustawienia manifestu; **SHA-256 nieprzypięte** (HF niedostępny przy tworzeniu) |
| `install` | instalator przez port `Fetcher` (HTTP `Range` w `app-*`): wznawianie z `.part`, SHA-256 w locie, ≤ 3 automatyczne wznowienia, limit 2× rozmiar z katalogu, anulowanie, atomowe `rename`, na końcu `embed.json`; `HashPolicy::PinnedOnly` (domyślnie) odmawia pliku bez przypiętego hasha, `TrustOnFirstUse` (jawna zgoda w UI) zapisuje hash pierwszego pobrania |
| `testkit` (feature) | zabawkowe enkodery ONNX (wagi deterministyczne; także o budowie eksportu HF XLM-R: `CumSum` pozycji, uwaga wielogłowicowa, `Erf`, rozłożona `LayerNorm`; kształt `e5_small()` do pomiarów) + tokenizer zabawkowy + manifest |

Crate `tokenizers` (HF) odrzucony: nie przechodzi `cargo deny` (RUSTSEC-2024-0436 — nieutrzymywany `paste`) i ciągnie
~30 zależności (rayon, derive_builder…). Własny tokenizer jest zgodny z HF `tokenizers` 0.23.2 na wektorach
referencyjnych (2 × 58 przypadków: PL z diakrytykami NFC/NFD, pełna szerokość, ligatury, emoji, CJK, twarde spacje,
tokeny specjalne w tekście, obcięcie) na tokenizerze w stylu XLM-R z pełnymi regułami `nmt_nfkc`.

## Model (poza repo)

Domyślny: **`intfloat/multilingual-e5-small`** — licencja **MIT**, 118 M parametrów, 384 wym., ~100 języków (w tym
polski), max 512 tokenów, pooling średni, prefiksy `query: ` / `passage: `. Pliki (HuggingFace):

| Plik | URL | Rozmiar |
|---|---|---|
| `onnx/model.onnx` (fp32) | `https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/onnx/model.onnx` | ~471 MB |
| `tokenizer.json` | `https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/tokenizer.json` | ~17 MB |

Tylko **`onnx/model.onnx`** (operatory standardowe ONNX): warianty `model_O2…O4.onnx` z `optimum` zawierają operatory
ONNX Runtime `com.microsoft` (`Attention`, `SkipLayerNormalization`, `FastGelu`…), których `tract` nie wykonuje.
Ścieżki i rozmiary pochodzą z wiedzy o repozytoriach HF (sieć HF była zablokowana) — **do potwierdzenia przy
przypinaniu SHA-256**; ten sam model w ONNX jest też w `Xenova/multilingual-e5-small` (`onnx/model.onnx`).

Zapasowy: `sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2` (Apache-2.0, 384 wym., ≤ 128 tokenów, bez
prefiksów; te same ścieżki `onnx/model.onnx`, `tokenizer.json`). Wariant int8 (`onnx/model_qint8_avx512_vnni.onnx`,
~118 MB; `tract` ma `DynamicQuantizeLinear`/`MatMulInteger`) zmieściłby się w budżecie 300 MB z SPEC `search` — **do
sprawdzenia** na prawdziwym pliku przed dopisaniem do katalogu.

RAM i czas (fp32) — pomiar na modelu o kształcie e5-small z losowymi wagami (`e5_small_shaped_latency_and_ram`,
`--release` bez LTO, kontener 4 vCPU, 2026-10-03): plik 448 MB, ładowanie (SHA-256 + `tract`) **2,5 s**, RSS
**9 → 524 MB** (po pracy 529 MB), krótkie zapytanie **29 ms** (mediana), 32 dokumenty ~30 tokenów **1,8 s**
(57 ms/dok., wsady po 8), tekst 512 tokenów **0,72 s**. Szczyt przy ładowaniu ≈ 2× plik (bajty → protobuf →
tensory, kolejno zwalniane). Wpis katalogu: `ram_mb = 640`. Na baseline (Ryzen 5 5600) do powtórzenia:
`cargo test --release -p lib-embed --test it e5_small_shaped -- --ignored --nocapture`.

Ręczna instalacja (do czasu podpięcia menedżera): katalog `%LOCALAPPDATA%\Alfa\models\embed\multilingual-e5-small\`
z `onnx/model.onnx`, `tokenizer.json` i `embed.json`:

```json
{
  "format": "alfa-embed-v1", "id": "multilingual-e5-small", "license": "MIT",
  "model": {"path": "onnx/model.onnx", "sha256": "<sha256sum onnx/model.onnx>"},
  "tokenizer": {"path": "tokenizer.json", "sha256": "<sha256sum tokenizer.json>"},
  "dims": 384, "max_tokens": 512, "pooling": "mean",
  "query_prefix": "query: ", "passage_prefix": "passage: ",
  "batch_size": 8, "ram_mb": 640, "idle_unload_s": 300
}
```

## Testy

- `cargo test -p lib-embed` — jednostkowe + `tests/it` (jedno binarium — każde linkuje `tract`): tokenizer vs HF,
  model zabawkowy i model o budowie eksportu XLM-R vs **onnxruntime** (≤ 1e-4, także wsad z wypełnieniem),
  `pooled`/`token_type_ids`, zły wymiar/wyjście, prefiksy i podział zleceń, podmieniony plik (SHA-256), okno
  `retry_after`, dzierżawa (`LeaseTable` z kontraktu — `lib-*` nie może zależeć od `-fake`), bezczynność, odebranie
  dzierżawy, odmowa zarządcy, instalator (TOFU/pin, wznowienie, serwer bez `Range`, zły hash, limit, anulowanie).
- `#[ignore]`: `real_model_semantic_sanity` (`ALFA_EMBED_MODEL=<katalog>/embed.json`), pomiar `e5_small_shaped…`,
  generator referencji `dump` (`ALFA_EMBED_DUMP_DIR`) + `tests/fixtures/gen_*.py` (Python: `tokenizers` 0.23.2,
  `sentencepiece` 0.2.2, `onnxruntime` 1.30 — z PyPI).
- Eval F7-02 na prawdziwym modelu: `evals/F7/recall/README.md` (`ALFA_F7_EMBEDDER`).

## Podpięcie w `app-*` (dla sesji kompozycji)

**Stan (2026-10-04): podpięte w `app-models`** (SPEC `docs/modules/models/SPEC.md`) — pkt 1–6 zrealizowane z dwiema
różnicami: pobieranie idzie wspólnym menedżerem pobrań aplikacji (HTTPS z `Range`, ten sam układ katalogu
i `embed.json` przez `CatalogEntry::manifest`), a nie przez `install`/`Fetcher`; zmiana embeddera to
`SqliteSearch::set_embedder` (migawka na operację) zamiast przebudowy obiektu `SqliteSearch`. Komendy:
`models_*` (w tym `models_download {itemId: "multilingual-e5-small"}`), `embed_model_activate`,
`search_reindex_*`. Pkt 7 (eval w kompozycji) — nadal otwarty.

1. **Zależność:** `app-modules` (albo `app-core`) → `lib-embed = { path = "../lib-embed" }` (bez `testkit`).
2. **Wybór embeddera przy budowie `search`** (`app-core/src/parts/mod.rs`, gałąź `"search"`): jeśli
   `installed(<models>/embed/<id>)` zwraca manifest → `OnnxEmbedder::from_manifest_file(&path)?.residency(residency
   .clone()).spawn()?` (dzierżawa w tym samym `Arc<dyn Residency>` co `providers-local`/głos) i `Arc` jako `Arc<dyn
   Embedder>` do `SqliteSearch::new`; inaczej dotychczasowy `LexicalEmbedder` (bez modelu = bez regresji). Ustawienie
   `[search.embedder] model = "multilingual-e5-small" | "lexical"` w `core-config`.
3. **`LateIndexer`** (`app-modules/src/late.rs`) musi przekazywać nowe metody `TxIndexer`: `vector_status_in`,
   `reindex_step` (i brakujące dziś `compact_in`) — inaczej trafią w domyślne „nic do zrobienia”.
4. **Przebudowa po zmianie embeddera:** po starcie `search` (i po przełączeniu w UI) `search.spawn_reindex(extra,
   ReindexOptions::default(), Some(progress))`, gdzie `extra` = `ReindexSource` z bazami zakresów pamięci
   (`app-memory`: `VaultScopeDbs::known()` → `db(scope, false)` → `(index_label(scope), Arc<Db>)`). Uchwyt trzymać
   w stanie aplikacji (`drop` = anuluj). Przebudowa wznawia się po restarcie (kursor w bazie). Zdarzenia
   `search.reindex.{started,progress,done}`, `search.vector.missing` — tylko liczniki.
5. **Wpis menedżera modeli** (UI „Modele”): `lib_embed::CATALOG` (id, nazwa, licencja, `download_mb()`, uwagi);
   pobieranie = `install(entry, <models>/embed/<id>, &fetcher, policy, &cancel, &progress)` w `spawn_blocking`;
   `Fetcher` = adapter na `reqwest` z `Range: bytes=<offset>-` (wzór: `providers-local-impl/src/download.rs`;
   `EmbedError::Fetch` dla błędów przejściowych — wznawiane, inne bez ponawiania). **Przed wydaniem człowiek przypina
   SHA-256** w `catalog.rs`; do tego czasu UI pyta o zgodę (`HashPolicy::TrustOnFirstUse`) i pokazuje policzony hash.
6. **Komendy UI** (`app-api`, `COMMANDS.md`): `embed_models_list` (katalog + zainstalowane + aktywny), `embed_model_
   download {id}` (postęp `embed.download.progress {file, done, total}`, anulowanie), `embed_model_activate {id|
   "lexical"}` (zapis konfiguracji → przebudowa `SqliteSearch` z nowym embedderem → `preload()` → `spawn_reindex`),
   `search_reindex_start/cancel/status` (z `ReindexHandle::snapshot()` i `SqliteSearch::vector_status(session)`:
   „Przebudowa wektorów 1234/5000 — wyszukiwanie działa pełnotekstowo”), `embed_model_remove {id}` (tylko
   nieaktywny).
7. **Eval w kompozycji** (opcjonalnie): test w `app-memory` z `memory_impl::eval` + `SqliteSearch` + `OnnxEmbedder`
   (`ALFA_F7_EMBEDDER`) — ta sama hybryda co produkcja (bm25 + `vec0`), zamiast atrapy FTS z `memory-impl`.
