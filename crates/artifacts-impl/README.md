# artifacts-impl

Rejestr artefaktów w szyfrowanej bazie sesji: `artifacts(seq, id, name, path, origin)` i
`artifact_versions(artifact_id, version, meta, snapshot)` — **wersje niezmienne** (wyzwalacze), migawka treści
≤ 1 MiB jako BLOB (podgląd i diff po nadpisaniu pliku; większe pliki — tylko najnowsza wersja z dysku, jeśli
rozmiar się nie zmienił). Akcje UI jako intencje (zdarzenia `artifact.exported`/`artifact.handoff`).
`Module` + `module.toml`. Testy: kontrakt, wyzwalacze, szyfrowanie migawek, duże pliki (podgląd 1 MiB
< 100 ms), zdarzenia.
