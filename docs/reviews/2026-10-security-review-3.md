# Przegląd bezpieczeństwa 2026-10 #3 — model-recenzentka

Data: 2026-10-06. Gałąź: `ccr-af4b63c6-3fyzaj` (po `8660bb6`). Recenzentka: drugi model w świeżym kontekście
(AGENTS.md, krok 6 — poprawność, bezpieczeństwo, zgodność ze SPEC). Punkty odniesienia: `docs/THREAT_MODEL.md`,
`docs/PLAN.md` §7–§10, przeglądy #1 i #2 (`docs/reviews/2026-10-security-review-{1,2}.md` — ustaleń zamkniętych
nie powtarzam), SPEC-i modułów.

## 1. Zakres i metoda

Kod dodany od przeglądu #2 (`efc8864..8660bb6`), czytany „jak napastnik” (agentka z wstrzykniętym poleceniem,
złośliwa strona/plik/wtyczka/serwer, proces tego samego konta, restart procesów Jądra):
`app-broker` (łącze IPC roli `Core`, tryby usługa/przenośny/w procesie, nadzór, linia życia, komunikaty
watchdoga), zmiany w `app-safety` (`--lifeline`, `--broker-pid`, sygnał rollbacku), `safety-broker-impl/src/approvals.rs`
i `broker-ui-impl/src/driver.rs` (Q-9), powłoka `apps/desktop/src-tauri/src/{kernel,shortcuts,lib}.rs`,
`app-plugins` (host, sieć), `plugin-runtime-*`, `app-models` (pobieranie, TOFU, rozpakowanie, ścieżki),
`lib-embed` (manifest, ładowanie, tokenizer na treści niezaufanej), `app-voice` F5 (słowa wywoławcze,
weryfikacja mówcy, dyktowanie, czytanie), `tools-office`/`tools-browser`/`platform-windows-office-impl` (COM,
CDP), `mcp-*` v1, `updater-*` (pełny cykl) i `app-updates`.

Każde ustalenie „naprawione” ma test, który **przed poprawką nie przechodził** (uruchomiony na kodzie sprzed
poprawki — dla `app-broker` przez tymczasowe przywrócenie plików z `HEAD`), a po niej przechodzi. Propozycje
(P3-xx) mają test `#[ignore]`, który dziś pada i pokazuje problem (`cargo test … -- --ignored`), albo — gdy
dotyczą FFI Windows bez maszyny Windows — opis z analizy. Wagi: skala CVSS-podobna 0–10 (wektor lokalny,
agentka z wstrzykniętym poleceniem albo złośliwa treść).

## 2. Ustalenia

