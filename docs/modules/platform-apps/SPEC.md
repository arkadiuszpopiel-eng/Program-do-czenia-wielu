# platform-apps — SPEC (v1: porty zaimplementowane, F6)

## Cel
Porty aplikacji i systemu dla computer use (PLAN §7.1 „API/CLI/COM > UIA > wizja > wejście”, §7.2): **Office przez COM** (Word/Excel), **izolowana przeglądarka** z CDP przez potok i **rejestr tylko do odczytu**. Nie zawiera narzędzi agentek (te są w `tools-office`, `tools-browser`, serwerze MCP v1) ani decyzji Brokera — porty egzekwują niezmienniki, których nie da się obejść z wyższej warstwy.

## Fala i priorytet
F6, P1. Osobny crate kontraktu (`platform-apps-contract`), bo `platform-contract` ma ~7 800 linii przy limicie 8 000 na crate.

## Kontrakt (`platform-apps-contract` — źródło prawdy)
```rust
trait OfficePort   { available(app); zone_of(path) -> FileZone; read(&OfficeFile, &OfficeQuery) -> OfficeContent;
                     edit(&OfficeFile, &[OfficeEdit]) -> OfficeEdited /* nowe bajty kopii */ }
trait BrowserPort  { open(&BrowserSpec, Arc<dyn EgressFilter>) -> BrowserSessionId; navigate; snapshot(max_nodes, max_text);
                     click(node); type_text(node, text, submit); screenshot(max_side) -> png; close }
trait EgressFilter { allows(host) -> bool }            // wołany dla KAŻDEGO żądania strony
trait RegistryPort { list(&RegKey, max) -> RegListing; read_value(&RegKey, name) -> RegValue }   // bez zapisu
// F6 `tools-system` / `tools-net`:
trait SysPort      { guard() -> &TargetGuard; processes(); process(pid); terminate(&ProcessIdentity);
                     services(); control_service(name, Start|Stop|Restart, timeout); events(&EventQuery);
                     env(Process|User|Machine); user_env_value(name); set_user_env(name, Option<value>) -> poprzednia }
trait DownloadStore { begin(dir, name) -> DownloadSink }   trait DownloadSink { write(chunk); commit(source_url) -> PathBuf }
```
Pomocniki wspólne dla atrapy i implementacji: `check_session` (dowód `AutomationSecurity = 3`), `check_edits`, `check_formula` (lista dozwolona), `safe_cell_text`, `FileZone::from_zone_identifier`, `BrowserSpec::validate`, `is_user_browser_profile`, `chromium_args`, `profile_preferences`, `check_navigation_url`, `request_allowed`, `RegKey::parse`, `check_key`, `guard_listing`, `RegValue::guarded`.

## Zależności
`platform-contract` (`PlatformError`), `serde`, `serde_json`, `thiserror`. Implementacja Windows: `platform-windows-office-impl` (windows-rs — `deny.toml`: `wrappers`); atrapa: `platform-apps-fake`.

