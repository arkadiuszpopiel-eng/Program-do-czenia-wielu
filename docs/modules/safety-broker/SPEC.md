# safety-broker — SPEC (v1: logika — część 1; usługa, potok z ACL, Broker-UI — część 2)

## Cel
Usługa Brokera na osobnym koncie Windows: wydaje **tokeny zdolności** (`fs.read/write(zakres)`, `shell.exec(zakres)`, `gui.control(app)`, `net.egress(host)`, `secrets.read(id)`, `system.admin(op)`; TTL; potomek ≤ rodzic), prowadzi zatwierdzenia, jest **jedynym writerem Audytu** (łańcuch hashy, kotwica), trzyma polityki Jądra, poziomy autonomii L0–L4, kill-switch (PLAN §8.1–8.4, §8.6). Logika bez UI — okno zatwierdzeń to `broker-ui`.

## Fala i priorytet
F3, P0. Część 1 (ten stan): kontrakt, silnik, Audyt, IPC na dowolnym strumieniu, atrapa. Część 2: binarka usługi na osobnym koncie, named pipe z ACL na SID (przez `platform-windows`), weryfikacja SID/Authenticode klienta, natywne Broker-UI na wyższym poziomie integralności, test odrzucenia SendInput. SPEC i PR-y przez właściciela.

## Kontrakt (źródło prawdy: `crates/safety-broker-contract`)
- `Capability` + zakresy `PathScope` (normalizacja jak `compliance-contract`), `HostPattern` (`*.x.y`, bez `*`/`*.com`), `AppSelector` (bez wieloznaczności), `SecretId`, `AdminOp`; `is_subset_of` tylko w obrębie rodziny.
- `CapToken {id, parent, cap, holder{session, agent, role}, boot, key_epoch, issued_at_ms, expires_at_ms, mac}` — format `ALFT` v1, MAC HMAC-SHA256 na końcu, parser ścisły (re-encode = te same bajty).
- `Broker`: `decide(ActionRequest) -> Decision{Allow(token)|NeedsApproval(ticket)|Deny(reason)}`, `verify(token, needed, presenter)`, `attenuate`, `revoke`, `revoke_holder` (zmiana obsady), `report_untrusted_input`, `submit_plan`, `approval_status`, `request_autonomy_change`, `request_policy_change`, `metrics`.
- `ApprovalChannel` (wyłącznie Broker-UI): `pending() -> [ApprovalChallenge{request, nonce}]`, `resolve(id, decision, PhysicalInputProof)`.
- `PhysicalInputProof`: prywatne pola, brak `Clone/Default/Deserialize`, konstruktor `broker_ui_only::physical_input_proof` (umowa jak `KernelAuthority`; doctesty `compile_fail`). Siłę daje Broker: jednorazowy nonce wysyłany tylko kanałem Broker-UI, wejście niewstrzyknięte, świeżość, opcjonalnie Hello.
- `KernelGuard`/`check_command`, `KernelPolicy`, `AnchorStore`, `ipc::{Hello, Request, Response, ClientRole}`; `KillSwitch`/`JobRegistry` z `watchdog-contract`.
Zdarzenia (Audyt): `broker.token.issued/revoked/denied`, `broker.approval.requested/decided`, `broker.autonomy.changed`, `broker.kernel_block`, `broker.kill_switch`, `broker.policy.changed`, `broker.session.tainted`, `broker.key.rotated`, `broker.audit.chain_started`.