| Id | Moduł | Waga | Opis | Reprodukcja | Status |
|---|---|---|---|---|---|
| SR3-01 | safety-broker-contract (polityka Jądra) | 6,0 średnia | **Właściwy plik aplikacji poza listą procesów Jądra Brokera.** `PROTECTED_PROCESSES` znało `alfa.exe`/`alfa-core.exe`, a aplikacja to `alfa-desktop.exe` (`updater_contract::APP_EXE`); brakowało też `alfa-updater.exe` i `alfa-mcp-proxy.exe` (lista platformy `PROTECTED_IMAGES` je ma). Skutki: (1) leksykalny strażnik powłoki przepuszczał `taskkill /f /im alfa-desktop.exe`, `Stop-Process -Name alfa-desktop` — zabicie aplikacji kończy też Broker trybu przenośnego (linia życia; utrata stanu — SR3-03), a zabijanie cykliczne (np. z Harmonogramu zadań) to pętla awarii → watchdog/launcher wycofuje wersję (powrót do wydania bez późniejszych poprawek); (2) `gui.control(alfa-desktop.exe)` nie było twardą blokadą Brokera (L4 — token bez pytania; chronił tylko strażnik celów platformy — obrona w głąb utracona). | `safety-broker-contract/tests/review.rs` (3 testy: zgodność z `PROTECTED_IMAGES`, `GuiControlOfKernelProcess`, `KillSwitchDisable`) — przed poprawką 3/3 FAILED | naprawione (`policy.rs`: nadzbiór `PROTECTED_IMAGES`, 9 obrazów; koordynatorka: osobna kopia listy w `platform-windows-impl` — `DEFAULT_PROTECTED_PROCESSES` strażnika okien bez `alfa-core.exe`, helpera `uiAccess` i `alfa-mcp-proxy.exe` — zastąpiona `PROTECTED_IMAGES`). **Ścieżka Jądra — przegląd człowieka** |
| SR3-02 | app-plugins (`net.get` wtyczek) | 3,7 niska | **Adresy niepubliczne w nieskanonicznym zapisie IP.** `https_host` dzielił adres tekstowo, a klient (`reqwest`, parser WHATWG) zamienia `2130706433`, `0x7f000001`, `0x7f.1`, `0177.0.0.1`, `127.1`, `0`, `10.1`, `2852039166` (169.254.169.254) na literał IP i łączy się z nim **bez** resolvera `PublicOnly` (resolver dotyczy tylko nazw). `is_public_ip` nie znał też IPv4 zgodnego (`::7f00:1`) i NAT64 (`64:ff9b::a00:1`). Wtyczka z tokenem `net.egress("2130706433")` (wygląda jak domena) łączyła się z 127.0.0.1:443 — SSRF do usług lokalnych (ograniczone przez HTTPS i weryfikację certyfikatu). Wzór klienta dla `tools-net` (F6). | `app-plugins/tests/review.rs` (4 testy; przed poprawką 4/4 FAILED — klient zgłaszał „error sending request”, czyli próbował połączenia) | naprawione: host z parsera klienta (`reqwest::Url`), literał IP po kanonizacji sprawdzany `is_public_ip`; IPv4 osadzony (`to_ipv4`, NAT64, lokalny NAT64) — `net.rs`. Agentka A buduje `lib-netguard` z tą samą regułą — po migracji `app-plugins` test zostaje jako regresja |
| SR3-03 | app-broker (Broker poza procesem) | 6,5 średnia | **Restart Brokera otwiera zabezpieczenia (fail-open).** Broker trzyma skażenie sesji (`SessionSecurity`) i poziomy autonomii wyłącznie w pamięci, a nadzór `app-broker` sam uruchamia ponownie proces trybu przenośnego po awarii („bezpiecznie — agentki proszą od nowa”), usługę restartuje SCM. Po restarcie: sesja, która czytała wstrzyknięcie, przestaje być skażona — `TaintedEgress` („także na L4”, §7) nie obowiązuje, wysyłka na hosty z allowlisty przechodzi bez zgody; obniżenie właściciela („panika” L0) wraca do L3. Wyzwalacz: awaria, zabicie procesu (SR3-01), restart usługi. | `app-broker/tests/review.rs`: `session_taint_survives_broker_restart`, `owner_lowering_survives_broker_restart` — na kodzie z `HEAD` 2/2 FAILED (taint utracony, poziom L3 zamiast L0) | naprawione w `app-broker` (nie Jądro, ale ścieżka bezpieczeństwa — **do przeglądu człowieka**): dziennik stanu zawężającego `replay::Journal` (skażenie zgłoszone/obserwowane, także z `decide` na treści niezaufanej; obniżenia właściciela poniżej L3), odtwarzany w każdym nowym połączeniu **przed** udostępnieniem go wołającym (`link.rs::open`); błąd strumienia przy odtwarzaniu = połączenie nieudane. Źródło prawdy po stronie Brokera — P3-01 |
| SR3-04 | platform-contract (`KeyChord::system_scope`) / powłoka Tauri | 5,0 średnia | **Agentka uruchamia globalne skróty Alfy.** Powłoka rejestruje `Ctrl+Alt+Space`, `Ctrl+Alt+D`, `Ctrl+Alt+R` przez `tauri-plugin-global-shortcut` (`RegisterHotKey`), **nie** przez `HotkeyPort` z filtrem pochodzenia z P2-04 — poprawka P2-04 („wymuszone w `platform-windows-impl`”) w aplikacji nie działa. `input_keys` agentki do dowolnego okna (np. Notatnika z `gui.control`) wysyła `Ctrl+Alt+D` → Alfa włącza dyktowanie z mikrofonu do okna na pierwszym planie, a agentka czyta potem transkrypcję rozmowy w pokoju (prywatność audio, aktywo §2); `Ctrl+Alt+R` — czytanie na głos; `Ctrl+Alt+Space` — okno Alfy na wierzch. | `tools-input-impl/tests/review.rs::alfa_global_shortcuts_are_never_sent_by_agents` — przed poprawką FAILED („wysłano 6 zdarzeń do notepad.exe”) | naprawione po stronie nadawcy: `system_scope` odmawia `Ctrl+Alt+{Space,D,R}` (skrót globalny i tak nie dociera do aplikacji docelowej); `keys.rs` (+ testy jednostkowe). Odbiorca — P3-03 |
| SR3-05 | mcp-impl (serwer MCP Alfy dla mostów) | 4,0 średnia | **Schowek dla mostu CLI bez redakcji.** Każde zadanie mostu dostaje `BridgeScope::windows_v1` (v0 + v1); narzędzia v1 idą przez Brokera, ale v0 `clipboard_read` oddawało modelowi chmurowemu (Claude Code/Codex) treść schowka bez `redact_secrets` i ścieżki plików bez filtra poświadczeń — inaczej niż narzędzie schowka agentek (`tools-clipboard`). Klucz API skopiowany przez właściciela trafiał do dostawcy mostu (S25, S28). | `mcp-impl/tests/review.rs` (2 testy; przed poprawką 2/2 FAILED — klucz `sk-ant-…` i hasło w wyniku) | naprawione: redakcja sekretów i pominięcie ścieżek `is_credential_path` w `clipboard_read` (`tools.rs`). Brama Brokera dla v0 — P3-07 |
| P3-01 | safety-broker (stan) | 6,5 | Trwałość stanu bezpieczeństwa w Brokerze: skażenie sesji, poziomy autonomii, zmiany polityk zatwierdzone w Broker-UI, „zawsze zezwalaj” — dziś tylko pamięć procesu. SR3-03 łata to po stronie aplikacji, ale dziennik aplikacji ginie razem z nią (restart aplikacji = nowy Broker trybu przenośnego i pusty dziennik; sesje rozmów z niezaufaną treścią są trwałe w `sessions`). | analiza (`state.rs::State::new`, `AutonomyTable::default`) | propozycja: zapis w katalogu prywatnym Brokera (DACL konta usługi, łańcuch jak Audyt) i odtworzenie przy starcie; skażenie także w metadanych sesji (`sessions`) jako drugie źródło. **Ścieżka Jądra — decyzja człowieka** |
| P3-02 | app-broker / platform-windows-kernel-impl (wybór trybu) | 5,0 | Tryb usługi jest wybierany po **istnieniu** `%ProgramData%\Alfa\broker\broker.json`, a ścieżka bierze się ze zmiennej środowiskowej procesu (`KernelSetup::system`). Zmienna użytkownika `ProgramData` (HKCU\Environment — bez administratora, także `setx`/narzędziem środowiska z `tools-system`) przesłania systemową: po restarcie aplikacja nie widzi konfiguracji usługi i **po cichu** przechodzi w tryb przenośny (Broker na koncie użytkownika, Broker-UI bez UIPI) — dokładnie to obniżenie izolacji, którego `mode.rs` miał nie dopuszczać (baner pokazuje „przenośny”, ale nie blokuje). | `app-broker/tests/review.rs::service_config_location_does_not_follow_user_environment` (`#[ignore]`, pada: `SERVICE_CONFIG=<katalog z env>/Alfa/broker/broker.json`) | propozycja: `SHGetKnownFolderPath(FOLDERID_ProgramData)` albo wykrycie usługi `AlfaBroker` w SCM (zarejestrowana usługa = tylko tryb usługi albo bezpieczny stan); FFI w `platform-windows-kernel-impl` — decyzja człowieka |
| P3-03 | powłoka Tauri (`shortcuts.rs`) | 4,0 | Skróty powłoki bez filtra pochodzenia (P2-04 działa tylko dla `HotkeyPort`). SR3-04 blokuje wysyłanie przez `tools-input`, ale nie `SendKeys`/`SendInput` z powłoki agentki (P-03). | analiza | propozycja: skróty aplikacji przez `HotkeyPort` (`WinHotkeys` z hookiem LL, `HotkeyPressOrigin::admits`) zamiast `tauri-plugin-global-shortcut`; lista w `system_scope` z konfiguracji skrótów, gdy staną się konfigurowalne |
| P3-04 | app-models (sidecary) | 5,5 | Pliki wykonywalne sidecarów (`llama-server` ×2, `whisper-server`, `piper` — archiwa z wydań GitHub, uruchamiane potem przez Alfę) bez przypiętego SHA-256 — tylko zgoda TOFU na policzony skrót, którego właściciel nie ma czym sprawdzić (S23). Pozycje `confirmed: false`, ale pobieranie nie jest blokowane. | `app-models/tests/review.rs::executable_sidecars_are_pinned` (`#[ignore]`, pada: 4 pozycje) | propozycja: przypiąć skróty potwierdzonych wydań (bramka człowieka) albo odmawiać pobierania nieprzypiętych plików wykonywalnych; przy uruchomieniu sidecara — porównanie z rekordem instalacji |
| P3-05 | tools-browser / tools-office | 3,5 | Pobrania przeglądarki w kwarantannie (`allowAndName`, nazwa = GUID) — czy Chromium nadaje im `Zone.Identifier` w trybie CDP/headless, nie jest sprawdzone; `tools-office` decyduje o Protected View wyłącznie po ADS (`zone_of_path`: brak strumienia = lokalny). | analiza (bez Windows) | propozycja: test self-hosted (pobranie → MOTW); pliki z katalogu kwarantanny przeglądarki zawsze jako `FileZone::Internet` (fail-closed) |
| P3-06 | updater-impl (`selfupdate.rs`) | 3,0 | `alfa.exe.new.sha256` jest liczony z pliku w `versions\<ver>` i zapisywany obok przez ten sam proces — chroni przed uszkodzeniem, nie przed podmianą (oba pliki w `%LOCALAPPDATA%\Alfa`); część P-08. | analiza | propozycja: skrót launchera z podpisanej paczki (manifest skrótów w paczce objęty minisign) albo Authenticode przy samoteście |
| P3-07 | mcp-impl (narzędzia v0) | 3,5 | `clipboard_read/write`, `windows_list/focus` dla mostu nie przechodzą przez Brokera (bez `gui.control`, bez zgłoszenia taintu), a ochrona okien Alfy to `is_protected_process` po nazwie procesu (bez `TargetGuard`, P2-01 — wyskakujące okna WebView2/UWP). | analiza (SPEC `mcp`: „do ujednolicenia z v1”) | propozycja: v0 przez `BrokerGate` jak `tools-clipboard`/`tools-window` (zdolność `gui.control(clipboard)`), strażnik celów z `platform-contract` |

