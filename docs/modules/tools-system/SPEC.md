# tools-system — SPEC (v1: narzędzia zaimplementowane, F6)

## Cel
Narzędzia agentek do „Procesy i usługi” oraz „System i ustawienia” z PLAN §7.2 (część odczytowa i najczęstsze zmiany): procesy (lista, szczegóły, zakończenie), usługi (lista/stan, start/stop/restart), Dziennik zdarzeń (odczyt z filtrem), zmienne środowiskowe (odczyt; zapis użytkownika z cofaniem), zasilanie/wyświetlacze/audio (odczyt). Rejestr — serwer MCP v1 (`RegistryPort`); zapora, sterowniki, winget, Windows Update — v2.

## Fala i priorytet
F6, P1.

## Kontrakt
```rust
system_processes { name_contains?, limit? } → { processes: [ProcOut{pid, parent_pid, name, own, protected}], total, truncated }
system_process_info { pid } → ProcDetailsOut { pid, name, path?, own, elevated?, session_id?, started_ms?, threads, memory_kb?, protected }
system_process_kill { pid, name } → { pid, name }            // `name` = obraz z listy (ochrona przed ponownym użyciem PID)
system_services { name_contains?, state?, limit? } → { services: [ServiceOut{name, display_name, state, pid?}], total, truncated }
system_service_control { name, action: start|stop|restart } → ServiceOut
system_events { log: application|system, level?, provider?, since_hours?, max? } → { events: [EventOut{time_ms, level, provider, event_id, message}], truncated }
system_env { scope: process|user|machine, name_contains? } → { vars: [EnvOut{name, value?, hidden}] }
system_env_set { name, value? /* brak = usuń */ } → { name, previous_set, undo_id }   // tylko zmienne użytkownika
system_status {} → { power?, displays: [..], audio: [..] }
pub struct SystemTools; impl SystemTools { new(SystemToolsDeps{ sys: SysPort, power?, desktop?, hardware?, guard: TargetGuard, broker, config, bus });
                                           undo_env(id) -> Result<(), EnvUndoError> /* karta „Cofnij” */ }
```
Port `SysPort` (`platform-apps-contract`): `processes`, `process(pid)`, `terminate(pid, &ProcessIdentity)`, `services`, `control_service`, `events(&EventQuery)`, `env(scope)`, `set_user_env(name, value) -> poprzednia`. Zdarzenia: `tool.system.process_killed`, `tool.system.service`, `tool.system.env_set` (bez wartości zmiennych).

## Zależności
`tools-common-contract`, `platform-apps-contract` (`SysPort`, polityki), `platform-contract` (`TargetGuard`, `PowerPort`, `DesktopPort`, `HardwarePort`), `safety-broker-contract`, `core-bus-contract`. Windows: `platform-windows-sys-impl::WinSys` (Toolhelp32, SCM, `EvtQuery`, `HKCU\Environment`); atrapa: `platform-apps-fake::FakeSys`.

## Niezmienniki
- **Każde wywołanie przez `BrokerGate`.** Odczyty: `gui.control(system-info.exe)` (pseudo-aplikacja do czasu `system.read` w Brokerze — jak `clipboard.exe`), fakty `touches_private_data`. Zakończenie procesu: `gui.control(<obraz celu>)` — Broker blokuje procesy Jądra (`GuiControlOfKernelProcess`) niezależnie od narzędzia; `destructive = permanent` (niezapisane dane), `reversible = no`. Usługi: `system.admin(service_control)` — zgoda przy każdej eskalacji, usługi Jądra (`alfabroker`, `alfawatchdog`, `eventlog`) to twarda blokada. Zapis zmiennej: `system.admin(setx …)` (reguły powłoki Jądra sprawdzają wartość).
- **Strażnik celów:** proces Alfy, jej drzewo (WebView2, sidecary, terminal), Broker, Broker-UI, watchdog, helper i obraz nieznany są chronione (`TargetGuard` + łańcuch przodków liczony przy każdym wywołaniu); procesy krytyczne systemu, procesy innych użytkowników i sesji — odmowa przed Brokerem. Port zabija przez uchwyt otwarty po PID-zie dopiero po ponownym sprawdzeniu obrazu i czasu startu (`ProcessIdentity`) — bez wyścigu z ponownym użyciem PID-u.
- **Usługi krytyczne** (Defender, zapora, BFE, RPC, DCOM, LSM, SAM, Winmgmt, Centrum zabezpieczeń, Windows Update…) — zatrzymanie/restart = odmowa narzędzia (start dozwolony).
- **Zmienne:** zapis tylko `HKCU\Environment`; deny-lista nazw przekierowujących dane Alfy lub wstrzykujących kod (`APPDATA`, `LOCALAPPDATA`, `USERPROFILE`, `TEMP`, `ComSpec`, `PATHEXT`, `PSModulePath`, `ALFA_*`, `WEBVIEW2_*`, `COR_*`, `DOTNET_*`, `*_PROXY`, `SSLKEYLOGFILE`, `SSL_CERT_*`, `NODE_OPTIONS`…); wartości zmiennych o nazwach sekretów (`*KEY*`, `*TOKEN*`, `*SECRET*`, `*PASSWORD*`, `*CREDENTIAL*`…) nie opuszczają portu (`hidden`), pozostałe redagowane. Cofnięcie przywraca poprzednią wartość tylko, gdy bieżąca jest wciąż tą zapisaną (konflikt = odmowa).
- **Dziennik zdarzeń:** tylko `Application` i `System` (nigdy `Security`), dostawca z listy znaków dozwolonych (bez wstrzyknięcia XPath), komunikaty redagowane i obcinane.
- **Wynik = treść niezaufana** (nazwy procesów, opisy usług, komunikaty zdarzeń): `untrusted = File`, taint sesji zgłaszany Brokerowi.

