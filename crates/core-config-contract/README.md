# core-config-contract

Kontrakt konfiguracji warstwowej (docs/PLAN.md §3.5, docs/modules/core-config/SPEC.md).
Warstwy (`ConfigLayer`): `Default` (z manifestu) < `Shared` (wspólna dla maszyn) < `Machine(MachineId)`
(nakładka `config/machine/<id>.toml`); zakresy (`Scope`): `Global`, `Session`, `Agent`
(sesja/agentka > maszyna > wspólna > domyślna). Klucze to ścieżki kropkowe `ConfigKey("voice.stt.engine")`
z walidacją segmentów. Trait `ConfigStore` ma `get`/`set`/`watch(prefix)`; `set` niesie `Origin`
(User/Module/Improver/Import/Broker) — klucze `kernel_policy` zmienia tylko Broker.
Funkcja `resolve` liczy wartość wynikową z listy warstw (czysta, testowana). Sekrety nigdy tu nie trafiają.
Reguła polityk Jądra: `ConfigKey::is_kernel_policy()` (prefiks `kernel.`) i `authorize(key, origin)` — wspólne dla `-impl`
i `-fake`. Feature `contract-tests`: `contract_tests::run_all(Harness)` ze schematem-fixture'em `fixture_schema()`.