## Usługa (część 2: `safety-broker-impl::service`, binarka `alfa-broker` w `app-safety`)
- Potok `\\.\pipe\alfa-broker` z `PipeSecurity`: DACL chroniony — pełny dostęp konto usługi, klienci (konto użytkownika) tylko `FILE_GENERIC_READ | FILE_WRITE_DATA` (bez tworzenia instancji), etykieta `ML;;NW;;;ME` (procesy niskiej integralności nie piszą), `PIPE_REJECT_REMOTE_CLIENTS`, `FILE_FLAG_FIRST_PIPE_INSTANCE` (zajęta nazwa = start odmówiony: możliwe przejęcie). Wątek na połączenie, protokół z części 1 (`BrokerServer::serve_with` + adapter `BlockingIo`).
- Rola klienta wiązana z tożsamością procesu ustaloną przez system (`GetNamedPipeClientProcessId` → token: SID, integralność, sesja; obraz; Authenticode przez port — w dev „niezweryfikowane”): `RoleBindings` (`core`, `agent`, `broker_ui`, `watchdog`; brak = rola wyłączona na potoku). Jądro i watchdog mogą się przedstawić bez MAC (zapis po tożsamości obrazu — startują z launchera), Broker-UI **zawsze** z poświadczeniem z biletu i z wysoką integralnością (walidacja konfiguracji). Odrzucenia → Audyt `broker.ipc.rejected`.
- Audyt i kotwica w katalogu prywatnym (`PrivateDirPort`: nowy — `CreateDirectoryW` z deskryptorem, istniejący — chroniony DACL tylko konto usługi + SYSTEM).
- Nadzór Broker-UI (`UiSupervisor`): bilet `UiLaunchTicket` (poświadczenie per uruchomienie, nazwa potoku, SID konta usługi) przez stdin, ponowne uruchomienie z przerwą 1 s → 30 s.
- Host usługi Windows z `platform-windows-impl` (`StartServiceCtrlDispatcherW`, STOP/SHUTDOWN) — bez crate'a `windows-service` (jedna wersja windows-rs, kod OS tylko w `platform-windows`); `--console` = tryb deweloperski.

