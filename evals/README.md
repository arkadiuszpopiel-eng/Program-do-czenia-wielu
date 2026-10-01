# evals/

Zadania wzorcowe, zestawy akceptacyjne fal, korpus własny i benchmarki (docs/PLAN.md §3.6, §4.4, §12.4;
progi w `docs/ACCEPTANCE.md`). Harness: moduł `evals` (`crates/evals-{contract,impl,fake}`,
`docs/modules/evals/SPEC.md`). Struktura:

| Katalog | Zawartość | W git? |
|---|---|---|
| `F2/`, `F3/`, `F5/`, `F7/`, `F8/` | zestawy akceptacyjne per fala (dane + `MANIFEST.json` z SHA-256 i progami) | tak |
| `F8/harness-examples/` | manifesty w formacie natywnym dla istniejących zestawów F2/F3/F7 (bez zmiany ich treści) | tak |
| `acceptance/` | zamrożone kopie zestawów + `HASHES` (`sha256sum --check` w CI) | tak |
| `spikes/` | protokoły i skrypty spike'ów F0 (wyniki jako artefakty) | tak |
| `corpus/` | korpus własny: nagrania głosu, transkrypcje, dane osobiste | **nie** (`.gitignore`) |
| `holdout/` | ukryty zestaw bramki ewaluacyjnej (ochrona przed Goodhartem, PLAN §12.4, ACCEPTANCE F8-03) | **nie** — poza gitem (ACCEPTANCE §13); na maszynie: `%LOCALAPPDATA%\Alfa\evals\holdout\` |

## Format zestawu (harness `evals`)

Manifest natywny (`MANIFEST.json` albo `*.suite.json`, `evals_contract::SuiteManifest`, schemat 1):

```json
{
  "schema": 1, "suite": "f7-recall-synthetic", "wave": "F7", "version": 1, "status": "proposed",
  "created": "2026-10-01", "accepted_by": null, "description": "…",
  "files": { "F7/recall/synthetic/queries.ndjson": "<sha256>" },
  "cases": [ { "path": "F7/recall/synthetic/queries.ndjson", "format": "f7_recall_queries", "split": "test" } ],
  "thresholds": [ { "id": "F7-02", "rule": "metric", "metric": "recall_at_5", "op": "ge", "value": 0.85 } ]
}
```

- Ścieżki względem korzenia magazynu (`evals/` albo katalog holdoutu), z `/`, bez `..`, `\`, `:`.
- `status`: `proposed` (rozjazd hashy tylko raportowany) → `frozen` po akceptacji człowieka (każda zmiana pliku =
  `IntegrityViolation`, przypadki nie są wydawane) → `retired`.
- Podziały: `dev` (strojenie, także Ulepszacz), `test` (piaskownica przed/po), `holdout` (tylko w katalogu holdoutu;
  manifest publiczny ze źródłem holdoutu jest odrzucany).
- Formaty przypadków: `eval_cases` (NDJSON `{id, split, class?, input, expected}`), adaptery `f2_voice_manifest`,
  `f3_tool_tasks`, `f7_recall_queries`, `opaque` (plik tylko hashowany). Stary format `F5/MANIFEST.json` jest czytany
  i konwertowany (progi tekstowe).
- Progi metryczne sprawdzane na **granicy przedziału ufności** (bootstrap po przypadkach, powtórzenia uśredniane;
  `≥` — dolna, `≤` — górna); `per_class` — dla każdej klasy osobno (ACCEPTANCE §1).

## Zasady

- Zestawy są **zamrożone hashem**: zmiana pliku bez nowej wersji manifestu = czerwone CI
  (`cargo run -p evals-impl --bin alfa-evals -- verify evals`, test `crates/evals-impl/tests/repo_suites.rs`);
  zmiana progów i zestawów wymaga zatwierdzenia człowieka (AGENTS.md „Czego nie wolno”).
- **Holdout jest niedostępny dla Ulepszacza**: katalog publiczny nigdy nie czyta `holdout/` ani `corpus/` (także przez
  dowiązanie czy inną wielkość liter); jedyną drogą jest bramka ewaluacyjna Jądra (`EvalGate`), która zwraca wynik
  zbiorczy (bez identyfikatorów, treści i nazw klas), wymaga N ≥ 5 powtórzeń, ma budżet zapytań w oknie i ściśle
  sprawdza hashe przy każdej ocenie. Ulepszacz nie ma portu zapisu plików — nie zmienia zestawów ani progów.
- Raporty: JSON (artefakt CI) i Markdown (`EvalReport::to_markdown`, `GateVerdict::to_markdown`).
