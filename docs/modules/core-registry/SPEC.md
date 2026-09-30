# core-registry — SPEC (szkic v0)

## Cel
Rejestr modułów: wczytuje manifesty `module.toml`, rozwiązuje kontrakty dostarczane/wymagane, zarządza cyklem życia (`always | lazy | on-demand`), izolacją (`inproc | process | wasm`), health-checkami i budżetami zasobów per moduł. Nie zawiera logiki domenowej.

## Fala i priorytet
F0 (kontrakt + schemat manifestu + fake). P0. Włączanie/wyłączanie w runtime (hot dla `process`/`wasm`) — F1; Wasm (`plugin-runtime`) — F8.

## Kontrakt (szkic Rust)
```rust
// core-registry-contract — SZKIC
pub struct ModuleManifest {
    pub id: ModuleId, pub version: Version,
    pub kind: ModuleKind,                 // Service | Tool | Provider | VoiceEngine | AgentPack | UiPanel
    pub provides: Vec<ContractId>, pub requires: Vec<ContractId>,
    pub capabilities: Vec<CapabilityRequest>,   // żądane zdolności (tokeny z Brokera)
    pub budget: ResourceBudget,           // ram_mb, cpu_pct, latency_ms (opcjonalnie)
    pub lifecycle: Lifecycle,             // Always | Lazy | OnDemand { idle_unload: Duration }
    pub isolation: Isolation,             // Inproc | Process { cmd } | Wasm { wit }
    pub config_schema: Option<SchemaRef>, pub ui: Option<UiContribution>,
    pub health: HealthSpec,
}
pub trait Registry: Send + Sync {
    fn list(&self) -> Vec<ModuleStatus>;
    fn enable(&self, id: &ModuleId) -> Result<(), RegistryError>;
    fn disable(&self, id: &ModuleId) -> Result<(), RegistryError>;
    fn resolve<C: Contract>(&self) -> Result<Arc<C>, RegistryError>;   // handle do kontraktu
    fn health(&self, id: &ModuleId) -> HealthReport;
}
pub enum ModuleState { Disabled, Loading, Ready, Degraded, Failed { restarts: u8 }, Unloaded }
```
Zdarzenia: `registry.module.state_changed`, `registry.module.budget_exceeded`, `registry.module.health`, `registry.resolve_failed`.

## Zależności
`core-bus-contract`, `core-config-contract`, `core-log-contract`. Uruchamianie procesów potomnych przez `platform-windows-contract` (`SystemPort`).

## Niezmienniki
- Moduł zależy wyłącznie od `-contract` innych modułów (sprawdzane też w CI, PLAN §4.3).
- Moduł niewłączony nie zajmuje pamięci ani CPU (zasada lekkości, PLAN §2).
- Graf kontraktów jest acykliczny; cykl = błąd startu z czytelnym komunikatem.
- Zdolności z manifestu to *żądanie*; przydział tokenów robi wyłącznie `safety-broker`.
- Przekroczenie budżetu: ostrzeżenie → zwolnienie modułu `lazy/on-demand`; nigdy zabicie modułu `always` bez watchdog.
- Restart modułu w crash-loopie ograniczony (N prób), potem stan `Failed` i raport do `watchdog`.

## Zdolności / uprawnienia
Brak własnych. Rejestr przekazuje żądania zdolności modułów do Brokera przy aktywacji.

## Izolacja
`inproc`, `always`. Sam rejestr uruchamia moduły `process` (JSON-RPC po stdio/named pipe; AppContainer + Job Object dla niezaufanych) i `wasm` (przez `plugin-runtime`, F8).

## Budżet zasobów
RAM ≤ 3 MB; start (wczytanie manifestów ≤ 60 modułów) ≤ 50 ms. Mierzone na baseline.

## Konfiguracja (klucze TOML)
`[modules.<id>] enabled = true|false`, `[modules.<id>.budget] ram_mb, cpu_pct` (nadpisanie manifestu w dół), `[core.registry] crash_loop_limit = 3`, `idle_unload_default = "10m"`. Nakładka per maszyna: lista modułów rezydentnych.

## Wkład do UI
Strona Ustawienia → Moduły (lista, włącz/wyłącz, budżety, stan zdrowia) generowana z rejestru; strony ustawień modułów z `config_schema`.

## Testy akceptacyjne
- `ACC-F0-core-registry-01`: moduł-przykład (trójka crate'ów) ładuje się, rozwiązuje kontrakt, przechodzi test kontraktowy.
- `ACC-F0-core-registry-02`: manifest z cyklem/brakującym kontraktem → błąd, nie panika.
- `ACC-F1-core-registry-03`: wyłączenie modułu `process` zwalnia pamięć drzewa procesów (pomiar Private WS).

## Fake
`core-registry-fake`: rejestr w pamięci z manifestami z fixture'ów, symulacja stanów (`Failed`, `Degraded`), wirtualne budżety — bez uruchamiania procesów.

## Otwarte pytania
- Schemat `module.toml` (JSON Schema) — powstaje w F0 pkt 2; wersjonowanie manifestu — do ustalenia w SPEC v1.
- Protokół JSON-RPC dla modułów `process` (nazwa metod, heartbeat) — wspólna z `watchdog`; do ustalenia w SPEC v1.
