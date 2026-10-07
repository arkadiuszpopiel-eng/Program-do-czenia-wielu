# artifacts-contract

Kontrakt rejestru artefaktów (docs/modules/artifacts/SPEC.md): `Artifacts` (`out_dir`, `register`,
`add_version`, `get`, `list`, `preview`, `diff`, `intent`), typy wersji (ścieżka, rozmiar, SHA-256, MIME,
tura źródłowa, migawka), `Preview` (tekst/binarny), `TextDiff` (linie + format ujednolicony, crate `similar`),
`ArtifactAction` (Open, Reveal, CopyAsFile, SaveAs, Zip, SendToSession) i `ArtifactIntent` dla
`platform-windows`. Funkcje wspólne `-impl`/`-fake`: `read_file_facts`, `guess_mime`, `looks_binary`,
`preview_bytes`, `diff_texts`/`diff_contents`, `next_version` (deduplikacja hasha), `version_content`,
`default_out_dir` (`<root>\Sesje\<nazwa>\out`), `validate_action`.