Przejrzane bez ustaleń:
- `approvals.rs` (Q-9): odmowa właściciela obowiązuje także przy błędzie Audytu, zgoda bez zapisu w Audycie nie
  wydaje tokenu (prośba czeka, nonce bez zmian — rozstrzyga tylko rola Broker-UI); `driver.rs`: tylko jawna odpowiedź
  Brokera (`Rejected`) zamyka kartę odmowy, błąd łącza wraca do pętli (karta nie znika); `ChannelLink` w procesie.
- `app-broker`: sprawdzenie serwera potoku przed powitaniem (PID procesu potomnego / sesja 0 + integralność
  systemowa + konto usługi), limity czasu = łącze zerwane, fail-closed (`AuditUnavailable`, taint + dane prywatne,
  L0), zmiany autonomii/polityk agentki odrzucane po stronie aplikacji, uszkodzona konfiguracja usługi nie przechodzi
  w tryb przenośny, build release bez izolowanego Brokera = stan „brak” (`kernel.rs::fallback`), kill-switch lokalnie
  przed Brokerem; `app-safety`: `--broker-pid` dla watchdoga, linia życia na stdin.
- `plugin-runtime-*`: wasmtime z paliwem, epokami, stosem, bez WASI i bez propozycji Wasm poza komponentami,
  kontrola importów/eksportów, nowy `Store` na wywołanie, limiter pamięci/tabel/instancji, operacje hosta przez
  `BrokerGate` za agentkę wywołującą z `untrusted_input_in_args`, `verify` w hoście, wynik niezaufany + taint;
  `app-plugins`: `verify` przed każdą operacją, deny-lista ścieżek, zapis przez dziennik cofania.
