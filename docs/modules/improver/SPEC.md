# improver — SPEC (v1: kontrakt zaimplementowany)

## Cel
Ulepszacz (PLAN §12.1, §12.4; THREAT_MODEL S20): w bezczynności (nie na baterii, nie w grze) obserwuje metryki i wyniki evali, proponuje zmiany konfiguracji, promptów, słownika wymowy, wag routera i umiejętności, ocenia je w piaskownicy (podział `test`, przed/po) i na **ukrytym holdoucie** (bramka Jądra, wynik zbiorczy), wdraża przez `core-config` z historią i automatycznie cofa przy regresji. Nie zmienia Jądra, polityk bezpieczeństwa, progów ani zestawów `evals`, kodu, własnych uprawnień ani autonomii.

## Fala i priorytet
F8, P1. Etapy z karty zadania ↔ pierścienie PLAN §12.1: „R0” = obserwacja/propozycje, „R1” = piaskownica + holdout, „R2” = wdrożenie po zatwierdzeniu z rollbackiem; pierścienie zmian: R0 (prompty, słownik, wagi, ustawienia niekrytyczne — auto tylko zawężające/bezpieczne), R1 (umiejętności, manifesty — zatwierdza użytkownik, podpis), R2 (wtyczki — klucz wersji, 1 klik; `plugin-runtime` później), R3 (kod — tylko szkic zgłoszenia), Jądro — nigdy.

## Kontrakt (źródło prawdy: `crates/improver-contract`)
```rust
pub enum ChangeTarget { Config{key, value}, File{path, content} /* zawsze odrzucane */, Code{path, diff} /* R3 → IssueDraft */ }
pub fn assess(&ChangeTarget, current) -> Result<Assessment{ring, safety: Narrowing|Safe|Neutral|Widening, auto_eligible}, Violation>
pub const IMPROVABLE: [KeyRule; 13];  FORBIDDEN_PREFIXES; FORBIDDEN_SEGMENTS   // lista zamknięta, domyślnie wszystko zabronione
#[async_trait] pub trait Improver { observe(&MetricsSnapshot, RunConditions); submit(CandidateSet); evaluate(id); approve(UserApproval{digest, signature}); reject; monitor(&MetricsSnapshot); rollback; proposals; blocked; issue_drafts; policy() /* tylko odczyt */ }
pub trait Proposer /* model lokalny — niezaufany */; pub trait ApprovalVerifier /* TPM/Hello dla R1–R2 */; pub trait ImproverHost
pub struct ImproverCore<H> // wspólny dla -impl i -fake; jedyny port zapisu: ConfigStore z Origin::Improver
```
Zdarzenia (samo-zmiany → Audyt przez kompozycję): `improver.proposal.{created,blocked,evaluated,rejected}`, `improver.change.{deployed,rolled_back}`.

## Zależności
`core-config-contract`, `evals-contract`, `core-log-contract` (redaktor sekretów), `core-bus-contract`, `core-registry-contract` (impl).

## Niezmienniki
- Strażnik przy propozycji **i tuż przed każdym zapisem** (TOCTOU); jedno naruszenie odrzuca cały zestaw; źródło propozycji nadaje rdzeń (model nie podszyje się pod regułę).
- Zakazane zawsze: `kernel.*` (także przez `core-config`), prefiksy/segmenty Jądra, bezpieczeństwa, prywatności i tagów, budżetów/kosztów, uprawnień/autonomii, egressu i allow/deny-list, progów i bramki, `evals`, `improver.*`, `diagnostician.*`, kont i sekretów; wartości z URL/ścieżkami sieciowymi, sekretami, znakami niewidocznymi/bidi, poza zakresem.
- Auto-wdrożenie wyłącznie: pierścień R0 ∧ (zawężające ∨ bezpieczne) ∧ obie bramki zaliczone; reszta czeka na zatwierdzenie dokładnie tego diffu (`digest`); R1/R2 z podpisem (`ApprovalVerifier`).
- Wdrożenie: wartość bieżąca = `old` (inaczej przerwane), zapis warstwy wspólnej, weryfikacja wartości wynikowej, błąd → cofnięcie zapisanych. Rollback porównuje-i-zamienia (zmiana użytkownika nie jest nadpisywana) i ustawia wychładzanie klucza.
- Limity: zmian na propozycję, propozycji na dobę, aktywnych wdrożeń; brak API zmiany polityki; brak mostów CLI, sieci i plików.

## Zdolności / uprawnienia
Brak (zapis tylko przez `ConfigStore`, `Origin::Improver`).

## Izolacja
`inproc`, `on-demand` (cykl bezczynności `ImproverService::cycle`).

## Budżet zasobów
RAM ≤ 6 MB (bez modelu); cykl bez modelu < 10 ms.

## Konfiguracja (klucze TOML)
`[improver] auto_deploy_r0 = true`, `repeats = 5`, `suite_r0/r1/r2`, `max_changes_per_proposal = 10`, `max_proposals_per_day = 20`, `max_active_deployments = 5`, `watch_window = "24h"`, `regression_tolerance = 0.02`, `cooldown_after_rollback = "7d"`, `require_idle = true` — klucze `improver.*` są poza zasięgiem samego Ulepszacza.

## Wkład do UI
Panel „Zdrowie systemu” → kolejka propozycji (diff, uzasadnienie, wynik piaskownicy/holdoutu, plan cofnięcia, „Zatwierdź/Odrzuć/Cofnij”), dziennik zablokowanych prób, szkice zgłoszeń R3.

## Testy akceptacyjne
- `ACC-F8-improver-01` (F8-02): 137 prób (`evals/F8/improver/attacks.json`) → 0 zapisów, hashe `evals/F8` bez zmian — impl + fake.
- `ACC-F8-improver-02` (F8-04): 50 zmian R0 (`r0-changes.json`) → 0 rozszerzających/neutralnych auto, każde wdrożenie cofalne 1:1.
- `ACC-F8-improver-03` (F8-03): integracja z bramką holdoutu (`evals-fake`): propozycja zawiera tylko werdykt zbiorczy.
- Kontrakt (impl + fake): auto R0 + rollback po regresji, zatwierdzenia (diff, podpis), porażki bramki, warunki pracy, konflikty, zestawy mieszane, niezaufany model, R3; property-based strażnika.

## Fake
`improver-fake`: ten sam rdzeń z wirtualnym zegarem, zdarzeniami i stanem w pamięci.

## Otwarte pytania
- Źródło propozycji z modelem lokalnym (Router) i retrospektywy z `core-log` — podpięcie w `app-*`.
- Podpis zatwierdzeń kluczem TPM / Windows Hello (`ApprovalVerifier`) — `platform-windows` + przegląd człowieka.
- Format schematów kluczy w `core-config` z metadaną „kierunek zawężania” zamiast tabeli `IMPROVABLE` — SPEC v2 razem z `core-config`.
