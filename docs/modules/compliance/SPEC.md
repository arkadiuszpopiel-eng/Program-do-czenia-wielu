# compliance — SPEC (szkic v0)

## Cel
Rejestr zgodności tras (`compliance-registry.json`): status trasy (zielona/szara/zabroniona), data weryfikacji, cytat źródła i archiwalna kopia regulaminu, wyłącznik trasy, tagi prywatności/jurysdykcji; nieświeży rejestr degraduje trasę do „szarej". Karty zgodności w UI (PLAN §1.3, §5.5).

## Fala i priorytet
F1: v0 (rejestr, tagi z katalogu, wyłącznik trasy). F4: v1 (karty zgodności w UI, archiwum regulaminów, statusy „CLI `-p`" i „Agent SDK" osobno). P0 (v0).

## Kontrakt (szkic Rust)
```rust
// compliance-contract — SZKIC
pub enum RouteStatus { Green, Grey { reason: String }, Forbidden { reason: String } }
pub enum RouteKind { Api, CliHeadless, AgentSdk, Voice }
pub struct RouteEntry { pub id: RouteId /* np. "anthropic.api", "claude-code.cli" */, pub kind: RouteKind, pub status: RouteStatus,
    pub verified_at: Date, pub stale_after: Duration, pub source_quote: String, pub source_url: Url, pub archive: Option<PathBuf>,
    pub privacy: PrivacyTag /* may_train | no_train */, pub jurisdiction: Jurisdiction /* EU | US | CN | SG | ... */,
    pub retention: Option<Duration>, pub enabled: bool, pub pinned_cli_version: Option<Version> }
pub trait Compliance: Send + Sync {
    fn route(&self, id: &RouteId) -> Option<RouteEntry>;
    fn effective_status(&self, id: &RouteId) -> RouteStatus;   // uwzględnia świeżość i wyłącznik
    fn set_enabled(&self, id: &RouteId, on: bool, origin: Origin) -> Result<()>;   // wyłączenie: User; włączenie: Broker/User
    fn tags(&self, id: &RouteId) -> (PrivacyTag, Jurisdiction);
}
```
Zdarzenia: `compliance.route.status_changed`, `compliance.route.stale`, `compliance.route.disabled/enabled`, `compliance.cli_version.unknown` (trasa wyłączona).

## Zależności
`core-bus/config/log-contract`. Dane: `docs/compliance/compliance-registry.json` (wersjonowany w repo), `docs/compliance/archive/`.

## Niezmienniki
- Trasa `Forbidden` (np. Qwen Coding Plan, GLM/ZCode przez plan) nie może być włączona z UI ani przez agentkę.
- `verified_at + stale_after < dziś` → status efektywny co najwyżej `Grey`.
- Nieznana (nieprzypięta) wersja CLI mostu wyłącza trasę `CliHeadless` (F4).
- Rejestr jest polityką Jądra: `improver`/agentki nie zmieniają wpisów; edycja = commit w repo + przegląd.
- Tagi z rejestru są źródłem prawdy dla Routera (nie z konfiguracji użytkownika).

## Zdolności / uprawnienia
Brak.

## Izolacja
`inproc`, `always` (mały, odczyt przy starcie).

## Budżet zasobów
RAM ≤ 1 MB; odczyt rejestru ≤ 5 ms.

## Konfiguracja (klucze TOML)
`[compliance] registry = "compliance-registry.json"`, `stale_after_default = "90d"`, `[compliance.routes.<id>] enabled = true` (wyłącznik użytkownika).

## Wkład do UI
Karty zgodności przy dostawcach/mostach (Ustawienia → Modele i dostawcy): status, data, cytat, link, wyłącznik; ostrzeżenie „szara trasa" przy dodawaniu konta.

## Testy akceptacyjne
- `ACC-F1-compliance-01`: rejestr z fixture'ów — statusy efektywne (świeży/nieświeży/wyłączony) zgodne z regułami, property-based na datach.
- `ACC-F1-compliance-02`: wyłącznik — 0 wywołań wyłączonej trasy w 100 próbach (z `router`).
- `ACC-F4-compliance-03`: nieznana wersja CLI → trasa wyłączona (z `agent-backends`).

## Fake
`compliance-fake`: rejestr w pamięci ze sterowanymi statusami i datą „dziś".

## Otwarte pytania
- Schemat JSON rejestru (powstaje w §20 pkt 4); podpis rejestru (minisign) — do ustalenia w SPEC v1.
- Wpisy [W] (z cytatów wtórnych) — jak oznaczać stopień pewności w UI.