## Zdolności / uprawnienia
`gui.control(system-info.exe)` (odczyty), `gui.control(<obraz>)` (zakończenie), `system.admin` (usługi, zapis zmiennej). Grupy ról: `system`, `system.read`, `system.act`; rola tylko do odczytu dostaje wyłącznie odczyty. W obsadzie: Wykonawczyni (`system`).

## Izolacja
`inproc`, `lazy`; wywołania portu na wątku blokującym.

## Budżet zasobów
RAM ≤ 8 MB; ≤ 500 procesów/usług w wyniku, ≤ 200 zdarzeń, komunikat ≤ 2 000 znaków, wynik dla modelu ≤ 40 000 znaków; sterowanie usługą ≤ 30 s.

## Konfiguracja (klucze TOML)
`[tools.system] max_list = 200`, `max_events = 50`, `max_message_chars = 2000`, `output_max_chars = 40000`.

## Testy akceptacyjne
- `ACC-F6-tools-system-01`: zabicie procesu Alfy/Brokera/watchdoga/WebView2 Alfy → odmowa i zero wywołań `terminate` (także przy PID-zie z listy, aliasie 8.3, obrazie z katalogu Alfy); zabicie procesu użytkownika → token `gui.control(<obraz>)` + `terminate` z tożsamością (`tests/system.rs`).
- `ACC-F6-tools-system-02`: usługi Jądra i krytyczne — odmowa; start/stop zwykłej usługi tylko z tokenem `system.admin`.
- `ACC-F6-tools-system-03`: sekrety w zmiennych i zdarzeniach nigdy w wyniku; zapis zmiennej z deny-listy → odmowa; cofnięcie z wykrywaniem konfliktu.
- Windows na żywo: `platform-windows-sys-impl/tests/live_sys.rs` `#[ignore]` (self-hosted).

## Fake
`tools-system-fake` (manifesty, walidacja, wyniki skryptowane); testy impl na `platform-apps-fake::FakeSys` (procesy z drzewem, usługi, zdarzenia, zmienne, dziennik wywołań `terminate`).

## Otwarte pytania
- Zdolność `system.read(obszar)` w Brokerze zamiast pseudo-aplikacji (zmiana Jądra — przegląd człowieka).
- Start/stop usług wymagających administratora — UAC na żądanie przez Brokera (PLAN §8.4); dziś `PermissionDenied` z wyjaśnieniem.
- ~~Karta „Cofnij” dla zapisu zmiennej w UI~~ — zrobione (fala 4): wynik `system_env_set` niesie `UndoRef { service: UndoService::System, id: undo_id }` (`tests/undo_card.rs`), token UI `"<sesja>:v<id>"`, `turns_undo_step` → `AgentTools::undo_env` tylko dla kroku przebiegu tej sesji (`app-chat/src/undo.rs`).

## Przegląd fali 3 (2026-10, `docs/reviews/2026-10-wave3-review.md`) — polityka do przeglądu człowieka
- **W3-02 (zrobione):** deny-lista zapisu zmiennych uzupełniona o zmienne ładujące kod w innych ekosystemach: `JAVA_TOOL_OPTIONS`, `_JAVA_OPTIONS`, `JDK_JAVA_OPTIONS`, `OPENSSL_CONF`, `OPENSSL_MODULES`, `OPENSSL_ENGINES`, `PSExecutionPolicyPreference`, `PYTHONHOME`, `PERL5LIB`, `PERLLIB`, `RUBYOPT`, `RUBYLIB`, prefiksy `CARGO_`, `NPM_CONFIG_` (`platform-apps-contract/tests/review.rs`).
- **W3-05 (zrobione):** wartość zmiennej bez znaków sterujących (poza tabulatorem) — karta Brokera `setx NAZWA "wartość"` nie rozpada się na wiele linii.
- **W3-08 (decyzja człowieka):** `PATH` użytkownika pozostaje zapisywalny (świadomie, test `tests_sys::env_secrets_hidden_and_writes_denied`); propozycje: osobne potwierdzenie z pokazaniem dodanych/usuniętych katalogów albo tylko dopisywanie istniejących katalogów spoza obszarów zapisywalnych przez agentki.