- `app-models`: rozpakowanie (ścieżki `validate_package_path`, dowiązania, duplikaty bez wielkości liter, limity
  wpisów/rozmiaru/stopnia kompresji, rozmiar rzeczywisty ≤ zadeklarowany, katalog roboczy + zamiana atomowa),
  pobieranie (tylko `https://`, przekierowania tylko na `https://`, twardy limit), zgoda TOFU wiąże pokazany skrót
  z policzonym, identyfikatory i ścieżki katalogu sprawdzane (`is_safe`).
- `lib-embed`: manifest (ścieżki względne bez `..`, SHA-256 liczony na tych samych bajtach, które są parsowane),
  tekst przycinany do `max_bytes` przed tokenizacją (Viterbi ograniczony), identyfikatory tokenów ⊆ słownik.
- `app-voice` F5: dyktowanie przez `InputPort` (strażnik celów, odmowa w polach haseł, Enter w terminalach → spacja),
  czytanie na głos — tekst niezaufany tylko w pamięci zadania, tury głosowe w trakcie dyktowania/czytania nie idą do
  modelu, weryfikacja mówcy tylko przy włączonym przełączniku, nasłuch słów tylko po uzbrojeniu.
- `platform-windows-office-impl`/`tools-office`: `AutomationSecurity = 3` ustawione i odczytane przed otwarciem,
  Protected View dla strefy Internet, kopia robocza (oryginał nigdy nie otwierany), formuły z listy dozwolonej (bez
  DDE, odwołań zewnętrznych, funkcji sieciowych), tekst komórek z apostrofem; `tools-browser`: CDP tylko przez potok,
  osobny profil Alfy, filtr egressu dla każdego żądania strony, Broker przy każdej akcji (taint bieżący).
