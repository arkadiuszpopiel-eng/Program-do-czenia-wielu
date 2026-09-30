# safety-broker — SPEC (szkic v0)

## Cel
Usługa Brokera na osobnym koncie Windows: wydaje **tokeny zdolności** (`fs.read/write(zakres)`, `shell.exec`, `gui.control(app)`, `net.egress(host)`, `secrets.read`, `system.admin`; TTL; potomek ≤ rodzic), prowadzi zatwierdzenia, jest **jedynym writerem Audytu** (łańcuch hashy, pliki append-only przez ACL), trzyma polityki Jądra, poziomy autonomii L0–L4, kill-switch, operacje z UAC (PLAN §8.1–8.4, §8.6). Logika bez UI — okno zatwierdzeń to `broker-ui`.

## Fala i priorytet
F0: spike (k) + `THREAT_MODEL.md`. F3: usługa (tokeny, Audyt, poziomy, kill-switch, UAC). P0. SPEC i PR-y zawsze przez właściciela.

## Kontrakt (szkic Rust)
```rust
// safety-broker-contract — SZKIC (IPC: named pipe z ACL na SID, token sesyjny z TTL; bez TCP)
pub enum Capability { FsRead(Scope), FsWrite(Scope), ShellExec(Scope), GuiControl(AppSelector), NetEgress(HostPattern), SecretsRead(AccountId), SystemAdmin(Op) }
pub struct CapToken { pub id: TokenId, pub cap: Capability, pub holder: Holder, pub parent: Option<TokenId>, pub ttl: Timestamp, pub session: SessionId, pub sig: Mac }
pub struct ApprovalRequest { pub id: ApprovalId, pub session: SessionId, pub persona: PersonaId, pub action: ActionDescription, pub risk: RiskVerdict,
                             pub reversible: Reversibility, pub source_voice: bool, pub tainted: bool, pub plan: Option<PlanSummary> /* „plan do zatwierdzenia” */ }
pub enum ApprovalDecision { Allow, AllowInScope { scope: Scope, until: Timestamp } /* nie eskaluje do L4 */, Deny, Modify(ActionDescription) }
pub trait Broker: Send + Sync {
    fn issue(&self, req: CapRequest, parent: Option<TokenId>) -> Result<CapToken, BrokerError /* NeedsApproval(ApprovalId) | Denied | KernelBlock */>;
    fn verify(&self, t: &CapToken, needed: &Capability) -> Result<(), BrokerError>;
    fn approvals(&self) -> Subscription<ApprovalRequest>;                    // konsumuje broker-ui
    fn decide(&self, id: ApprovalId, d: ApprovalDecision, proof: PhysicalInputProof) -> Result<()>;
    fn set_autonomy(&self, target: AutonomyTarget /* Global | Session | Persona */, level: AutonomyLevel, proof: PhysicalInputProof) -> Result<()>;
    fn audit_append(&self, ev: &Envelope) -> Result<AuditHash>;
    fn kill_switch(&self, reason: KillReason) -> Result<()>;
    fn elevate(&self, op: AdminOp) -> Result<ElevatedHandle>;                // UAC na żądanie
}
```
Zdarzenia (Audyt): `broker.token.issued/revoked/denied`, `broker.approval.requested/decided`, `broker.autonomy.changed`, `broker.kernel_block`, `broker.kill_switch`, `broker.elevation`, `broker.policy.changed`.

## Zależności
`core-bus-contract`, `core-log-contract` (format rekordów), `risk-classifier-contract`, `platform-windows-contract` (pipe z ACL, tokeny restricted/AppContainer, Job Objects). Usługa nie zależy od `-impl` żadnego modułu agentek.

## Niezmienniki
- Osobny proces i osobne konto; agentki nie mają `gui.control` wobec procesów Alfy/Brokera/helpera; zakaz egzekwowany przed wydaniem tokenu.
- Potomek ≤ rodzic; reguły tylko zawężają; TTL zawsze skończony; token związany z sesją i holderem (MAC).
- Decyzje i zmiany poziomu tylko z `PhysicalInputProof` (wejście niewstrzyknięte z `broker-ui`); Windows Hello opcjonalnie dla L4/admin/polityk.
- Twarde blokady Jądra obowiązują na każdym poziomie, także L4: wyłączenie audytu, formatowanie dysku systemowego, zmiana polityk Jądra przez agentkę, destrukcja zlecona głosem bez potwierdzenia nie-głosem, deny-listy §1.3.
- Audyt: append-only, łańcuch hashy po digestach, głowa kotwiczona poza zasięgiem agentów; zdarzenia mostów CLI oznaczone „niezależnie niezweryfikowane".
- Brak nasłuchu TCP; IPC tylko named pipe z ACL na SID + token sesyjny z TTL.
- „Zawsze zezwalaj w tym zakresie" nigdy nie eskaluje do L4; zmiana obsady = nowe tokeny.
- Kill-switch: od klawisza do ciszy audio i zabicia Job Objects < 200 ms p95 (obsługa z `watchdog`/Brokera, nie UI).

## Zdolności / uprawnienia
Jest źródłem zdolności; sam działa z uprawnieniami konta usługi + `system.admin` przez UAC na żądanie (opcjonalnie allowlista poleceń z weryfikacją Authenticode+SID).

## Izolacja
`process` (usługa Windows, sesja 0, osobne konto), `always`.

## Budżet zasobów
RAM ≤ 15 MB; `verify` ≤ 0,2 ms; `issue` bez zatwierdzenia ≤ 5 ms; zapis Audytu ≤ 2 ms p95.

## Konfiguracja (klucze TOML)
Polityki Jądra (`kernel_policy`, zmiana tylko przez Broker): `[security] autonomy.default = "L3"`, `hello.required_for = ["L4", "admin", "policy"]` (opcjonalne), `egress.allowlist`, `denylist.paths`, `denylist.apps`, `token.ttl_default = "30m"`, `kill_switch.hotkey = "Ctrl+Shift+F12"`, `hard_blocks = [...]`.

## Wkład do UI
Brak własnego (okno w `broker-ui`); dane dla Ustawienia → Uprawnienia i bezpieczeństwo (poziomy, szablony uprawnień, metryka pytań/godz.) i dla karty „czeka na zatwierdzenie".

## Testy akceptacyjne
- `ACC-F3-safety-broker-01`: kill-switch < 200 ms p95 z 50 prób pod obciążeniem UI.
- `ACC-F3-safety-broker-02`: „agentka zmienia Jądro / zatwierdza sama siebie" ≥ 100 scenariuszy (w tym SendInput do Broker-UI) = 0 sukcesów.
- `ACC-F3-safety-broker-03`: property-based tokenów — potomek nigdy szerszy niż rodzic; wygasły/obcy token odrzucony 100%.
- `ACC-F3-safety-broker-04`: łańcuch hashy Audytu weryfikowalny po 10k zdarzeń; próba nadpisania pliku z konta użytkownika odrzucona (ACL).

## Fake
`safety-broker-fake`: in-proc, skryptowane decyzje (Allow/Deny/NeedsApproval), tokeny bez MAC, Audyt w pamięci — do testów `agent-runtime`, `tools-*`, UI.

## Otwarte pytania
- Kotwica głowy łańcucha hashy (plik pod ACL usługi vs TPM) — ADR z THREAT_MODEL.
- Format `PhysicalInputProof` (nonce z broker-ui + czas + źródło wejścia) — spike (k), ADR (3).
- Instalacja usługi na osobnym koncie (bramka ludzka #10) — procedura w `docs/`.
