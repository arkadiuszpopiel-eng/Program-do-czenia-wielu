# broker-ui — SPEC (v1: F3 część 2 — logika, okno Win32, IPC)

## Cel
Okno zatwierdzeń: osobny mały natywny proces w sesji użytkownika, uruchamiany z usługi Brokera na **wyższym poziomie integralności** niż agentki (UIPI blokuje SendInput z ich procesów); pokazuje prośby (akcja, ryzyko, odwracalność, plan), przyjmuje decyzje tylko z fizycznego wejścia, obsługuje zmianę poziomu autonomii, opcjonalnie Windows Hello. Nie WebView z treścią LLM (PLAN §8.2, §8.3).

## Fala i priorytet
F0: spike (k) (uruchomienie z usługi + test odrzucenia SendInput). F3: moduł. P0. Makieta 15.

## Nazwa i pomoc (właściciel 2026-10-31)
- Okno nazywa się dla właściciela **„Sędzia Cieni”** (EN: Shadow Judge) — nagłówek karty, pasek tytułu „Sędzia Cieni — <tytuł>”, wszystkie teksty interfejsu; identyfikatory i nazwy plików (`broker-ui`, `alfa-broker-ui.exe`) bez zmian.
- Przycisk **„?” w kółku** w nagłówku (też `F1`; `IDHELP` = 9, poniżej `MIN_BUTTON_ID`, nigdy nie jest decyzją) rozwija opis `JUDGE_HELP`: czym jest Sędzia Cieni, że każde działanie agentek przechodzi przez niego, związek z menu „Tryb pracy”, twarde blokady Jądra, dziennik i `Ctrl+Shift+F12`. Pole `SurfaceView.help` (≤ 1 200 znaków).
- **Na „Pełnej automatyce” (L4) okno nie pokazuje się wcale**, aż właściciel obniży tryb: klasyfikator na L4 daje tylko „wykonaj” albo twardą blokadę, a zatwierdzenie L4 wycofuje czekające prośby akcji i planów objętych tym poziomem.

## Kontrakt (źródło prawdy: `crates/broker-ui-contract`)
- `ApprovalCard::from_request(&ApprovalRequest, now, CardOptions)` — czysta funkcja: najpierw „Co dokładnie” (konkret od narzędzia, `DeclaredFacts.summary` → `ApprovalSubject::Action.summary`, np. „trwałe usunięcie pliku C:\….txt”; tytuł karty z tym konkretem, gdy ≤ 90 znaków), potem kto (persona), rodzaj (rodzina zdolności po polsku), gdzie (plik, folder, strona, program po ludzku), ryzyko + odwracalność, źródło polecenia, taint, „dlaczego pytam” z trybem pracy, przy którym padło pytanie (`ApprovalRequest.autonomy`), na końcu nazwa techniczna narzędzia; bez odliczania czasu (prośba nadal wygasa w Brokerze); plan (≤ 8 kroków wprost + „… i jeszcze N”), autonomia (cel, L→L, „na czas”), polityka. Opcje: „Odmów” (zawsze pierwsza, fokus startowy), „Zezwól tylko teraz”, „Zawsze w tym zakresie” (tylko pojedyncza akcja `grantable`, zakres = zdolność prośby, ≤ 24 h, `[broker_ui] grant_hours`). Tekst sanityzowany (`sanitize`: znaki sterujące, bidi, zerowej szerokości; obcięcie „…”).
- Reguły dowodu `check_input(foreground, input, occluded, shown_at, expires_at)`: wejście niewstrzyknięte → urządzenie rozpoznane → po pokazaniu karty i przed wygaśnięciem → okno **nieprzerwanie na pierwszym planie ≥ 500 ms** (`MIN_FOREGROUND_MS`, ochrona przed clickjackingiem; nowa karta w aktywnym oknie odlicza od nowa) → okno niezasłonięte przez okno innego procesu wyżej w kolejności Z.
- `BrokerUi` (`show`, `poll_decision`, `withdraw`, `queued`, `status`, `drain_events`) → `UiDecision { id, decision, proof: PhysicalInputProof }`; `BrokerLink` (`pending`, `resolve`); `HelloPort` (`NoHello`; Windows Hello — atrapa i typ w F3).
- Zmiana trybu pracy (autonomia, właściciel 27.10): karta ma `mode_change` — tytuł „Zmiana trybu pracy: <tryb>”, nazwy trybów jak w menu paska (Tylko podgląd, Pytaj o wszystko, Pytaj o ryzykowne, Automatycznie, Pełna automatyka), wiersze Tryb / Dotyczy / Co się zmieni / Zawsze zostaje (L4) / Na stałe albo Na czas / Źródło polecenia; bez „Kto”, „Ryzyko”, „Dlaczego pytam” i odliczania „Wygasa za” (prośba nadal wygasa w Brokerze); przycisk „Włącz: <tryb>”.
- `PhysicalInputProof` powstaje **wyłącznie** w `broker-ui-impl::session` (po `check_input`; nonce zużywany raz) i w atrapie testowej.
Zdarzenia: `broker_ui.shown`, `broker_ui.decided`, `broker_ui.injection_rejected`, `broker_ui.input_rejected` (za wcześnie, zasłonięte, nieaktywne), `broker_ui.hello.used`, `broker_ui.withdrawn` — w F3 do dziennika procesu (rola `BrokerUi` nie ma `AuditAppend`; zapis do Audytu przez Brokera — SPEC v2).

