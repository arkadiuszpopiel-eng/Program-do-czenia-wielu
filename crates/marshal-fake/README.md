# marshal-fake

Atrapa Marszałka: ten sam `MarshalCore`, tłumacz record/replay (`script(text, drafts)`), wirtualny
zegar, nagrane zdarzenia i zapisana księga reguł. Tylko jako dev-dependency. Testy: kontrakt oraz
**F5-09** (`tests/narrowing.rs`): 0 reguł rozszerzających z 50 (`evals/F5/marshal-rules.json`)
i właściwość „polityka efektywna ⊆ sufit” dla losowych zbiorów reguł.
