# core-log-fake

Atrapa logów jądra (docs/modules/core-log/SPEC.md, „Fake”) do testów innych modułów (tylko
`dev-dependencies`). `FakeLogSink` (`LogSink`) trzyma rekordy w pamięci, nadaje `seq` od 0 per
strumień, redaguje sekrety tym samym `RegexRedactor` co implementacja, filtruje zapytania wspólnym
`LogQuery::matches`, liczy rekordy (`count`) i pozwala wstrzyknąć błąd (`fail_next`).
`FakeAuditWriter` (`AuditWriter`) prowadzi łańcuch w pamięci z deterministycznym, **niekryptograficznym**
skrótem FNV-1a (tylko do testów). Bez plików i szyfrowania. Przechodzi ten sam test kontraktowy co `-impl`.
