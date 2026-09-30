# accounts-hub — SPEC (szkic v0)

## Cel
Konta i klucze dostawców dodawane w dowolnym momencie, bez restartu i bez zmian w kodzie: deklaratywny katalog dostawców (`providers-catalog/*.toml`), kreator „Dodaj dostawcę / konto / klucz" (test połączenia, wykrycie modeli, przypisanie do klas zadań i głosu, limit kosztów), klucze wyłącznie w Windows Credential Manager (PLAN §5.6). Wykrywanie CLI mostów i logowanie w ConPTY — od F4.

## Fala i priorytet
F1 (katalog, kreator, Credential Manager, stany „nieskonfigurowany"). F4: krok „mosty CLI". P0.

## Kontrakt (szkic Rust)
```rust
// accounts-hub-contract — SZKIC
pub struct ProviderCatalogEntry { pub id: ProviderId, pub name: String, pub base_url: Url, pub auth: AuthKind /* ApiKey | Bearer | None | Cli */,
    pub models: Vec<ModelInfo>, pub caps: Capabilities, pub pricing: PriceTableRef, pub privacy: PrivacyTag,
    pub jurisdiction: Jurisdiction, pub tos_url: Url, pub compliance: ComplianceStatus }
pub struct Account { pub id: AccountId, pub provider: ProviderId, pub label: String, pub secret: SecretRef /* uchwyt, nie wartość */,
    pub state: AccountState /* Unconfigured | Testing | Ok | Invalid | RateLimited | Disabled */,
    pub assignments: Assignments /* klasy zadań, agentki/role, STT/TTS */, pub cost_limit: Option<CostLimit> }
pub trait AccountsHub: Send + Sync {
    fn catalog(&self) -> Vec<ProviderCatalogEntry>;
    fn add_account(&self, provider: ProviderId, secret: SecretInput, label: String) -> Result<AccountId>;
    fn test(&self, id: AccountId) -> Result<TestReport>;          // połączenie + wykrycie modeli (np. Models API)
    fn rotate(&self, id: AccountId, secret: SecretInput) -> Result<()>;
    fn remove(&self, id: AccountId) -> Result<()>;
    fn secret_handle(&self, id: AccountId, caller: ModuleId) -> Result<SecretRef>; // tylko providers-*
}
```
Zdarzenia: `accounts.added`, `accounts.tested`, `accounts.state_changed`, `accounts.removed`, `accounts.catalog.updated`, `accounts.models.discovered`.

## Zależności
`core-bus/config/log-contract`, `platform-windows-contract` (Credential Manager, env), `compliance-contract` (status trasy), `cost-meter-contract` (limity), `providers-api-contract`/`providers-local-contract` (test połączenia przez adapter).

## Niezmienniki
- Wartość klucza nigdy nie opuszcza `accounts-hub` inaczej niż jako `SecretRef` odczytywany przez adapter dostawcy; nigdy w TOML, logach, paczkach `.alfa` (redakcja + test szpiegowski).
- Dodanie/rotacja/usunięcie klucza działa bez restartu (zdarzenie → Router przelicza trasy).
- Dostawca bez konta ma stan `Unconfigured`; Router go pomija; UI pokazuje „dodaj klucz, aby odblokować X".
- Katalog jest danymi (TOML), nie kodem; nowy dostawca kompatybilny z OpenAI/Anthropic = wpis + adapter generyczny.
- Trasa ze statusem zgodności „zabroniona" nie może dostać konta typu `Cli`; „szara" — z ostrzeżeniem.
- Alfa nigdy nie czyta ani nie przechowuje tokenów CLI (`~/.claude`, `~/.codex`).

## Zdolności / uprawnienia
`secrets.read` / `secrets.write` (Credential Manager) — tylko ten moduł; `net.egress(host)` dla testu połączenia z hostem z katalogu.

## Izolacja
`inproc`, `lazy` (aktywny przy kreatorze i przy odczycie `SecretRef`).

## Budżet zasobów
RAM ≤ 3 MB; test połączenia ≤ 10 s z timeoutem; zero CPU w bezczynności.

## Konfiguracja (klucze TOML)
`[accounts.<account_id>] provider, label, assignments.*, cost_limit_pln` (bez sekretów); `[accounts] catalog_dir = "providers-catalog"`, `import_env = ["ANTHROPIC_API_KEY", "OPENAI_API_KEY"]` (import na życzenie).

## Wkład do UI
Ustawienia → Modele i dostawcy → Hub kont i kluczy (makieta 12): lista, kreator, test, limity, usuń; krok onboardingu „konta i klucze: dodaj teraz / pomiń"; podpowiedzi „dodaj klucz, aby odblokować X".

## Testy akceptacyjne
- `ACC-F1-accounts-hub-01`: dodanie klucza (atrapa dostawcy) bez restartu → trasa dostępna ≤ 2 s.
- `ACC-F1-accounts-hub-02`: test szpiegowski — wartość klucza nieobecna w konfiguracji, logach, eksporcie `.alfa`.
- `ACC-F1-accounts-hub-03`: usunięcie klucza → dostawca `Unconfigured`, 0 wywołań po usunięciu.

## Fake
`accounts-hub-fake`: katalog z fixture'ów, sekrety w pamięci, `test()` zwraca skryptowane raporty (Ok/Invalid/RateLimited) i listy modeli.

## Otwarte pytania
- Schemat `providers-catalog/*.toml` (powstaje w §20 pkt 4) i jego aktualizacja (z repo aktualizacji?) — do ustalenia w SPEC v1.
- Import kluczy z eksportu sekretów `.alfa` (format `secrets.enc`) — razem z `transfer` v1.
