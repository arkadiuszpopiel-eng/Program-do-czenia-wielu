# risk-classifier — SPEC (v1: kontrakt zaimplementowany)

## Cel
Deterministyczna ocena ryzyka akcji dla Brokera i poziomów autonomii: odwracalność, zakres, wpływ zewnętrzny (egress), destrukcyjność, masowość, pewność STT, źródło polecenia, taint, „lethal trifecta”, reguły Jądra → klasa ryzyka + werdykt + wyjaśnienie (PLAN §8.3, §8.0, §6.10). Bez LLM.

## Fala i priorytet
F3. P0. Jądro (agent nie zmienia; progi to polityka).

## Kontrakt (źródło prawdy: `crates/risk-classifier-contract`)
```rust
pub struct ActionFacts { tool, class: ActionClass /* Read|Write|Shell|GuiControl|Egress|SecretsRead|Admin */, reversible: Reversibility /* Yes|Scoped|No */,
    scope: ScopeRelation /* InScope|AllowedApp|Outside */, egress: Option<String>, egress_allowlisted, destructive /* None|Recoverable|Permanent */,
    bulk: u32, install, origin: CommandOrigin /* UserText|UserVoice{confidence: SttConfidence(‰), speaker_verified}|Agent|UntrustedContent */,
    tainted, untrusted_input_in_args, touches_private_data, kernel_rule: Option<KernelRule> }
pub enum RiskLevel { Low, Medium, High, Critical }
pub enum Verdict { Proceed, Ask { non_voice, grantable }, HardBlock { rule: KernelRule } }
pub fn evaluate(&ActionFacts, AutonomyLevel, &RiskPolicy) -> RiskVerdict { level, verdict, rules, factors, explanation }
pub trait RiskClassifier { fn policy(&self) -> RiskPolicy; fn evaluate(&self, f, autonomy) -> RiskVerdict; fn rules(&self) -> Vec<RuleDescription>; }
```
Zdarzenia: `risk.classified` (przy `Ask`/`HardBlock`), `risk.trifecta_detected`, `risk.rules.changed` (tylko z `KernelAuthority` Brokera).

## Tabela reguł (kolejność oceny; „każdy” = także L4, nigdy `grantable`)
| Reguła | Zasięg | Warunek |
|---|---|---|
| KernelBlock | każdy (blokada) | `kernel_rule` ustawione przez Brokera |
| VoiceDestructive | każdy, nie-głosem | głos + destrukcja (także do Kosza — bezpieczniejsza interpretacja §6.10) |
| VoiceLowConfidence | każdy, nie-głosem | głos + zmiana stanu + pewność < `stt_confidence_min` (800‰) |
| VoiceUnverifiedRisky | każdy, nie-głosem | głos bez weryfikacji mówcy + ryzyko ≥ średnie (do F5) |
| AdminConsent / Trifecta / TaintedEgress | każdy | `system.admin`; dane prywatne + niezaufane + egress; egress z sesji tainted |
| MutationNeedsYes | ≤ L1 | każda zmiana |
| RiskyAtL2 | ≤ L2 | usuwanie, egress, instalacja, sekrety, admin, nieodwracalne |
| TaintedHighRisk / UntrustedSource / CriticalRisk | ≤ L3 | tainted + ≥ wysokie; polecenie z niezaufanej treści; krytyczne (np. masowe trwałe usunięcie) |
| IrreversibleOutside / EgressNotAllowlisted / GuiOutsideApps | ≤ L3, grantable | nieodwracalne poza zakresem; host spoza allowlisty; aplikacja spoza wskazanych |
Klasa ryzyka: baza wg klasy akcji, podniesienia (destrukcja, masowość ≥ `bulk_threshold`, nieodwracalność, poza zakresem, instalacja, egress z danymi/taintem, trifecta = krytyczne, niezaufane źródło, obszar Jądra), niska pewność STT podnosi o stopień.

## Niezmienniki
- Deterministyczny; monotoniczny w poziomie autonomii z konstrukcji (każda reguła „do poziomu X” albo „każdy”) i w czynnikach ryzyka (taint, niezaufane, dane prywatne, instalacja, masowość, niższa pewność STT nigdy nie luzują) — testy własności.
- Twarde blokady i destrukcja głosem pytają/blokują na L4; L4 poza nimi pyta tylko o egress z taintem, trifectę, admina i głos.

## Zależności
`core-bus-contract`; wywoływany przez `safety-broker` (in-proc), podglądowo przez `agent-runtime`.

## Izolacja / budżet
`inproc` w usłudze Brokera, `always`. `classify` ≤ 0,1 ms; RAM ≤ 1 MB.

## Konfiguracja (klucze TOML)
`[security.risk] stt_confidence_min = 0.8` (500–990‰), `bulk_threshold = 50` (≥ 2) — `kernel_policy`.

## Testy akceptacyjne
- `ACC-F3-risk-classifier-01`: tabela 80 przypadków × 5 poziomów (`tests/table.rs`) + własności po 2000 przypadków (`tests/props.rs`); zamrożenie ≥ 200 × 5 w `evals/` — przez recenzenta.
- `ACC-F3-risk-classifier-02`: egress z sesji tainted nigdy `Proceed` (własność, każdy poziom).
- `ACC-F3-risk-classifier-03`: destrukcja głosem na L4 → zawsze `Ask{non_voice}` (własność + tabela).

## Fake
`risk-classifier-fake`: tabela + skrypt per narzędzie, rejestr wywołań; reguł Jądra nie da się zaskryptować.

## Otwarte pytania
- Czy `Critical` z niezaufanego źródła ma pytać także na L4 — dziś nie (zgodnie z PLAN §8.3); decyzja właściciela.
