# artifacts-fake

Atrapa rejestru artefaktów: `FakeArtifacts` — rejestr i migawki w RAM, pliki czytane z dysku (w testach —
katalog tymczasowy), identyfikatory `art-0001`…, rejestr intencji zamiast Eksploratora (`intents()`),
konfigurowalny limit migawki. Przechodzi ten sam `contract_tests::run_all` co `artifacts-impl`.
