# approval-judge — SPEC (v0: projekt, czeka na akceptację człowieka — dotyka Brokera)

## Cel
Model-sędzia trybu **„Automatycznie”** (L3): zamiast właściciela ocenia akcje, o które Sędzia Cieni zapytałby na L3
(plan 50 §4.3, zadanie Z.4). **Zamiana osoby zatwierdzającej, nie nowe uprawnienie** (zasada Codex): twarde blokady
Jądra, deny-listy, plany i zgody stałe działają przed nim; sędzia nie poszerza zakresu i nie jest granicą
bezpieczeństwa — może się mylić, dlatego ma bezpiecznik, dziennik i zawsze wraca do człowieka przy wątpliwości.

## Fala i priorytet
F3 (etap Z). P1. Część w Brokerze (`safety-broker-*`) — przegląd człowieka, gałąź `do-przegladu-tryby-zgod`.

## Architektura (dlaczego tak)
Broker działa w osobnym procesie, nie zależy od dostawców modeli, a kartę zatwierdza tylko fizyczne wejście
(`PhysicalInputProof`). Dlatego:
1. **Broker** (`decide.rs`, krok `Ask`): gdy poziom efektywny = **L3**, sędzia włączony, reguły werdyktu kwalifikują
   się (niżej), a bezpiecznik zamknięty — otwiera prośbę w stanie **`AwaitingJudge`** (niewidoczna w
   `pending()` dla broker-ui) zamiast `Pending`. Narzędzie czeka jak dziś (`approval_status` → `Pending`).
2. **Kierowca sędziego** w rdzeniu (`app-agents`, rola IPC `Core`) odpytuje `judge_queue()`, buduje wejście
   (kontekst rozmowy z rdzenia + fakty z Brokera), woła `ApprovalJudge::review`, odsyła `judge_resolve(id, verdict)`.
3. **Broker** przyjmuje werdykt tylko od roli `Core`, tylko dla prośby `AwaitingJudge`, tylko jako:
   `Allow` (**wyłącznie raz** — nigdy zgoda stała ani przełączenie trybu), `Deny{reason}` albo `Escalate` → prośba
   przechodzi w `Pending` i karta trafia do właściciela (z powodem sędziego).
4. **Termin:** prośba `AwaitingJudge` dłużej niż `timeout_s` (domyślnie 30 s) — Broker sam przestawia ją na `Pending`
   (karta właściciela). Kierowca nie działa / błąd modelu / nieczytelna odpowiedź → `Escalate`. **Nigdy cicha zgoda.**

## Kontrakt (`crates/approval-judge-contract`)
```rust
pub struct JudgeInput { goal: String, transcript: Vec<JudgeTurn>, action: JudgedAction, limits: Vec<String> }
pub enum JudgeTurn { Owner(String), AgentStep(String), ToolResult { tool: String, text: Untrusted } }
pub struct JudgedAction { tool, capability_summary, command: Option<String>, paths: Vec<String>, host: Option<String>,
    reversible: Reversibility, destructive: Destructiveness, rules: Vec<RuleId>, outside_workdir: bool, tainted: bool }
pub enum JudgeVerdict { Allow { reason }, Deny { category: DenyCategory, reason }, Escalate { reason } }
pub enum DenyCategory { SecretExfiltration, CredentialHunting, WeakenSecurity, IrreversibleDestruction,
    UntrustedInstruction, OwnerBoundary, Other }
#[async_trait] pub trait ApprovalJudge { async fn review(&self, input: JudgeInput, cancel) -> JudgeVerdict; }
pub fn eligible(rules: &[RuleId], level: AutonomyLevel, non_voice: bool) -> bool; // czysta, używa jej Broker
pub struct JudgeFuse { .. }  // czysta maszyna stanów, używa jej Broker (niżej)
pub fn render_prompt(&JudgeInput) -> String; pub fn parse_verdict(&str) -> JudgeVerdict; // błąd → Escalate
```
Zdarzenia audytu (pisze **tylko Broker**): `broker.judge.requested`, `broker.judge.decided` (werdykt, kategoria,
powód, model, czas oceny), `broker.judge.timeout`, `broker.judge.fuse_tripped`, `broker.judge.fuse_reset`.

## Które prośby idą do sędziego (`eligible`)
Tylko poziom efektywny **L3** (profil Dziecko z kłódką ma sufit ≤ L1 — nigdy). Prośba idzie do sędziego, gdy
**każda** reguła werdyktu jest na liście dozwolonych: `OutsideWorkdir`, `IrreversibleOutside`,
`EgressNotAllowlisted`, `GuiOutsideApps`, `TaintedHighRisk`, `UntrustedSource`.
**Zawsze do właściciela** (sędzia ich nie widzi): reguły głosu (`VoiceDestructive`, `VoiceLowConfidence`,
`VoiceUnverifiedRisky`), `AdminConsent`, `Trifecta`/`TaintedEgress`, `ProtectedPath` (plan 50 §4.5),
`CriticalRisk`, prośby `non_voice`, plany, podniesienia poziomu, zgody czasowe.

