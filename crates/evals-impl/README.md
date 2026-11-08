# evals-impl

Harness ewaluacji na dysku (`docs/modules/evals/SPEC.md`):

- `DirCatalog` — katalog zestawów publicznych z korzenia `evals/`: odkrywa `F*/MANIFEST.json` (format natywny albo
  stary format F5) i `*.suite.json`, waliduje ścieżki (bez `..`, `\`, `:`, dowiązań poza korzeń), weryfikuje SHA-256,
  czyta przypadki przez adaptery formatów (natywny NDJSON, F2, F3, F7). Katalogów `holdout/` i `corpus/` **nigdy** nie
  czyta (także przez dowiązanie ani inną wielkość liter).
- `HoldoutGate` — bramka ewaluacyjna (`EvalGate`): piaskownica na podziale `test` zestawu publicznego albo holdout z
  osobnego katalogu (poza gitem); integralność holdoutu sprawdzana przy każdej ocenie (ściśle), N ≥ 5, budżet zapytań,
  werdykt wyłącznie zbiorczy; zdarzenia `evals.gate.decided` / `evals.integrity.failed`.
- `run_suite` / `compare_suite` — przebieg wariantu na `dev`/`test` z raportem JSON/Markdown.
- `EvalsModule` + `module.toml`; bin `alfa-evals` (`list`, `verify`, `digest`) dla CI.

Testy: kontraktowe (`tests/contract.rs`), zapieczętowanie holdoutu i ścieżki (`tests/holdout.rs`), zestawy z repo
(`tests/repo_suites.rs`: F2/F3/F5/F7 jako przykłady formatu, F8 ściśle).
