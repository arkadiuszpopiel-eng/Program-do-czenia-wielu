# marshal-impl

Moduł `marshal`: `MarshalModule` (`Marshal` + `Module`) — nadzór na zdarzeniach `scheduler.*`
i `triggers.*` z magistrali, przegląd długich blokad co minutę, eskalacje `marshal.escalation`, raport
dzienny `marshal.report.daily` wg crona (domyślnie `0 21 * * *`, Europe/Warsaw), księga reguł
w `FileMarshalStore`, port `RuleTranslator` (w kompozycji: LLM przez Router; bez niego `NoTranslator` —
reguły tylko z edytora UI).
