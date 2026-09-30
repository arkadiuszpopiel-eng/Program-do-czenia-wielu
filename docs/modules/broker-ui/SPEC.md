# broker-ui — SPEC (szkic v0)

## Cel
Okno zatwierdzeń: osobny mały natywny proces w sesji użytkownika, uruchamiany z usługi Brokera na **wyższym poziomie integralności** niż agentki (UIPI blokuje SendInput z ich procesów); pokazuje prośby (akcja, ryzyko, odwracalność, plan), przyjmuje decyzje tylko z fizycznego wejścia, obsługuje zmianę poziomu autonomii, opcjonalnie Windows Hello. Nie WebView z treścią LLM (PLAN §8.2, §8.3).

## Fala i priorytet
F0: spike (k) (uruchomienie z usługi + test odrzucenia SendInput). F3: moduł. P0. Makieta 15.

## Kontrakt (szkic Rust)
```rust
// broker-ui-contract — SZKIC (proces natywny; IPC z usługą przez named pipe z ACL)
pub struct ApprovalCard { pub id: ApprovalId, pub persona: PersonaDisplay /* imię, glif, kolor */, pub title: String, pub details: Vec<(String, String)>,
                          pub risk: RiskLevel /* Low | Mid | High */, pub reversible: Reversibility, pub voice_origin: bool, pub tainted: bool,
                          pub options: Vec<ApprovalDecisionKind>, pub read_aloud: Option<String> /* „Usuwam 14 plików z X — potwierdź” */ }
pub trait BrokerUi: Send + Sync {
    fn show(&self, card: ApprovalCard) -> Result<()>;
    fn show_autonomy_dialog(&self, target: AutonomyTarget, from: AutonomyLevel, to: AutonomyLevel) -> Result<()>;
    fn decisions(&self) -> Subscription<(ApprovalId, ApprovalDecision, PhysicalInputProof)>;
    fn status(&self) -> UiStatus /* Hidden | Pending(u8) | Blocked { reason } */;
}
```
Zdarzenia: `broker_ui.shown`, `broker_ui.decided` (Audyt), `broker_ui.injection_rejected` (próba SendInput z niższej integralności), `broker_ui.hello.used`.

## Zależności
`safety-broker-contract` (prośby, decyzje), `platform-windows-contract` (poziom integralności, Hello, fokus), `ui-kit` (tokeny kolorów jako wartości — bez frameworka WebView), `notify-contract` (plakietka/toast „przejdź do okna Brokera").

## Niezmienniki
- Poziom integralności wyższy niż procesy agentek; test odrzucenia SendInput z procesu agentki w CI sprzętowym.
- Zatwierdzenie tylko z wejścia niewstrzykniętego (fizyczne kliknięcie/klawisz); `PhysicalInputProof` z nonce.
- Brak WebView, brak renderowania HTML/markdown z LLM; tekst z LLM tylko jako zwykły tekst z obcięciem.
- Destrukcja zlecona głosem: potwierdzenie wyłącznie tu, kliknięciem/klawiszem, na każdym poziomie autonomii; treść odczytana na głos przez `voice-tts` (zlecenie, nie zatwierdzenie).
- Okno nie kradnie fokusu w trakcie pisania (focus-stealing prevention) — plakietka + dźwięk; przy wysokim ryzyku alertdialog z pułapką fokusu.
- Kolor ryzyka zawsze z ikoną i tekstem; obsługa klawiaturą; PL/EN.
- Nigdy w toaście, nigdy w oknie głównym.

## Zdolności / uprawnienia
Uruchamiany przez usługę Brokera; komunikuje się tylko z nią; nie ma tokenów zdolności.

## Izolacja
`process` (natywny, wyższy poziom integralności), `always` (ukryty, gdy brak próśb).

## Budżet zasobów
RAM ≤ 8 MB (natywne okno, bez WebView); pokazanie karty ≤ 100 ms; decyzja → Broker ≤ 20 ms.

## Konfiguracja (klucze TOML)
`[broker_ui] position = "near_cursor" | "fixed"` (per maszyna), `hello_enabled = false`, `sound = true`, `plan_mode = true` („plan do zatwierdzenia" zamiast 40 pytań), `show_metric_questions_per_hour = true`.

## Wkład do UI
Okno Brokera (makieta 15: zatwierdzenie, poziomy autonomii); karta „czeka na zatwierdzenie" w oknie głównym tylko przekierowuje tutaj.

## Testy akceptacyjne
- `ACC-F0-broker-ui-01`: spike (k): start z usługi na wyższym poziomie integralności działa; SendInput z procesu niższej integralności odrzucony (100/100).
- `ACC-F3-broker-ui-02`: ≥ 100 scenariuszy „sama zatwierdza" (SendInput, UIA Invoke, wstrzyknięcie przez schowek) = 0 sukcesów.
- `ACC-F3-broker-ui-03`: karta widoczna ≤ 100 ms; obsługa wyłącznie klawiaturą; kontrasty ryzyka w progach.

## Fake
`broker-ui-fake`: automatyczne decyzje ze skryptu z syntetycznym `PhysicalInputProof` (tylko w testach; produkcja odrzuca proof bez nonce usługi).

## Otwarte pytania
- Toolkit natywnego okna (Win32 czysty vs WinUI 3 — WinUI dodaje zależności) — ADR (3) po spike (k).
- Windows Hello: API (`UserConsentVerifier`) i zachowanie bez czujnika — do ustalenia w SPEC v1.
