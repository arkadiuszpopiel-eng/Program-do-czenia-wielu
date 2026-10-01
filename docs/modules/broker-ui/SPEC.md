# broker-ui — SPEC (v1: F3 część 2 — logika, okno Win32, IPC)

## Cel
Okno zatwierdzeń: osobny mały natywny proces w sesji użytkownika, uruchamiany z usługi Brokera na **wyższym poziomie integralności** niż agentki (UIPI blokuje SendInput z ich procesów); pokazuje prośby (akcja, ryzyko, odwracalność, plan), przyjmuje decyzje tylko z fizycznego wejścia, obsługuje zmianę poziomu autonomii, opcjonalnie Windows Hello. Nie WebView z treścią LLM (PLAN §8.2, §8.3).

## Fala i priorytet
F0: spike (k) (uruchomienie z usługi + test odrzucenia SendInput). F3: moduł. P0. Makieta 15.

## Kontrakt (źródło prawdy: `crates/broker-ui-contract`)
- `ApprovalCard::from_request(&ApprovalRequest, now, CardOptions)` — czysta funkcja: kto (persona), co (rodzina zdolności po polsku), narzędzie, zakres, ryzyko + odwracalność, źródło polecenia, taint, „dlaczego pytam”, czas do wygaśnięcia; plan (≤ 8 kroków wprost + „… i jeszcze N”), autonomia (cel, L→L, „na czas”), polityka. Opcje: „Odmów” (zawsze pierwsza, fokus startowy), „Zezwól tylko teraz”, „Zawsze w tym zakresie” (tylko pojedyncza akcja `grantable`, zakres = zdolność prośby, ≤ 24 h, `[broker_ui] grant_hours`). Tekst sanityzowany (`sanitize`: znaki sterujące, bidi, zerowej szerokości; obcięcie „…”).
- Reguły dowodu `check_input(foreground, input, occluded, shown_at, expires_at)`: wejście niewstrzyknięte → urządzenie rozpoznane → po pokazaniu karty i przed wygaśnięciem → okno **nieprzerwanie na pierwszym planie ≥ 500 ms** (`MIN_FOREGROUND_MS`, ochrona przed clickjackingiem; nowa karta w aktywnym oknie odlicza od nowa) → okno niezasłonięte przez okno innego procesu wyżej w kolejności Z.
- `BrokerUi` (`show`, `poll_decision`, `withdraw`, `queued`, `status`, `drain_events`) → `UiDecision { id, decision, proof: PhysicalInputProof }`; `BrokerLink` (`pending`, `resolve`); `HelloPort` (`NoHello`; Windows Hello — atrapa i typ w F3).
- `PhysicalInputProof` powstaje **wyłącznie** w `broker-ui-impl::session` (po `check_input`; nonce zużywany raz) i w atrapie testowej.
Zdarzenia: `broker_ui.shown`, `broker_ui.decided`, `broker_ui.injection_rejected`, `broker_ui.input_rejected` (za wcześnie, zasłonięte, nieaktywne), `broker_ui.hello.used`, `broker_ui.withdrawn` — w F3 do dziennika procesu (rola `BrokerUi` nie ma `AuditAppend`; zapis do Audytu przez Brokera — SPEC v2).

## Implementacja (F3, część 2)
- `broker-ui-impl`: `NativeBrokerUi<S: ApprovalSurfacePort>` (kolejka kart, najstarsza na ekranie), `driver::cycle/run` (synchronizacja z Brokerem: nowe → karty, rozstrzygnięte gdzie indziej / wygasłe / po kill-switchu → wycofane), `ChannelLink` (w procesie), `PipeLink` (named pipe; bilet `UiLaunchTicket` ze stdin; konto serwera potoku musi być kontem usługi z biletu).
- Okno: `platform-windows-impl::WinApprovalSurface` (czysty Win32 — decyzja otwartego pytania: bez WinUI 3), własny wątek z pętlą komunikatów, topmost, STATIC z `SS_NOPREFIX`, BUTTON bez przycisku domyślnego (`Enter` → `IDOK` ignorowany; `Tab` + `Spacja`), `Esc`/zamknięcie = odmowa, bez aktywacji (miganie) poza wysokim ryzykiem (`take_focus`), Segoe UI Variable. Próbka wejścia: `GetCurrentInputMessageSource` + hooki `WH_KEYBOARD_LL`/`WH_MOUSE_LL` (`LLKHF_INJECTED`, `LLKHF_LOWER_IL_INJECTED`, `LLMHF_*`) → `input_is_injected` (fail-closed).
- Uruchomienie: usługa Brokera (`app-safety`/`alfa-broker`) → `WTSQueryUserToken` (sesja konsoli) → `DuplicateTokenEx` → etykieta `High` (S-1-16-12288) → `CreateProcessAsUserW` na `winsta0\default`, bilet przez anonimowy potok stdin. Proces ma konto użytkownika bez praw administratora, ale UIPI blokuje SendInput/komunikaty z procesów średniej i niskiej integralności. Broker przyjmuje rolę `BrokerUi` tylko od obrazu `alfa-broker-ui.exe` z wysoką integralnością i z poświadczeniem z biletu (MAC; nigdy „zapis po tożsamości”). Tryb deweloperski: proces potomny bez UIPI (ostrzeżenie w logu).

