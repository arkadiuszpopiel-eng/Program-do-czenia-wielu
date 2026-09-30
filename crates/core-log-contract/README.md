# core-log-contract

Kontrakt logów jądra (docs/PLAN.md §13, docs/modules/core-log/SPEC.md).
`LogStream` to strumienie zapisywane przez `core-log`: `ModelCalls`, `ToolsGui`, `Voice`, `Diagnostics`.
Strumień **Audyt** ma osobny trait `AuditWriter` (jedyny writer to Broker, łańcuch hashy) —
rozdzielenie typami, nie konwencją. `LogSink` jest append-only: ma tylko `append` i `query`,
bez API modyfikującego czy usuwającego (usuwanie = crypto-shredding klucza sesji).
`Redactor` redaguje sekrety przed zapisem; `RegexRedactor::default()` zna wzorce `sk-…`, `xai-…`,
`Bearer …`, `AIza…`, `ghp_…`, `xoxb-…` i redaguje także zagnieżdżone wartości JSON.