## Implementacja (F3, część 2)
- `broker-ui-impl`: `NativeBrokerUi<S: ApprovalSurfacePort>` (kolejka kart, najstarsza na ekranie), `driver::cycle/run` (synchronizacja z Brokerem: nowe → karty, rozstrzygnięte gdzie indziej / wygasłe / po kill-switchu → wycofane), `ChannelLink` (w procesie), `PipeLink` (named pipe; bilet `UiLaunchTicket` ze stdin; konto serwera potoku musi być kontem usługi z biletu).
- Okno: `platform-windows-kernel-impl::WinApprovalSurface` (czysty Win32 — decyzja otwartego pytania: bez WinUI 3), własny wątek z pętlą komunikatów, topmost; cały tekst rysowany w `WM_PAINT` i mierzony prawdziwą czcionką (`DrawTextW` + `DT_CALCRECT`, `DT_EDITCONTROL` łamie długie ścieżki i adresy, `DT_NOPREFIX`) — nic nie jest ucinane ani nie nachodzi na siebie (31.10); przyciski szerokie według napisów, a gdy się nie mieszczą — jeden pod drugim; za wysokie okno najpierw skraca opis pod „?”, potem wiersze od dołu, stopka zawsze widoczna. Ten sam kod rysuje podgląd w pliku (`render_preview`, test `app-broker/tests/okno_podglad.rs` → `target/verify/okno-sedziego/`, motyw jasny i ciemny, 100 % i 150 %). BUTTON rysowane samodzielnie bez przycisku domyślnego (`Enter` → `IDOK` ignorowany; `Tab` + `Spacja`), `Esc`/zamknięcie = odmowa, bez aktywacji (miganie) poza wysokim ryzykiem (`take_focus`), Segoe UI Variable. Próbka wejścia: `GetCurrentInputMessageSource` + hooki `WH_KEYBOARD_LL`/`WH_MOUSE_LL` (`LLKHF_INJECTED`, `LLKHF_LOWER_IL_INJECTED`, `LLMHF_*`) → `input_is_injected` (fail-closed).
- Uruchomienie: usługa Brokera (`app-safety`/`alfa-broker`) → `WTSQueryUserToken` (sesja konsoli) → `DuplicateTokenEx` → etykieta `High` (S-1-16-12288) → `CreateProcessAsUserW` na `winsta0\default`, bilet przez anonimowy potok stdin. Proces ma konto użytkownika bez praw administratora, ale UIPI blokuje SendInput/komunikaty z procesów średniej i niskiej integralności. Broker przyjmuje rolę `BrokerUi` tylko od obrazu `alfa-broker-ui.exe` z wysoką integralnością i z poświadczeniem z biletu (MAC; nigdy „zapis po tożsamości”). Tryb deweloperski: proces potomny bez UIPI (ostrzeżenie w logu).

## Aplikacja i tryby uruchomienia (F3 część 3, `app-broker`)
- Usługa: jak wyżej (sesja użytkownika, wysoka integralność). **Tryb przenośny** (`alfa-broker --console`, proces potomny aplikacji): Broker uruchamia Broker-UI „jak wywołujący” (`ChildLauncher`, średnia integralność, bez UIPI) — UI oznacza tryb jako słabszą izolację; Broker-UI kończy się po zerwaniu łącza, Broker uruchamia go ponownie.
- Aplikacja (WebView) nigdy nie zatwierdza: karta „czeka na zatwierdzenie” tylko przenosi uwagę do okna Sędziego Cieni (okno samo pokazuje wszystkie oczekujące prośby); przy zerwanym łączu albo bez okna karta wyjaśnia, że prośba zostanie odrzucona.
- **Przegląd Q-9** (`driver::cycle`): `UiError::Rejected` (nowy wariant kontraktu) = Broker jawnie odpowiedział błędem na decyzję — przy odmowie prośba po stronie Brokera już nie czeka (odmowa stosowana także przy awarii Audytu), karta znika; każdy inny błąd (np. łącze) także przy odmowie wraca do wołającego, karta nie jest uznana za rozstrzygniętą i wraca przy następnej synchronizacji (`tests/driver_errors.rs`). `PipeLink`/`ChannelLink` mapują odpowiedź `Error` Brokera na `Rejected`, błędy strumienia — na `Link`.

## Zależności
`safety-broker-contract` (prośby, decyzje, `ipc_blocking`), `platform-contract` (`ApprovalSurfacePort`, `InputSample`, potoki, tożsamość), `watchdog-contract` (zegar), `ui-kit` (tokeny kolorów jako wartości — bez frameworka WebView), `notify-contract` (plakietka/toast „przejdź do okna Sędziego Cieni").

## Niezmienniki
- Poziom integralności wyższy niż procesy agentek; test odrzucenia SendInput z procesu agentki w CI sprzętowym.
- Zatwierdzenie tylko z wejścia niewstrzykniętego (fizyczne kliknięcie/klawisz); `PhysicalInputProof` z nonce.
- Brak WebView, brak renderowania HTML/markdown z LLM; tekst z LLM tylko jako zwykły tekst z obcięciem.
- Destrukcja zlecona głosem: potwierdzenie wyłącznie tu, kliknięciem/klawiszem, do L3 (na L4 „Pełna automatyka” okno się nie pokazuje — decyzja właściciela 2026-10-31, wcześniej 27.10: obcy głos i niepewne rozpoznanie pytały także na L4); treść odczytana na głos przez `voice-tts` (zlecenie, nie zatwierdzenie).
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