## Zależności
`safety-broker-contract` (prośby, decyzje, `ipc_blocking`), `platform-contract` (`ApprovalSurfacePort`, `InputSample`, potoki, tożsamość), `watchdog-contract` (zegar), `ui-kit` (tokeny kolorów jako wartości — bez frameworka WebView), `notify-contract` (plakietka/toast „przejdź do okna Brokera").

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
`[broker_ui] grant_hours = 8` (1–24), `hello_enabled = false`, `poll_ms = 250`; później: `position`, `sound` (F3: tylko miganie okna), `plan_mode = true` („plan do zatwierdzenia" zamiast 40 pytań), `show_metric_questions_per_hour = true`.

## Wkład do UI
Okno Brokera (makieta 15: zatwierdzenie, poziomy autonomii); karta „czeka na zatwierdzenie" w oknie głównym tylko przekierowuje tutaj.

## Testy akceptacyjne
- Stan F3/2: logika — 3000 przypadków własności `check_input` (wstrzyknięte nigdy), 2000 ciągów zdarzeń okna z wejściem wstrzykniętym = 0 decyzji, ciągi mieszane: każda decyzja z wejścia fizycznego ≥ 500 ms po aktywacji (`broker-ui-impl/tests/props.rs`), pełny cykl z Brokerem (`tests/flow.rs`), łańcuch procesów na atrapach z 100 kliknięciami wstrzykniętymi = 0 sukcesów (`app-safety/tests/chain.rs`); sprzęt — 100 × `SendInput` (mysz) + 20 × Spacja w prawdziwe okno: każde zdarzenie oznaczone jako wstrzyknięte (`platform-windows-impl/tests/kernel_windows.rs`, `#[ignore]`, self-hosted).
- `ACC-F0-broker-ui-01`: spike (k): start z usługi na wyższym poziomie integralności działa; SendInput z procesu niższej integralności odrzucony (100/100).
- `ACC-F3-broker-ui-02`: ≥ 100 scenariuszy „sama zatwierdza" (SendInput, UIA Invoke, wstrzyknięcie przez schowek) = 0 sukcesów.
- `ACC-F3-broker-ui-03`: karta widoczna ≤ 100 ms; obsługa wyłącznie klawiaturą; kontrasty ryzyka w progach.

## Fake
`broker-ui-fake::ScriptedBrokerUi`: skrypt per prośba / FIFO / domyślny (`Allow`, `AllowInScope{hours}`, `Deny`, `Injected` = odrzucone bez decyzji, `Ignore`) z syntetycznym `PhysicalInputProof` z nonce wyzwania (tylko testy).

## Otwarte pytania
- Windows Hello: `UserConsentVerifier` (WinRT) i zachowanie bez czujnika — SPEC v2; w F3 `NoHello` (prośby `hello_required` nie da się zatwierdzić, odmowa działa).
- Wejście z technologii asystujących (klawiatura ekranowa, Narrator) jest wstrzykiwane — odrzucane; alternatywa (Hello / helper `uiAccess`) do decyzji właściciela.
- Nakładki systemowe (np. overlay gier) nad oknem są traktowane jak zasłonięcie — lista wyjątków po podpisie obrazu w SPEC v2.
- Zdarzenia Broker-UI do Audytu (nowe żądanie IPC dla roli `BrokerUi`) — SPEC v2.
