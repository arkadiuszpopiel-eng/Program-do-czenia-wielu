# marshal — SPEC (v1: kontrakt zaimplementowany)

## Cel
Marszałek (PLAN §9.4): **nie szereguje** — zamienia polecenia w języku naturalnym na deklaratywne reguły, proponuje je, wykrywa konflikty; zatwierdza **użytkownik**. Reguły tylko zawężają uprawnienia. Dodatkowo nadzór postępu zadań (blokady, budżety, pętle, porażki, przerwania) z eskalacjami i raportem dziennym, relacjonowanymi przez Dyrygentkę.

## Fala i priorytet
F5, P1.

## Kontrakt (źródło prawdy: `crates/marshal-contract`)
```rust
pub struct Rule { id, description, when: When{resource, event: user_speaks, confidence, agent, role, task_class, origin, tainted, time}, then: Vec<Effect> }
pub enum Effect { Exclusive{resource, max_wait_ms ≤ 600 s, on_timeout}, Preempt{classes ≤ interactive}, PauseAtAtomic{scope: Gui|Audio}, DenyCapability, DenyFamily, RestrictTo{caps ⊆ sufit},
  RequireApproval, CapAutonomy{max ≤ sufit}, CapBudget{≤ sufit}, MaxParallel{≤ sufit}, QuietHours, DenyBridges }   // brak efektów rozszerzających
#[async_trait] pub trait Marshal { async fn propose(&str) -> Proposal; fn propose_drafts; fn approve(id, Approver /* tylko użytkownik */); fn reject; fn revoke(rule, Approver);
  fn rules; fn effective -> EffectivePolicy; fn set_ceiling(Ceiling); fn observe(&Event) -> Vec<Escalation>; fn check; fn daily_report(NaiveDate);
  fn proposals() -> Vec<Proposal> /* najnowsze pierwsze; domyślnie pusto */ }
#[async_trait] pub trait RuleTranslator { async fn translate(&str, &Ceiling) -> Vec<serde_json::Value> }   // LLM — szkice niezaufane
```
Zdarzenia: `marshal.rule.{proposed,approved,rejected,revoked}`, `marshal.escalation`, `marshal.report.daily`.

## Zależności
`scheduler`, `triggers` (strefa, cron raportu), `safety-broker` (`Capability`, `AutonomyLevel`), `personas`, `core-bus`, `core-registry` (`-contract`).

## Niezmienniki
- Szkic przechodzi tylko po ścisłym parsowaniu (nieznane pola/efekty = odrzucenie) i sprawdzeniu względem sufitu; `compose(sufit, reguły) ⊆ sufit` zawsze.
- Zatwierdza i cofa wyłącznie użytkownik (UI/głos); agentka może tylko zaproponować.
- Pauza tylko w punktach atomowych i tylko zadań GUI/audio; mowy użytkownika nie da się wywłaszczyć regułą.
- Propozycje (oczekujące i rozstrzygnięte) są częścią księgi (`RuleBook`) — zapisywane przy każdej zmianie (`MarshalHost::persist` → `FileMarshalStore`) i **przeżywają restart**; numeracja ciągła. Limit: ≤ `MAX_PENDING_PROPOSALS` = 50 oczekujących (nadmiar: najstarsze odrzucone, zdarzenie `marshal.rule.rejected` z `reason: limit` — odrzucenie nigdy nie rozszerza), ≤ `MAX_DECIDED_PROPOSALS` = 100 rozstrzygniętych (najstarsze usuwane).
- Eskalacje deduplikowane per zadanie i rodzaj, ≤ 20/h (nadmiar w raporcie); pamięć nadzoru ograniczona (24 h / 31 dni).

## Zdolności / uprawnienia
Brak.

## Izolacja
`inproc`, `always`.

## Budżet zasobów
RAM ≤ 4 MB; sprawdzenie reguły ≤ 1 ms.

## Konfiguracja (klucze TOML)
`[marshal] report_cron = "0 21 * * *"`, `stalled_after = "10m"`, `retry_alert_attempt = 3`, `max_escalations_per_hour = 20`, `rules_path`.

## Wkład do UI
Karta propozycji reguł (reguły, odrzucone szkice z powodami, konflikty, „Zatwierdź/Odrzuć”), Ustawienia → Agentki → Reguły, powiadomienia eskalacji, raport dnia.

## Testy akceptacyjne
- `ACC-F5-marshal-01` (F5-09): 0 reguł rozszerzających przechodzi z 50 (w tym złośliwe) — `marshal-fake/tests/narrowing.rs`, `evals/F5/marshal-rules.json`; właściwość „polityka ⊆ sufit”.
- Kontrakt (fake + impl): tylko użytkownik zatwierdza, konflikty, przykłady z PLAN §9.4, eskalacje, raport, lista propozycji z limitem; restart: `marshal-impl/tests/persistence.rs` (plik), `marshal-fake/tests/proposals.rs` (`FakeMarshal::restarted`).

## Fake
`marshal-fake`: tłumacz record/replay, wirtualny zegar, nagrane zdarzenia, restart z zapisanej księgi (`with_book`, `restarted`).

## Otwarte pytania
- Egzekucja `EffectivePolicy` w Brokerze (zawężanie tokenów) i schedulerze (limity, pauzy) — podpięcie w `app-*`/Broker (przegląd człowieka).
- Propozycja kolejności zadań („proponuje kolejność”) — do SPEC v2 na podstawie Osi czasu.

## Przegląd bezpieczeństwa #2 (2026-10, `docs/reviews/2026-10-security-review-2.md`)
- **SR2-05:** szkic z identyfikatorem aktywnej reguły jest odrzucany przy propozycji („najpierw ją cofnij”), a zatwierdzenie nigdy nie zastępuje aktywnej reguły — wcześniej „nowa” reguła o tym samym id po cichu usuwała obowiązujące zawężenie (np. `deny_bridges`), `marshal-fake/tests/review.rs`.
