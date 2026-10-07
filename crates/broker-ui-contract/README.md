# broker-ui-contract

Kontrakt Broker-UI (docs/modules/broker-ui/SPEC.md, PLAN §8.2, ADR 3, THREAT_MODEL S11).
- **Karta** `ApprovalCard::from_request` — czysta funkcja z `ApprovalRequest`: kto, co, narzędzie, zakres, ryzyko
  z odwracalnością, źródło polecenia, taint, „dlaczego pytam”, czas do wygaśnięcia, plan (≤ 8 kroków + reszta),
  zmiana autonomii, polityka; opcje „Odmów” / „Zezwól tylko teraz” / „Zawsze w tym zakresie (≤ 24 h)”.
  Tekst przez `sanitize` (sterujące, bidi, zerowej szerokości, obcięcie).
- **Reguły dowodu** `check_input`: wejście niewstrzyknięte, rozpoznane urządzenie, w oknie ważności, okno
  nieprzerwanie na pierwszym planie ≥ 500 ms (`MIN_FOREGROUND_MS`), niezasłonięte.
- **Traity** `BrokerUi`, `BrokerLink`, `HelloPort` (`NoHello`), typy `UiDecision` (z `PhysicalInputProof`),
  `UiEvent`, `UiStatus`, `UiConfig`; zdarzenia `broker_ui.*`.
Testy: karta dla każdego rodzaju prośby, sanityzacja, 2 × 3000 przypadków własności (`tests/guard_props.rs`),
testy kontraktowe `contract-tests` (kolejka, wygasłe wyzwanie, `decision_matches`).
