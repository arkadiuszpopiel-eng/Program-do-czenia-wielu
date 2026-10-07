# core-config-impl

Konfiguracja warstwowa jądra (docs/modules/core-config/SPEC.md). `FileConfigStore` (`ConfigStore`):
`shared.toml` + `machine/<id>.toml` nakładane wg `ConfigLayer::precedence()` (Default < Shared < Machine);
nadpisania sesji/agentek w tabelach `["@session".<id>]` / `["@agent".<id>]` wygrywają z warstwami.
`register_schema(prefix, schema)` — JSON Schema modułu (crate `jsonschema`, bez zdalnych `$ref`),
`default` → warstwa Default; zapis niezgodny ze schematem → `SchemaViolation`. Klucze `kernel.*`
zmienia tylko `Origin::Broker` (także przy przeładowaniu z pliku). Zapis atomowy (tmp + fsync + rename),
historia append-only `history.ndjson` (klucz/stara/nowa/origin/źródło), `watch(prefix)`, jawne `reload()`
i `watch_files` (`notify` + debounce). Niepoprawny plik: wartości bez zmian, `config.invalid`,
zapis do tego pliku wstrzymany do naprawy (edycja użytkownika nie jest nadpisywana).
