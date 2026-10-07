# core-registry-contract

Kontrakt rejestru modułów (docs/PLAN.md §3.2, docs/modules/core-registry/SPEC.md).
`ModuleManifest` to model pliku `module.toml`: `id` (kebab-case), `version` (semver), `kind`,
`provides`/`requires` (`nazwa-contract@major`), `capabilities`, `budget` (RAM/CPU), `lifecycle`
(`lazy|on-demand|always`), `isolation` (`inproc|process|wasm`), `config_schema`, `ui`, `health`.
`ModuleManifest::parse_toml` parsuje i waliduje (błędy jako `ManifestError`, nigdy panika).
Trait `Module` (`manifest()`, `start(ctx)`, `stop()`, `health()`) jest wzorcem dla każdego `-impl`.
JSON Schema manifestu: `manifest_schema()` → `packages/schemas/module-manifest.v1.json`
(test snapshot, aktualizacja przez `UPDATE_SCHEMAS=1 cargo test -p core-registry-contract`).
Trait `Registry` (rejestracja, `start_order`, `boot`, `acquire`, `activate`, `deactivate`, `set_enabled`, `health`,
`unload_idle`, `shutdown`) ze stanami `ModuleState` i błędami `RegistryError`; czysty `DependencyGraph` (braki,
konflikty, cykle, deterministyczna kolejność topologiczna) wspólny dla `-impl` i `-fake`; zdarzenia `registry.*`.
Feature `contract-tests`: `contract_tests::run_all(Harness)` + `StubModule`/`manifest()` do testów.