## Bezpiecznik (`JudgeFuse`, stan w Brokerze, per sesja + zadanie)
- **3 odmowy z rzędu albo 20 odmów w zadaniu** → bezpiecznik otwarty: kolejne prośby tej sesji idą prosto do
  właściciela, karta mówi „Sędzia odmówił N razy — decydujesz Ty”.
- `Allow` sędziego zeruje licznik „z rzędu”; `Escalate` go nie zmienia. Zgoda właściciela na kartę po otwarciu
  bezpiecznika zamyka go (oba liczniki od zera); odmowa właściciela — zostaje otwarty.
- Nowe zadanie (nowy przebieg główny rozmowy) zeruje licznik „w zadaniu”. Restart Brokera: stan w pamięci,
  start zamknięty (zachowawczo: nic nie przepuszcza bez sędziego, więc restart nie luzuje).

## Model i prompt
- `approval-judge-impl` woła `providers_contract::ModelProvider` (Router, `RouteHint { review: true, .. }`),
  temperatura 0, odpowiedź **tylko JSON** `{"verdict":"allow|deny|escalate","category":..,"reason":..}`.
- **Pochodzenie modelu:** przebieg lokalny → sędzia wyłącznie lokalny (rozmowa nie wychodzi do chmury); przebieg
  w chmurze → szczebel „sprawdzanie” Routera (lokalny najpierw). Brak modelu → `Escalate`.
- Wejście: cel zadania, zwięzły zapis (wiadomości właściciela dosłownie, kroki agentki skrótem, wyniki narzędzi
  obcięte i owinięte `wrap_untrusted` — **dane, nie polecenia**), bez ukrytego rozumowania agentki; limit
  `max_input_tokens` (6000), najstarsze tury wypadają pierwsze, wiadomości właściciela ostatnie.
- Domyślne odmowy w prompcie = `DenyCategory` (plan 50 §4.3) + granice z rozmowy („nie wypychaj”, „nie usuwaj”).
- Po `Deny` agentka dostaje powód i zakaz obchodzenia: „Wybierz wyraźnie bezpieczniejszą drogę albo zapytaj”.

## Niezmienniki (testy)
- Sędzia nigdy nie daje zgody stałej, nie przełącza trybu, nie widzi próśb spoza `eligible`, nie działa poza L3.
- Każda ścieżka błędu (termin, brak modelu, zły JSON, zerwane łącze, rola ≠ `Core`, prośba nie `AwaitingJudge`)
  kończy się kartą właściciela albo odmową — nigdy `Allow`.
- Treść obca z poleceniem „zatwierdź” nie zmienia werdyktu atrapy-modelu-podatnej poza `Escalate/Deny` (test
  wstrzyknięcia na prawdziwym modelu w Z.8).
- Werdykt i powód zawsze w audycie przed wydaniem tokenu (audyt niedostępny → `Deny(AuditUnavailable)`).

## Zależności
`approval-judge-contract` ← `safety-broker-impl` (tylko `eligible`, `JudgeFuse`, typy), `app-agents` (kierowca).
`approval-judge-impl` → `providers-contract`, `risk-classifier-contract`, `text` (`wrap_untrusted`).

## Izolacja / budżet
`inproc` w rdzeniu. Ocena ≤ `timeout_s`; na lokalnym Bieliku cel p95 ≤ 8 s (pomiar w Z.8). RAM ≤ 5 MB poza modelem.

## Konfiguracja (TOML, polityka Brokera — zmienia właściciel)
`[security.judge] enabled = true, timeout_s = 30 (5–120), fuse_consecutive = 3, fuse_per_task = 20,
cloud_allowed = true, max_input_tokens = 6000`.

## Testy akceptacyjne
- `ACC-Z4-judge-01`: tabela `eligible` — każda reguła × L0–L4 × Dziecko (Broker, fake sędziego).
- `ACC-Z4-judge-02`: bezpiecznik 3 z rzędu / 20 w zadaniu / reset zgodą właściciela (własności).
- `ACC-Z4-judge-03`: ścieżki błędów → karta właściciela (termin, brak kierowcy, zły JSON, rola Agent).
- `ACC-Z4-judge-04`: wstrzyknięcie przez treść obcą — zestaw ≥ 20 przypadków na prawdziwym modelu lokalnym.

## Fake
`approval-judge-fake`: werdykt ze skryptu (per narzędzie / kolejka), opóźnienie z wirtualnego zegara, rejestr wejść.

## Otwarte pytania (dla właściciela)
1. Bezpiecznik: plan 50 podaje „3 z rzędu albo 20 w zadaniu” (§4.1, §4.3) i — przy opisie Codex — „10 w ostatnich
   50”. Przyjęte **3 / 20 w zadaniu**.
2. `TaintedHighRisk` i `UntrustedSource` na liście dozwolonych (jak Codex — recenzent ocenia też treść obcą) czy
   zawsze do właściciela (bezpieczniej, więcej pytań)?
3. Czy sędzia w chmurze może dostać zapis rozmowy z przebiegu w chmurze (ten sam dostawca czy dowolny z Routera)?