## Niezmienniki
- **Office:** port dostaje bajty, nie ścieżkę oryginału — pracuje na kopii roboczej w prywatnym katalogu; `AutomationSecurity = msoAutomationSecurityForceDisable` ustawione **i odczytane** przed otwarciem (inaczej odmowa); każdy wynik niesie `OfficeSession`, wynik bez dowodu jest odrzucany (fail-closed). Plik z MOTW (strefa 3/4, nieczytelny znacznik = Internet) — tylko `ProtectedViewWindows.Open`, edycja → `ProtectedView`. Hasła-atrapy przy otwarciu (dokument chroniony = błąd, nie okno). Excel: tryb obliczeń ręczny przed otwarciem, bez zdarzeń i aktualizacji łączy; instancja użytkownika odrzucana. Formuły tylko z listy dozwolonej (bez `|` DDE, `[`/`\`/URL, `WEBSERVICE`, `IMAGE`, `HYPERLINK`, `INDIRECT`, `INFO`, `CALL`, `PY`…); tekst od `=`/`+`/`-`/`@` dostaje apostrof.
- **Przeglądarka:** profil i kwarantanna wewnątrz katalogu Alfy, rozłączne, nigdy profil Chrome/Edge/Firefox/Brave/… użytkownika; CDP wyłącznie `--remote-debugging-pipe`; `Default/Preferences` bez menedżera haseł i autouzupełniania; każde żądanie przez filtr (`Fetch.requestPaused` rozstrzygany w wątku czytającym), WebSockety zablokowane, cele potomne wstrzymane do włączenia przechwytywania (błąd → zamknięcie celu); pobrania `allowAndName` do kwarantanny; pola `type=password` bez wartości i bez wpisywania; Job Object `KILL_ON_JOB_CLOSE`.
- **System (`SysPort`):** zakończenie procesu tylko przez uchwyt otwarty po PID-zie po ponownym sprawdzeniu obrazu i czasu startu (`ProcessIdentity`), strażnika celów (pełna ścieżka, świeże drzewo przodków — Alfa i jej potomkowie, Broker, watchdog, katalogi Alfy), właściciela (SID tokenu = bieżący użytkownik) i podniesienia; PID ≤ 4 i procesy krytyczne (`CRITICAL_PROCESSES`) — nigdy; zatrzymanie usług krytycznych (`CRITICAL_SERVICES`, w tym `alfabroker`, `alfawatchdog`, `eventlog`, Defender, zapora) — odmowa portu; Dziennik tylko `Application`/`System`, XPath z wartości sprawdzonych (`check_provider`); zmienne o nazwach sekretów ukryte (`guard_env`), zapis tylko `HKCU\Environment` z deny-listą nazw (`env_write_denied`: ścieżki Alfy, interpreter, proxy, TLS, profilery .NET, WebView2, `ALFA*`).
- **Kwarantanna (`DownloadStore`):** nazwa oczyszczona (`sanitize_file_name`), katalog tworzony komponent po komponencie dopiero, gdy najgłębszy istniejący przodek ma ścieżkę rzeczywistą równą podanej (dowiązanie/junction → odmowa, nic nie powstaje po drugiej stronie), plik `.part` jako nowy, MOTW (`ZoneId=3`, `HostUrl` bez CR/LF) przed nadaniem nazwy, nazwa końcowa dowiązaniem twardym bez nadpisania, porzucenie usuwa część.
- **Rejestr:** tylko `HKCU`/`HKLM`; deny-lista segmentów (Credentials, Protected Storage, SAM, SECURITY, Secrets, Lsa, Winlogon, IntelliForms, IdentityCRL, Cryptography, SystemCertificates, PuTTY/WinSCP/VNC/TeamViewer, Outlook, Alfa…) i fragmentów (`password`, `token`, `secret`, `credential`, `apikey`…) sprawdzana **przed** otwarciem klucza i dla podkluczy listy; wartości o nazwach sekretów → `Redacted`; ścieżka sprawdzana = ścieżka otwierana.

## Zdolności / uprawnienia
Brak własnych — decyzje Brokera podejmują narzędzia (`tools-office`: `fs.*` + `gui.control`; `tools-browser`: `net.egress(host)`; MCP v1: `system.admin(reg query)`; `tools-system`: `gui.control(system-info.exe)`, `gui.control(<obraz>)`, `system.admin`; `tools-net`: `net.egress(host)`, `fs.write(kwarantanna)`). Implementacja Windows `SysPort`/`DownloadStore`: `platform-windows-sys-impl` (`WinSys`, `DiskDownloads`).

## Izolacja
`inproc`; Office: dedykowany wątek STA z limitem czasu (porzucanie wiszących, ≤ 2 naraz); przeglądarka: osobny proces w Job Object + wątek czytający CDP.

## Budżet zasobów
Office ≤ 60 s na operację, plik ≤ 64 MiB, ≤ 10 000 komórek, ≤ 200 edycji; CDP ≤ 30 s na polecenie, wiadomość ≤ 64 MiB, ≤ 4 przeglądarki; rejestr ≤ 500 wpisów, napis ≤ 4 096 znaków.

## Testy akceptacyjne
`platform-apps-contract` (11 testów + property: klucz z sekretami w dowolnej pisowni → odmowa), `platform-apps-fake` (makra nigdy nie uruchomione, Protected View, filtr egressu, kwarantanna), `platform-windows-office-impl` (pełna sesja CDP na skryptowanej przeglądarce, dekodowanie rejestru, kopia robocza), `tests/live_windows.rs` — Word/Edge/rejestr na żywo `#[ignore]` (self-hosted Windows z Office i Edge).

## Fake
`FakeOffice` (dokument w pamięci, dziennik `AutomationSecurity`, licznik makr — uruchamiają się tylko przy złym ustawieniu), `FakeBrowser` (wirtualna sieć: zasoby, linki, formularze, pobrania; dziennik żądań z decyzją filtra), `FakeRegistry` (licznik surowych odczytów kluczy z sekretami — musi być 0), `FakeSys` (procesy z drzewem i strażnikiem, usługi, zdarzenia, zmienne; dziennik `terminate`/`control_service`/`set_user_env`, podmiana procesu tuż przed zakończeniem — wyścig PID), `FakeDownloads` (pliki w pamięci ze znacznikiem strefy, katalogi-dowiązania, licznik porzuconych zapisów).

## Otwarte pytania
- Zabicie zawieszonego `WINWORD.EXE`/`EXCEL.EXE` po limicie (dziś wątek porzucany, proces zostaje).
- Ruch spoza `Fetch` (WebRTC/UDP, WebTransport, DNS prefetch) — flagi ograniczają, pełna kontrola wymaga lokalnego proxy egressu (helper).
- Okno wyścigu dziedziczenia uchwytów potoku przy równoległym `CreateProcess` spoza Alfy (lista `HANDLE_LIST` ogranicza do naszego dziecka).
