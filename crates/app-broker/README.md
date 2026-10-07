# app-broker

Korzeń kompozycji (`app-*`, check-deps): Broker **poza procesem** aplikacji — ADR 0003, PLAN §8.1–8.2, §8.6;
SPEC-i `safety-broker` (część 3), `broker-ui`, `watchdog`. **Obszar Jądra — przegląd człowieka.**

| Moduł | Co robi |
|---|---|
| `mode` | wybór trybu: usługa (`%ProgramData%\Alfa\broker\broker.json`) → tryb przenośny (`alfa-broker.exe` obok aplikacji) → brak binarek; uszkodzona konfiguracja usługi **nie** przechodzi na tryb przenośny |
| `link` | klient IPC roli `Core`: sprawdzenie serwera potoku (PID procesu potomnego / sesja 0 + konto usługi), powitanie po tożsamości obrazu, wątek połączenia, limity czasu, zerwanie = bezpieczny stan |
| `remote` | `RemoteBroker` (`Broker` nad łączem; fail-closed: `AuditUnavailable`, odrzucony token, sesja skażona, L0), `RemoteKill` (drzewa narzędzi + cisza audio + `KillAll`) |
| `window`, `status` | `ApprovalWindow` i `BrokerStatusView` dla UI (komenda `broker_status`, zdarzenie `BrokerStatus`), zdrowie modułu `safety-broker` |
| `supervise`, `children`, `notice`, `kernel` | nadzór (heartbeat, ponowne łączenie, ponowne uruchomienie Brokera 1 s → 30 s), procesy potomne z linią życia na stdin, komunikaty `alfa-watchdog` (gotowość, kill-switch), `KernelProcesses::start` dla powłoki |
| `inproc` | Broker w procesie (build deweloperski, Linux/CI) — przeniesiony z `app-core` |

Wybór w `AppOptions::kernel` (`app-core`): `None` = Broker w procesie (testy/dev), `Some(RemoteKernel)` = Broker
poza procesem albo `RemoteKernel::unavailable` (release bez izolowanego Brokera — przegląd CX-b).

Testy (`cargo test -p app-broker`): `contract_ipc.rs` (kontrakt Brokera przez IPC z udokumentowanym odstępstwem),
`scenario.rs` (zgoda w Broker-UI → narzędzie wykonane, odmowa, wygaśnięcie, zerwanie łącza, kill-switch),
`kernel.rs` (tryby startu, awaria Brokera, watchdog, podstawiony serwer), `e2e_unix.rs` (gniazda Unix 0600).
Kod Windows (`children.rs`: `CREATE_NO_WINDOW`) — `cargo clippy --target x86_64-pc-windows-msvc` nie buduje się na
Linuksie (zależność `app-api` → SQLCipher/OpenSSL); sprawdza go job CI Windows.