- `mcp` v1: Broker przy każdym wywołaniu (podmiot „most CLI”, argumenty niezaufane, `unverified_by_alfa`), rejestr
  z deny-listą kluczy i redakcją; `updater`: minisign z wersją w komentarzu zaufanym, wersja ≤ bieżącej tylko z jawnego
  wycofania, ścieżki paczki, limity manifestu/paczki, przekierowania tylko `https://`, klucz i adres wydań wbudowane.
- Reguła AltGr: skróty powłoki `Ctrl+Alt+D`/`R`/`Space` — litery spoza listy; kill-switch nadal zastrzeżony
  (`system_scope`, `Hotkey::validate`).

## 3. Regresje i status poprzednich ustaleń

- **P2-04 (przegląd #2)** — poprawka istnieje w `platform-windows-impl` (`HotkeyPort`), ale skróty aplikacji
  rejestruje powłoka Tauri innym mechanizmem, więc w działającej aplikacji filtr pochodzenia nie obowiązuje (SR3-04,
  P3-03). Opis w `docs/modules/tools-input/SPEC.md` („`system_scope` bez zmian”) był nieaktualny — uzupełniony.
- Testy `tests/review.rs` z przeglądów #1 i #2 w crate'ach dotkniętych zmianami (`safety-broker-impl`,
  `tools-input-impl`, `platform-fake`) przechodzą.
- Bez zmian: P-01, P-03, P-05, P-08, P-09, P2-08, P2-10. P-09 (egress do sieci lokalnej) częściowo dotyka SR3-02 —
  host wtyczek odrzuca teraz adresy niepubliczne w każdym zapisie, polityka Brokera nadal nie.

## 4. Reguły THREAT_MODEL dla nowego kodu → testy

| Reguła / próg | Test(y) | Luka |
|---|---|---|
| S09/F8-05 Wasm | `plugin-runtime-impl/tests/plugins/*` (`load`, `exec`, `hostcalls`, `supply`), `app-plugins/tests/{host,app}.rs` | — |
| S14 SSRF/rebinding | **SR3-02** `app-plugins/tests/review.rs`; `lib-netguard` (agentka A, w toku) | P-09 (polityka Brokera) |
| S10/S21, fail-closed Brokera poza procesem | `app-broker/tests/{scenario,kernel,contract_ipc}.rs`, **SR3-03** `app-broker/tests/review.rs` | P3-01, P3-02 |
| S11/§7 `gui.control` wobec Alfy, zabicie Jądra | **SR3-01** `safety-broker-contract/tests/review.rs`, **SR3-04** `tools-input-impl/tests/review.rs` | P3-03 |
| S04/S06 głos (F5) | `app-voice/tests/f5_{dictation,read,speaker,wake}.rs` | — |
| S07/S08/S18 MCP v1 | `mcp-impl/tests/{v1,v1_registry,static_rules}.rs`, **SR3-05** `mcp-impl/tests/review.rs` | P3-07 |
| S23 łańcuch dostaw | `updater-impl/tests/{update_safety,update_flow,launcher_modes}.rs`, `app-models/tests/unpack.rs` | P3-04, P3-06 |
| S02/S26 Office, przeglądarka | `platform-apps-contract` (`tests.rs`, `tests_browser.rs`), `tools-office-impl/tests/office.rs`, `tools-browser-impl/tests/browser.rs` | P3-05 |

## 5. Bramki

Uwaga: w trakcie przeglądu agentki A, B i C tworzyły nowe crate'y — manifest workspace był chwilami
niekompletny (`app-files`, `tools-system-*`, `tools-net-*` bez `src/lib.rs`), więc bramki uruchamiałam
w oknach, gdy `cargo metadata` przechodziło. Pełne `cargo test --workspace` i `clippy --workspace` — koordynator.

