# core-log-impl

Log-writer jądra (docs/modules/core-log/SPEC.md). `FileLogSink` (`LogSink`): append-only NDJSON
w `<root>/<strumień>/<pierwszy-seq>.ndjson`; rotacja po `max_segment_bytes` (8 MiB), limit dysku
per strumień (512 MiB; usuwane najstarsze segmenty), retencja w dniach (Narzędzia/GUI: 7),
redakcja `Redactor` przed zapisem, `query` skanem segmentów (sesja/rodzaj/czas/`from_seq`/`limit`),
odtwarzanie numeracji po restarcie, urwany ogon → nowy segment (bez modyfikacji starych danych).
`PreBrokerAuditWriter` (`AuditWriter`, oznaczenie `pre-broker` do F3): łańcuch SHA-256 po kanonicznym
JSON rekordu, `prev_hash` w zdarzeniu; `verify_bytes`/`verify_file`/`verify_chain` wykrywają
modyfikację, usunięcie, wstawienie i ucięcie ogona. `spawn_bus_writer` zapisuje zdarzenia z
magistrali (Audyt → łańcuch, zdarzenia modułów → Diagnostyka). Indeks SQLite, szyfrowanie payloadów,
deny-lista i kolejka zapisu — F1.
