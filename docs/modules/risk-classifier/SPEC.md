# risk-classifier — SPEC (szkic v0)

## Cel
Deterministyczna ocena ryzyka akcji dla Brokera i poziomów autonomii: odwracalność, zakres (w profilu / poza), wpływ zewnętrzny (egress), destrukcyjność, pewność STT (gdy zlecenie głosem), taint sesji, źródło (niezaufana treść) → werdykt: wykonaj / zapytaj / twarda blokada; „lethal trifecta" (dane prywatne + niezaufana treść + kanał wyjścia) (PLAN §8.3, §8.0, §6.10).

## Fala i priorytet
F3. P0. Jądro (agent nie zmienia; progi to polityka).

## Kontrakt (szkic Rust)
```rust
// risk-classifier-contract — SZKIC
pub struct ActionFacts { pub tool: ToolId, pub reversible: Reversibility, pub scope: ScopeRelation /* InProfile | AllowedApp | Outside */, pub egress: Option<HostPattern>,
                         pub destructive: Destructiveness /* None | Recoverable | Permanent */, pub bulk: Option<u32>, pub stt_confidence: Option<f32>,
                         pub source_voice: bool, pub tainted: bool, pub untrusted_input_in_args: bool, pub touches_private_data: bool, pub kernel_area: bool }
pub enum RiskLevel { Low, Mid, High }
pub enum Verdict { Proceed, Ask { reason: Vec<Reason> }, HardBlock { rule: KernelRule } }
pub struct RiskVerdict { pub level: RiskLevel, pub verdict: Verdict, pub explanation: String /* do karty zatwierdzenia */ }
pub trait RiskClassifier: Send + Sync {
    fn classify(&self, facts: &ActionFacts, autonomy: AutonomyLevel) -> RiskVerdict;
    fn rules(&self) -> Vec<RuleDescription>;    // do UI „dlaczego pyta”
}
```
Zdarzenia: `risk.classified` (Audyt przy `Ask`/`HardBlock`), `risk.trifecta_detected`, `risk.rules.changed` (tylko Broker).

## Zależności
`core-bus/config/log-contract`; wywoływany przez `safety-broker` (i podglądowo przez `agent-runtime`). Brak zależności od modeli.

## Niezmienniki
- Deterministyczny, bez LLM; ten sam `ActionFacts` + poziom = ten sam werdykt (property-based).
- Twarde blokady niezależne od poziomu (także L4): wyłączenie audytu, formatowanie dysku systemowego, zmiana polityk Jądra przez agentkę, `gui.control` wobec Alfy/Brokera/helpera, deny-listy §1.3, destrukcja zlecona głosem bez potwierdzenia nie-głosem.
- Mapowanie poziomów: L0 → wszystko `Ask`/blok zmian; L1 → każda zmiana `Ask`; L2 → `Ask` przy usuwaniu, egressie, instalacji; L3 → `Ask` przy nieodwracalnych poza zakresem lub przy niezaufanym wejściu; L4 → `Ask` tylko przy twardych regułach i głosowej destrukcji.
- Taint: `tainted && (egress || High)` → `Ask` na każdym poziomie ≤ L3; trifecta → `Ask`.
- Niska pewność STT podnosi poziom ryzyka akcji zleconej głosem.
- Progi i reguły to polityka Jądra (`kernel_policy`): `improver` i agentki nie zmieniają.

## Zdolności / uprawnienia
Brak.

## Izolacja
`inproc` w usłudze Brokera (i kopia read-only w jądrze dla podglądu), `always`.

## Budżet zasobów
`classify` ≤ 0,1 ms; RAM ≤ 1 MB.

## Konfiguracja (klucze TOML)
`[security.risk] stt_confidence_min = 0.8`, `bulk_threshold = 50`, `[security.risk.rules]` (tabela reguł, kernel_policy), `explain_in_ui = true`.

## Wkład do UI
Kolor/etykieta ryzyka i wyjaśnienie na karcie zatwierdzenia (Broker-UI), „dlaczego pyta" w Ustawieniach → Uprawnienia, prosty opis poziomów w onboardingu.

## Testy akceptacyjne
- `ACC-F3-risk-classifier-01`: tabela decyzyjna (≥ 200 przypadków × 5 poziomów) zamrożona w `evals/` — 100% zgodności; property-based determinizm i monotoniczność (wyższy poziom nigdy nie pyta więcej niż niższy, poza twardymi regułami).
- `ACC-F3-risk-classifier-02`: red-team injection ≥ 100 przypadków → 0 `Proceed` dla egressu z sesji tainted ≤ L3.
- `ACC-F3-risk-classifier-03`: destrukcja głosem na L4 → zawsze `Ask` (100/100).

## Fake
`risk-classifier-fake`: werdykty ze skryptu (per narzędzie) — testy `agent-runtime`, `broker-ui`.

## Otwarte pytania
- Dokładna tabela reguł i wagi — `THREAT_MODEL.md` (F0) i ADR (15); do ustalenia w SPEC v1.
- Czy klasyfikator ma dostęp do treści argumentów (np. ścieżek) czy tylko do faktów wyliczonych przez narzędzie — preferencja: fakty z narzędzia + walidacja ścieżek w Brokerze.
