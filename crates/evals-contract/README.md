# evals-contract

Kontrakt harnessu ewaluacji (`docs/modules/evals/SPEC.md`): manifest zestawu (`SuiteManifest`, SHA-256 każdego
pliku, status `proposed`/`frozen`, progi), podziały `dev`/`test`/`holdout`, przypadki (`EvalCase`) i wyniki
(`CaseOutcome`), statystyka bez zależności zewnętrznych (bootstrap percentylowy z deterministycznym SplitMix64,
porównanie sparowane wariantów), raport JSON/Markdown, weryfikacja integralności oraz bramka ewaluacyjna
(`EvalGate`) — jedyna droga do holdoutu, zwracająca **wyłącznie wynik zbiorczy**. Rdzeń bramki (`run_variant`,
`decide`, `QueryBudget`) jest wspólny dla `evals-impl` i `evals-fake`; testy kontraktowe pod feature `contract-tests`.