## Aplikacja jako klient Brokera poza procesem (`app-broker`, F3 część 3)
- **Wybór trybu przy starcie** (powłoka Tauri → `KernelProcesses::start`): (1) **usługa** — jest `%ProgramData%\Alfa\broker\broker.json` (`ServiceConfig`, zapisywany przy instalacji usługi — bramka #10): łączymy się z jej potokiem; usługa niedziałająca albo konfiguracja uszkodzona = bezpieczny stan z wyjaśnieniem, **bez cichego przejścia na tryb przenośny** (obniżenie izolacji byłoby atakiem); (2) **tryb przenośny** — obok aplikacji `alfa-broker.exe`: proces potomny `--console --lifeline` (konto użytkownika, potok `alfa-broker-dev`, Broker-UI uruchamia sam Broker przez `ChildLauncher`, oznaczone w UI jako słabsza izolacja); (3) **brak binarek**: build deweloperski (debug) i Linux/CI — Broker w procesie, jawnie oznaczony; **build produkcyjny (release) — bezpieczny stan „brak”** (`RemoteKernel::unavailable`: każda decyzja = odmowa; przegląd CX-b).
- **Klient roli `Core`** (`BrokerLink`): przed powitaniem sprawdza serwer potoku — tryb przenośny: PID procesu potomnego (inny proces tego samego konta może utworzyć kolejną instancję potoku), usługa: sesja 0, integralność systemowa, konto z `broker.json`; powitanie bez MAC (rola `Core` po tożsamości obrazu: `alfa-desktop.exe` z katalogu wersji). Jedno połączenie, wątek obsługi, limit 3 s na odpowiedź (kill-switch 500 ms); heartbeat `Metrics` co 1 s; ponowne łączenie, w trybie przenośnym ponowne uruchomienie Brokera z przerwą 1 s → 30 s (nowy klucz: tokeny i prośby poprzedniego uruchomienia wygasają).
- **Bezpieczny stan (fail-closed)** przy zerwaniu/limicie czasu/braku Brokera: `decide`, `attenuate`, `approval_status`… → `AuditUnavailable` (narzędzia: odmowa „Audyt niedostępny”, także dla czekających na zgodę), `verify` → odrzucenie, `session_security` → skażona + dane prywatne, `autonomy` → L0, okno zatwierdzeń niedostępne (karta w wątku tego nie obiecuje), zdrowie `safety-broker` = niesprawny, zdarzenie `BrokerStatus` → baner.
- **Kill-switch w aplikacji** (`RemoteKill`): najpierw lokalne drzewa procesów narzędzi (uchwyty Job Objects należą do procesu aplikacji — ten sam port, który je uruchomił), cisza audio na magistrali, potem `KillAll` w Brokerze (limit 500 ms). Watchdog robi `KillAll` niezależnie (dwa wpisy w Audycie — świadomie, odporność na zawieszoną aplikację).
- **Odstępstwo od kontraktu** (testy kontraktowe przez IPC: `app-broker/tests/contract_ipc.rs` — cały zestaw poza jednym scenariuszem): rola `Core` nie wyraża źródła „agentka”, więc prośbę agentki o zmianę poziomu `RemoteBroker` odrzuca jako `KernelBlock(SelfEscalation)` **także przy obniżeniu** (podanie jej jako zmiany właściciela fałszowałoby Audyt i kartę w Broker-UI); zmiana polityk z innego źródła niż Ustawienia → `KernelBlock(KernelPolicyChange)`. Obniżenie przez właściciela działa od razu.
- **Przegląd Q-9**: odmowa właściciela obowiązuje także przy awarii zapisu Audytu (`resolve` stosuje odmowę i zwraca `AuditUnavailable`); zgoda bez zapisu — prośba czeka dalej, bez tokenu (`tests/contract.rs::owner_denial_applies_even_when_audit_fails`).

## Zależności
`risk-classifier-contract`, `compliance-contract` (deny-listy, normalizacja), `watchdog-contract` (kill-switch, Job Objects, zegar), `core-bus/log/registry-contract`, `platform-contract`. Krypto: `hmac 0.12.1`, `sha2 0.10.9`, `getrandom 0.4.3`.

## Niezmienniki
- Potomek ⊆ rodzic (zakres, rodzina, TTL ≤ rodzic, ta sama sesja i agentka — delegacja do innej agentki wymaga nowej decyzji); TTL zawsze skończony (domyślnie 30 min, max 4 h).
- Token związany z podmiotem, uruchomieniem (`boot`) i epoką klucza; klucz tylko w pamięci, rotacja z oknem łaski = max TTL; kill-switch: nowy klucz bez łaski + czyszczenie rejestru, zgód, planów, próśb.
- Reguły Jądra sprawdzane przy wydaniu (zakres wewnątrz obszaru chronionego) i przy **każdym użyciu** (konkretna ścieżka/host), na każdym poziomie, także L4.
- Fail-closed: bez zapisu w Audycie nie ma tokenu ani prośby; odmowy i kill-switch działają także bez Audytu.
- Podniesienie poziomu i zmiana polityk wyłącznie przez `ApprovalChannel` z dowodem; agentka → `KernelBlock(SelfEscalation | KernelPolicyChange)` bez tworzenia prośby. Obniżenie (poziom **ściśle** niższy) działa od razu; prośba o poziom równy bieżącemu to podniesienie w czasie (utrwalenie/przedłużenie czasowego L4) — agentka → `SelfEscalation`, właściciel → prośba w Broker-UI (SR-02); obniżenie „na czas” jest bezterminowe, jeśli po wygaśnięciu poziom byłby wyższy niż przed żądaniem (regresja w `tests/negative.rs`); termin w przeszłości = błąd.
- „Zawsze zezwalaj” pokrywa tylko reguły zależne od poziomu (nigdy: głos, taint, trifecta, admin, Jądro), ma limit 24 h, nie zmienia poziomu. Plan pokrywa akcje ⊆ krok, nie groźniejsze niż zadeklarowane, tego samego podmiotu i źródła — wyłącznie kroki pokazane właścicielowi na karcie (plan bez kroków wymagających zgody niczego nie zapisuje), a każda reguła „każdego poziomu” bieżącego werdyktu (taint, trifecta, głos, admin) musiała być na karcie przy zatwierdzeniu (SR-01).
- Taint sesji monotoniczny (zdejmuje go tylko nowa sesja); składnik A trifecty = wydany `fs.read`/`secrets.read`/`shell.exec`.
- Brak nasłuchu TCP; IPC: poświadczenie klienta (rola + termin + MAC), uprawnienia per rola, źródło zmian ustalane z roli; rola `Agent` działa wyłącznie we własnym imieniu (`holder.agent` = `client_id` dla `Decide`, `SubmitPlan`, `Verify`, `Attenuate`, `ApprovalStatus`, `Revoke` cudzego tokenu = `Unauthorized`; SR-03).
- Obszar Jądra (`kernel_paths` bazowej polityki): katalog danych Brokera, `%APPDATA%\Alfa` i `%LOCALAPPDATA%\Alfa` (instalacja i wersje launchera, bazy, logi, profil WebView2, konfiguracja) — zapis = `KernelPolicyChange`; usunięcie katalogu zawierającego obszar Jądra albo `%SystemRoot%` = blokada niezależnie od rodzaju zakresu (SR-07). `gui.control` aplikacji dostawców planów (`PROVIDER_APPS`: `claude.exe`, `chatgpt.exe`, `codex.exe`…) = `ProviderWebUi` (SR-09). Regresje przeglądu 2026-10: `tests/review.rs`.

## Zdolności / uprawnienia
Źródło zdolności; `system.admin` przez UAC na żądanie (część 2).

## Izolacja
`process` (usługa Windows, sesja 0, osobne konto), `always`.

## Budżet zasobów
RAM ≤ 15 MB; `verify` ≤ 0,2 ms; `issue` bez zatwierdzenia ≤ 5 ms; zapis Audytu ≤ 2 ms p95; kill-switch (logika) zmierzony: p95 ≈ 0,1 ms na atrapach.

## Konfiguracja (klucze TOML)
`kernel_policy` = `KernelPolicy`: `token_ttl_default_ms`, `token_ttl_max_ms`, `approval_ttl_ms`, `grant_max_ms`, `plan_ttl_max_ms`, `profile_roots`, `allowed_apps`, `egress_allowlist`, `extra_protected_processes`, `kernel_paths`, `system_drive`, `deny_lists`, `risk`, `hello_required_for`. Procesy i usługi Jądra (`PROTECTED_*`) i obowiązkowe deny-listy nie do usunięcia.

## Testy akceptacyjne
- `ACC-F3-safety-broker-01`: kill-switch < 200 ms p95 z 50 prób (logika: `tests/budget.rs`; prawdziwy system — część 2, CI self-hosted).
- `ACC-F3-safety-broker-02`: „agentka zmienia Jądro / zatwierdza sama siebie” — logika: 116 scenariuszy = 0 sukcesów (`tests/negative.rs`, `tests/ipc.rs`); SendInput do Broker-UI — część 2.
- `ACC-F3-safety-broker-03`: property — potomek nigdy szerszy, zmiana dowolnego bajtu wykrywana, wygasły/obcy token odrzucony (`tests/props.rs`, testy kontraktowe).
- `ACC-F3-safety-broker-04`: łańcuch Audytu weryfikowalny po 10 000 zdarzeń, manipulacje i ucięcie ogona wykrywane kotwicą (`tests/audit.rs`); katalog prywatny — `tests/service.rs` (atrapa) i `app-safety/tests/windows_ports.rs` (Windows CI).
- Część 2: wiązanie ról z tożsamością (obcy obraz jako jądro, rola wyłączona, Broker-UI bez biletu / ze średnią integralnością, inne konto na DACL = odmowa), przejęcie nazwy potoku, nadzór Broker-UI (`tests/service.rs`); pełny łańcuch procesów (`app-safety/tests/chain.rs`); DACL egzekwowany przez system (`windows_ports.rs`).
- Część 3 (aplikacja ↔ Broker poza procesem, `app-broker/tests`): kontrakt przez IPC (`contract_ipc.rs`), scenariusz zgoda w Broker-UI → narzędzie wykonane / odmowa / wygaśnięcie / zerwanie łącza (bezpieczny stan, ponowne połączenie) / kill-switch (`scenario.rs`), tryby startu, awaria i ponowne uruchomienie Brokera, podstawiony serwer potoku usługi, uszkodzona konfiguracja bez trybu przenośnego (`kernel.rs`), koniec-koniec przez gniazda Unix 0600 w katalogu 0700 (`e2e_unix.rs`); release bez izolowanego Brokera = bezpieczny stan (`app-core/tests/wiring.rs`).

## Fake
`safety-broker-fake`: prawdziwy silnik z kluczem z jawnego ziarna, Audyt w pamięci, cisza audio jako zdarzenie, skrypt `Allow/NeedsApproval/Deny` per narzędzie (blokady Jądra nie do zdjęcia), `auto_approve` jak `broker-ui-fake`.

## Otwarte pytania
- Kotwica: plik w katalogu z chronionym DACL konta usługi (część 2, zrobione) vs TPM — ADR (THREAT_MODEL §11).
- Konto usługi: LocalSystem vs osobne konto z `SeTcbPrivilege` (potrzebne do `WTSQueryUserToken` i podniesienia etykiety) — bramka ludzka #10.
- Weryfikacja PID-em ma okno wyścigu przy przekazaniu uchwytu potoku i ponownym użyciu PID — obrona: MAC dla Broker-UI, wysoka integralność; `ImpersonateNamedPipeClient` jako drugie źródło — SPEC v2.
- (Broker poza procesem, **do decyzji człowieka**) Usługa wiąże rolę `Core` z pełną ścieżką obrazu, a instalacja per-user ma `versions\<ver>\alfa-desktop.exe` zmieniające się przy każdej aktualizacji: wiązanie po podpisie Authenticode (`signer`) albo aplikacja w `Program Files` dla trybu usługi. Do sprawdzenia na sprzęcie: czy proces użytkownika odczyta tożsamość procesu usługi (`OpenProcessToken`) — inaczej `GetNamedPipeServerSessionId` w porcie potoków. Stan Broker-UI dla jądra (dziś: „okno działa” = łącze + konfiguracja z Broker-UI) — żądanie IPC `UiStatus` w SPEC v2. Instalacja usługi z Ustawień (jednorazowy UAC) — poza tym SPEC-iem (bramka #10); UI tylko informuje.
- Źródło polecenia (`CommandOrigin`) deklaruje jądro; Broker utwardza je własnym taintem — pełna niezależność po przeniesieniu `voice-cmd` → Broker (F5).
- Strażnik poleceń powłoki jest leksykalny (obrona w głąb obok ograniczonego tokenu procesu); polecenia zakodowane (`-EncodedCommand`, `iex`) blokowane jako nieczytelne.

## Przegląd bezpieczeństwa #2 (2026-10) — zależności od Brokera (**do potwierdzenia przez człowieka**)
- **P2-07:** `agent-runtime` traktuje `SessionSecurity` Brokera jako źródło prawdy skażenia sesji (`BrokerSessionTaint`) — semantyka Brokera bez zmian (taint monotoniczny do końca sesji). Reset skażenia w runtime wymaga potwierdzenia właściciela nie-głosem, ale nie zdejmuje taintu Brokera; decyzja: czy Broker ma dostać jawny reset z dowodem fizycznego wejścia (Broker-UI), czy reset = nowa sesja (stan obecny, bezpieczniejszy).
- **P-07 (przegląd #1):** Broker-UI startuje z jawną listą dziedziczonych uchwytów (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`) — `platform-windows-kernel-impl/src/win_launch.rs`.

## Przegląd bezpieczeństwa #3 (2026-10, `docs/reviews/2026-10-security-review-3.md`) — **do przeglądu człowieka**
- **SR3-01 (zrobione, ścieżka Jądra):** `PROTECTED_PROCESSES` = nadzbiór `platform_contract::PROTECTED_IMAGES` — dodane `alfa-desktop.exe` (właściwy plik aplikacji), `alfa-updater.exe`, `alfa-mcp-proxy.exe`: twarda blokada `gui.control` i strażnik powłoki (`taskkill /im alfa-desktop.exe` = `KillSwitchDisable`). Test zgodności list: `tests/review.rs`.
- **SR3-03 (zrobione w `app-broker`):** Broker trzyma skażenie sesji i poziomy autonomii tylko w pamięci, a proces bywa uruchamiany ponownie (tryb przenośny — nadzór aplikacji; usługa — SCM). Aplikacja odtwarza w nowym połączeniu stan zawężający (skażenie, obniżenia poniżej L3) przed pierwszą decyzją (`app-broker::replay`). **P3-01:** trwały stan bezpieczeństwa po stronie Brokera (katalog prywatny usługi) jako źródło prawdy.
- **P3-02:** tryb usługi wybierany po istnieniu `%ProgramData%\Alfa\broker\broker.json`, ścieżka ze zmiennej środowiskowej procesu (zmienna użytkownika ją przesłania) — ciche przejście do trybu przenośnego; Known Folder albo rejestracja usługi w SCM (FFI w `platform-windows-kernel-impl`).