| Bramka | Wynik |
|---|---|
| `rustfmt --check` (pliki tego przeglądu) | czysto |
| `cargo test` | zielono: `safety-broker-contract` (6 binarek, w tym `review` 3/3), `safety-broker-impl` (9 binarek, w tym `review` z #1), `app-broker` (7 binarek; `review` 2/2 + 1 `ignored`; jednostkowe 8 z testem `replay`), `app-plugins` (5 binarek; `review` 4/4), `mcp-impl` (9 binarek; `review` 2/2, `static_rules` zielone po zmianie danych testu), `tools-input-impl` (4 binarki; `review` 6/6 z #2 i #3), `platform-contract --lib` (53), `platform-fake` (10 binarek, w tym `review`/`review_contract` z #2) |
| `cargo test … -- --ignored` (propozycje) | zgodnie z oczekiwaniem padają: P3-02 (`app-broker`: konfiguracja usługi w `<katalog z env>/Alfa/broker/broker.json`), P3-04 (`app-models`: `sidecar-llama-vulkan`, `sidecar-llama-cpu`, `sidecar-whisper-cpu`, `sidecar-piper`) |
| `cargo clippy --all-targets -D warnings` (`app-broker`, `app-plugins`, `safety-broker-contract`, `platform-contract`, `mcp-impl`, `tools-input-impl`, `app-models`) | czysto |
| clippy `--target x86_64-pc-windows-msvc --all-targets -D warnings` | czysto: `safety-broker-contract`, `platform-contract`; `app-broker`, `mcp-impl`, `tools-input-impl` — niemożliwe w tym środowisku (zależności budują `openssl-sys`/`sqlite-vec`, brak `perl`), zmiany są niezależne od systemu |
| `scripts/check-deps.sh` | OK (1552 krawędzie, zero naruszeń) |
| `cargo deny check` | nie dotyczy — bez zmian zależności (`reqwest::Url` z istniejącego `reqwest`) |

## 6. Zmienione i nowe ścieżki (do commita przez koordynatora)

Zmienione: `crates/safety-broker-contract/src/policy.rs`, `crates/app-plugins/src/net.rs`,
`crates/app-broker/src/{lib.rs,link.rs,remote.rs}`, `crates/platform-contract/src/keys.rs`,
`crates/mcp-impl/src/tools.rs`, `crates/tools-input-impl/tests/review.rs` (test dopisany na końcu),
`docs/modules/{safety-broker,mcp,tools-input,plugin-runtime,models}/SPEC.md` (sekcja „Przegląd bezpieczeństwa #3”).

Nowe: `crates/app-broker/src/replay.rs`, `crates/app-broker/tests/review.rs`,
`crates/safety-broker-contract/tests/review.rs`, `crates/app-plugins/tests/review.rs`,
`crates/mcp-impl/tests/review.rs`, `crates/app-models/tests/review.rs`,
`docs/reviews/2026-10-security-review-3.md`.

Bez zmian w `Cargo.toml`/`Cargo.lock`, `deny.toml`, `crates/README.md`.

## 7. Do decyzji człowieka

1. **SR3-01** — zmiana listy procesów Jądra w polityce Brokera (`safety-broker-contract`, ścieżka Jądra):
   przegląd i zatwierdzenie.
2. **SR3-03** — odtwarzanie stanu zawężającego przez aplikację po restarcie Brokera (`app-broker`): semantyka
   (odtwarzane tylko skażenie i obniżenia poniżej L3; obniżenie odtworzone po późniejszym podniesieniu daje stan
   ostrzejszy) i **P3-01** — trwały stan w Brokerze jako docelowe źródło prawdy.
3. **P3-02** — wybór trybu usługi niezależny od środowiska procesu (Known Folder / SCM) — FFI w ścieżce Jądra.
4. **P3-03** — przeniesienie skrótów powłoki na `HotkeyPort` z filtrem pochodzenia (P2-04 w aplikacji).
5. **P3-04** — sidecary wykonywalne tylko z przypiętym SHA-256 (bramka potwierdzenia wydań).
6. P3-05…P3-07 — weryfikacja MOTW na self-hosted, skrót launchera z podpisanej paczki, Broker dla narzędzi MCP v0.
7. Nadal otwarte z #1/#2: P-01, P-03, P-05, P-08, P-09, P2-08, P2-10.
