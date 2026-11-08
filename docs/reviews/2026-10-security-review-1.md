# Przegląd bezpieczeństwa 2026-10 #1 — drugi model-recenzent

Data: 2026-10-01. Gałąź: `ccr-af4b63c6-3fyzaj` (po `be4dcd5`). Recenzent: drugi model w świeżym kontekście
(AGENTS.md, krok 6 — poprawność, bezpieczeństwo, zgodność ze SPEC). Punkty odniesienia: `docs/THREAT_MODEL.md`,
`docs/PLAN.md` §1.3, §1.4, §8, §15.1, `docs/ACCEPTANCE.md` (F3, F4), SPEC-i modułów, `docs/compliance/*`.

## 1. Zakres i metoda

Przeczytane „jak napastnik”: `safety-broker-*` (decyzje, plany, zgody, autonomia, tokeny, IPC, Audyt, kill-switch,
usługa), `broker-ui-*`, `app-safety`, `watchdog-*`, `risk-classifier-*`, `tools-common-contract`, `tools-fs-*`,
`tools-shell-*` (+ strażnik poleceń), `tools-clipboard-*` (pobieżnie), `undo-journal-*` (pobieżnie), `transfer-*`
(ZIP, ścieżki, AEAD/Argon2), `mcp-*` (kanał lokalny, odciski), `agent-backends-*` (§1.3), `compliance-*`
(deny-listy, normalizacja), `platform-windows-impl` (`fs/guard*`, porty Jądra — FFI), `updater-*` (minisign,
crash-loop).

Każde ustalenie „naprawione” ma test reprodukujący, który **przed poprawką nie przechodził** (sprawdzone), a po
niej przechodzi; testy regresyjne są w plikach `tests/review.rs` odpowiednich crate'ów. Ustalenia bez reprodukcji
oznaczono „podejrzenie”; wymagające decyzji architektonicznej/człowieka — „propozycja”.

Wagi: skala CVSS-podobna 0–10 (wektor: lokalny, wymaga agentki z wstrzykniętym poleceniem albo złośliwej treści;
wpływ wg aktywów THREAT_MODEL §2).

## 2. Ustalenia

| Id | Moduł | Waga | Opis | Reprodukcja | Status |
|---|---|---|---|---|---|
| SR-01 | safety-broker | 7,6 wysoka | **Plan „pierze” reguły każdego poziomu.** Plan, w którym żaden krok nie wymagał zgody, był zapisywany jako zatwierdzony; plan zatwierdzony przed taintem pokrywał kroki później. Po oznaczeniu sesji jako `tainted` egress (reguły `TaintedEgress`/`Trifecta`, „także na L4”) przechodził przez plan bez potwierdzenia → eksfiltracja (S01, S03, §7). | `safety-broker-impl/tests/review.rs`: `auto_approved_plan_does_not_cover_tainted_egress`, `approved_plan_does_not_cover_rules_added_after_approval` | naprawione: plan zapisuje wyłącznie kroki pokazane właścicielowi wraz z regułami z karty; pokrycie wymaga, by każda reguła niepokrywalna „zawsze zezwalaj” z bieżącego werdyktu była na karcie (`state.rs::ApprovedStep::covers`, `decide.rs`) |
| SR-02 | safety-broker | 7,1 wysoka | **Agentka przedłuża/utrwala czasowe L4.** Ścieżka „obniżenia” przyjmowała poziom równy bieżącemu (`<=`), więc prośba agentki o L4 bez terminu na cel szczegółowy (albo z dłuższym terminem) utrwalała L4 właściciela „na czas” — podniesienie w czasie bez Broker-UI (S10). | `review.rs::equal_level_request_cannot_extend_timed_l4` | naprawione: obniżenie tylko ściśle niższe; równy poziom od agentki = `KernelBlock(SelfEscalation)`, od właściciela = prośba w Broker-UI; identyczny wpis = brak zmiany (`identical_autonomy_entry_is_a_no_op`) |
| SR-03 | safety-broker (IPC) | 6,5 średnia | **Klient roli `Agent` działał cudzym imieniem.** Serwer brał podmiot (`holder`, `presenter`, `requester`) z treści żądania: agentka o niższym poziomie dostawała tokeny przy poziomie innej agentki (np. L4), odbierała jej token z zatwierdzonej prośby, okazywała i unieważniała jej tokeny. | `review.rs::agent_client_is_bound_to_its_own_identity` | naprawione: dla roli `Agent` `holder.agent` musi równać się `client_id` z poświadczenia (`Decide`, `SubmitPlan`, `Verify`, `Attenuate`, `ApprovalStatus`, `Revoke` cudzego tokenu → `Unauthorized`; `ipc.rs::foreign_holder`) |
| SR-04 | compliance (+ Broker) | 7,4 wysoka | **Domena dostawcy zapisana znakami zgodności Unicode omijała deny-listę** (`ⅽlaude.ai` U+217D, pełna szerokość `ｃｌａｕｄｅ．ａｉ`, litery matematyczne). Klient HTTP (`url`) i WebView2 mapują IDNA/UTS 46 do `claude.ai`, a `normalize_host` porównywał postać niezmapowaną → `net.egress` do webowego UI dostawcy bez blokady (S16, §1.3 zas. 6). | `compliance-contract/tests/review.rs` (w tym proptest pełnej szerokości), `safety-broker-impl/tests/review.rs::provider_desktop_apps_are_hard_blocked` (egress) | naprawione: host z nie-ASCII → `idna::domain_to_ascii` (`idna 1.1.0`, ta sama wersja co `url`, już w `Cargo.lock`); niepoprawny IDN = brak hosta |
| SR-05 | platform-windows-impl (`fs/guard`) | 7,0 wysoka | **Dowiązanie/junction omijało deny-listę Jądra.** Postać kanoniczna (po rozwiązaniu dowiązań) jest sprawdzana tylko w `WinFs`, a ten znał wyłącznie znaczniki kontraktu platformy; wpisy listy Jądra spoza nich (`.claude.json`, `.gemini`, `.grok`, `.kimi`, `.agy`, `.npmrc`, `.pypirc`, profile przeglądarek, `%APPDATA%\gh`) były sprawdzane tylko leksykalnie w `tools-fs`/Brokerze. Junction w profilu (np. z obfuskowanego polecenia powłoki) → odczyt tokenu innego CLI (S15). | `platform-windows-impl/tests/review.rs` (`symlink_into_kernel_denylisted_store_is_denied`, `every_kernel_segment_is_denied_by_last_line`) | naprawione: domyślne `extra_deny_names`/`extra_deny_prefixes` obejmują całą bazową listę Jądra; test spójności z `DenyLists::baseline()` |
| SR-06 | agent-backends | 5,9 średnia | **Kopia katalogu roboczego mostu czytała i powielała tokeny CLI.** `copy_tree` kopiował `~/.claude`, `~/.codex`, `.ssh`, `.aws`… do katalogu Alfy, gdy źródłem był profil lub katalog z tymi wpisami — kod Alfy czytał i przechowywał tokeny (§1.3 zas. 1, F4-04 „0 odczytów”). | `agent-backends-impl/tests/review.rs::copy_skips_credential_stores` | naprawione: kopia pomija wpisy z bazowej deny-listy Jądra (`workspace.rs::CopyDeny`) |
| SR-07 | safety-broker (polityka bazowa) | 8,1 wysoka | **Pliki samej Alfy poza obszarem Jądra.** `kernel_paths` obejmowały tylko `%APPDATA%\Alfa\kernel` i `%LOCALAPPDATA%\Alfa\audit`. Agentka na L3 (zapis w profilu bez pytania) mogła podmienić `%LOCALAPPDATA%\Alfa\versions\<v>\*.exe` (launcher uruchomi; obraz w tej ścieżce przechodzi wiązanie roli `Core` w Brokerze — zapis po obrazie bez Authenticode, zob. P-08), `current.json`, profil WebView2, bazy, konfigurację `%APPDATA%\Alfa\config` (dostawcy/`baseURL`, MCP, budżety). Usunięcie przodka (`C:\`, `C:\Users\ala`) nie było blokowane, bo `fs_delete` prosi o zakres dokładny, a reguła działała tylko dla poddrzewa. | `review.rs::alfa_own_files_are_kernel_area` | naprawione: `kernel_paths` = katalog Brokera + `%APPDATA%\Alfa` + `%LOCALAPPDATA%\Alfa` (katalogi sesji `%USERPROFILE%\Alfa\Sesje` zostają w zasięgu); destrukcja katalogu zawierającego obszar Jądra albo `%SystemRoot%` = blokada niezależnie od rodzaju zakresu (`guard.rs::path_rule`) |
| SR-08 | platform-windows-kernel-impl (`win_sec`) | 6,0 średnia | **Przejęcie katalogu prywatnego Brokera.** Istniejący katalog dostawał tylko chroniony DACL; katalog założony zawczasu przez użytkownika (np. w `ProgramData`, gdzie każdy tworzy podkatalogi) albo junction zachowywał mu niejawne `WRITE_DAC` właściciela (lub DACL trafiał w cel junction) → Audyt i kotwica w zasięgu agentek (S21). | brak testu automatycznego (FFI Windows; analiza kodu + cross-clippy) | naprawione (bez reprodukcji — propozycja testu Windows CI: katalog z innym właścicielem / junction = odmowa): odmowa dla dowiązania/junction/pliku i właściciela spoza {konto usługi, SYSTEM, Administratorzy} |
| SR-09 | safety-broker | 6,8 średnia | **`gui.control` aplikacji desktopowych dostawców nieblokowane** (`claude.exe`, `chatgpt.exe`, `codex.exe`…): na L4 bez pytania — „używanie UI dostawcy” (§1.3 zas. 6, THREAT_MODEL §7, §9, subscription-routes §2.1). | `review.rs::provider_desktop_apps_are_hard_blocked` | naprawione: `PROVIDER_APPS` (z aliasami 8.3) → `KernelRule::ProviderWebUi` na każdym poziomie |
| SR-10 | transfer | 3,1 niska | Reguła ścieżek paczki nie znała nazw urządzeń `COM0`, `LPT0`, `COM¹–³`, `LPT¹–³`, `CONIN$`, `CONOUT$` (zapis na urządzenie zamiast pliku przy imporcie dokumentu). | `transfer-contract/src/paths.rs` (test `rejects_*` rozszerzony) | naprawione |
| P-01 | safety-broker (autonomia) | 6,0 | Wpis szczegółowy (`SessionAgent`) przesłania późniejsze **obniżenie** ogólniejszego celu przez właściciela (np. „panika” Global → L0 nie obniża agentki z wpisem pary L4; agentka może „przypiąć” poziom przez obniżenie pary o jeden). | analiza (`AutonomyTable::effective`) | propozycja: obniżenie celu ogólnego obcina wpisy szczegółowe (kaskada) albo osobne „sufity” agentek; zmiana semantyki ADR 15 — decyzja człowieka |
| P-02 | safety-broker | 4,0 | `CommandOrigin` deklaruje klient; rola `Agent` może podać `UserText`/pominąć `UntrustedContent` (dziś bez wpływu na klasyfikator poza dopasowaniem planu; taint liczy Broker). | analiza | propozycja: serwer IPC wymusza `Agent` dla roli `Agent` (jak `ChangeOrigin`); SPEC — otwarte pytanie F5 |
| P-03 | tools-shell | 7,5 (projekt) | Proces powłoki startuje z `Integrity::Medium` — THREAT_MODEL §4 / PLAN §8.1 wymagają restricted token / low-IL / AppContainer dla ≤ L3. Jedyną barierą jest strażnik leksykalny, obchodzony obfuskacją (`& ('ta'+'skkill') …`, `Invoke-CimMethod … Terminate`, zmienne w ścieżkach). | podejrzenie (bez Windows) | propozycja: Low IL + zapis przez Brokera albo AppContainer; decyzja właściciela (THREAT_MODEL §11) |
| P-04 | platform / tools-fs | 5,5 | Dowiązanie twarde (`mklink /H`, `fsutil hardlink`) do pliku z deny-listy nie zmienia postaci kanonicznej — omija SR-05. | podejrzenie | propozycja: odmowa odczytu/zapisu pliku z `nNumberOfLinks > 1` w obszarze profilu (lub ostrzeżenie + zgoda) |
| P-05 | mcp-impl | 3,5 | Utwardzenie (b) — potok MCP przez `SecurePipePort` — **niewykonalne bez zmiany kontraktu**: port jest blokujący i półdupleksowy (synchroniczny uchwyt serializuje `ReadFile`/`WriteFile`), host potrzebuje pełnego dupleksu (równoległe `tools/call`), a `alfa-mcp-proxy` otwiera potok przez tokio z `GENERIC_WRITE`, którego DACL `PipeSecurity` (`0x12008B`) nie przyzna. Dzisiejszy domyślny DACL + token sesji + etykieta ME są akceptowalne. | analiza | propozycja (SPEC `mcp`): wariant overlapped/asynchroniczny w `platform-contract` albo dwa połączenia półdupleksowe na sesję |
| P-06 | platform (tożsamość potoku) | 5,0 | Tożsamość klienta przez PID (`GetNamedPipeClientProcessId` → `OpenProcess`) — wyścig przy przekazaniu uchwytu i ponownym użyciu PID (znane, SPEC). | znane | propozycja: `ImpersonateNamedPipeClient` + `OpenThreadToken` dla SID/integralności; PID tylko do obrazu |
| P-07 | platform-windows-kernel-impl (`win_launch`) | 3,5 | `CreateProcessAsUserW(bInheritHandles = TRUE)` dziedziczy **wszystkie** dziedziczne uchwyty usługi; między `CreatePipe` a `SetHandleInformation` okno wyścigu z innym `CreateProcess`. | analiza | propozycja: `STARTUPINFOEXW` + `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` (tylko koniec potoku) |
| P-08 | app-safety | 6,5 | Produkcyjne `run()` używa `UnverifiedSignatures`, `signer: None`; role `Core`/`Watchdog` z `enroll` przechodzą po samej ścieżce obrazu. Po SR-07 ścieżki instalacji są obszarem Jądra, ale ochrona jest leksykalna. | analiza | propozycja: Authenticode + przypięty certyfikat (bramka #10) i obrazy Jądra poza `%LOCALAPPDATA%` (Program Files) |
| P-09 | safety-broker / risk-classifier | 6,0 | `net.egress` do loopback/sieci lokalnej (`127.0.0.1`, `::1`, `169.254.0.0/16`, `10/8`…) na L4 bez pytania: dostęp do lokalnych usług (port CDP przeglądarki → ciasteczka, Docker API) — S13/S14/S15. Wzorzec hosta nie zna portu. | analiza | propozycja: reguła każdego poziomu „egress lokalny” (zgoda) albo twarda blokada z wyjątkami w polityce |
| P-10 | mcp-contract | 3,0 | `initialize.instructions` serwera MCP nie wchodzi do odcisku (dziś nieużywane w promptach). | analiza | propozycja: przy pierwszym użyciu w promptach — do odcisku i zgody |
| P-11 | safety-broker (Audyt) | 3,0 | `AuditAppend` (jądro, watchdog) zapisuje zdarzenie dowolnego rodzaju (np. `broker.token.issued`) — odróżnialne tylko polem `relayed_by`. | analiza | propozycja: prefiks rodzaju `relay.*` albo odrzucanie rodzajów `broker.*` |
| P-12 | safety-broker (plany) | 3,0 | Plan z krokiem `system.admin` pokrywa wiele identycznych eskalacji w czasie ważności planu (§8.4 „zgoda per eskalacja”). | analiza | propozycja: wykluczyć `SystemAdmin` z planów |
| P-13 | providers (poza zakresem) | — | S14: brak walidacji hosta `baseURL` (loopback/rebinding) w adapterach — brak testu. | — | do karty modułu `providers-*` |

Przejrzane bez ustaleń: format i parser tokenu `ALFT` (ścisły, re-encode), MAC w czasie stałym, rotacja i
czyszczenie kluczy, kill-switch (unieważnienie przez nowy klucz, Job Objects przed Brokerem, limity czasu
watchdoga), łańcuch Audytu (kanoniczny JSON, kotwica, ucięcie ogona, rekord startowy), dowód fizycznego wejścia
(nonce tylko kanałem Broker-UI, `injected`, świeżość, Hello), Broker-UI (zakres „zawsze zezwalaj” = dokładnie
prośba, fokus ≥ 500 ms, zasłonięcie), `transfer` (limity ZIP i stopnia kompresji, `take(bytes+1)`, sumy SHA-256,
zip-slip, katalog magazynu przez `canonicalize`, Argon2id z limitami, klucz maszyny tylko pod stałą nazwą,
XChaCha20-Poly1305 STREAM z flagą ostatniego fragmentu), `agent-backends` (środowisko CLI z listy dozwolonej,
odmowa wyzwalaczy/Ulepszacza, przypięte wersje), `updater` (minisign strumieniowo, wiązanie `version:` w
komentarzu zaufanym, crash-loop → poprzednia wersja), `mcp` (odciski, zmiana opisu = blokada, brak nasłuchu TCP).

## 3. Reguły THREAT_MODEL → testy

| Reguła | Test(y) | Luka |
|---|---|---|
| S01/S03 taint, trifecta, egress | `risk-classifier-contract` proptest `tainted_egress_never_proceeds_even_on_l4`; `safety-broker-contract` `flows::tainted_egress_asks_on_l4`; SR-01 `review.rs` | — |
| S02 destrukcja fs, Kosz, cofanie | `tools-fs-impl/tests/ops.rs` (`move_copy_rename_delete_with_undo`, `delete_permanent_always_needs_owner`); `undo-journal-impl/tests/journal.rs` | — |
| S04/S05/S06 głos | `flows::voice_destruction_asks_on_l4`; reguły `Voice*` w `risk-classifier-contract` | EER/FAR — F5 |
| S07/S08 opis MCP, rug pull | `mcp-contract` `trust.rs` (`changed_description_blocks_even_on_trusted_server`, `injected_description_is_never_auto_approved`) | `instructions` poza odciskiem (P-10) |
| S09 Wasm | — | moduł `plugin-runtime` nie istnieje (F8) |
| S10 podniesienie poziomu / polityki | `safety-broker-impl/tests/negative.rs` (`zero_successful_attacks`, `timed_lowering_cannot_escalate_after_expiry`); SR-02 `review.rs` | kaskada obniżeń (P-01) |
| S11 zatwierdzanie samej siebie | `broker-ui-*` testy dowodu i fokusu; `platform-windows-kernel-impl/tests/kernel_windows.rs` (`SendInput`, `#[ignore]`); `negative.rs` (`forged_proof_attempts`); SR-03 | test sprzętowy tylko self-hosted |
| S12 XSS markdown | `lib-markdown/tests/xss.rs` | — |
| S13 port CDP | `apps/desktop/src-tauri/src/windows.rs` (tylko build testowy) | brak testu CI zamkniętego portu w produkcji (F3-13) |
| S14 DNS rebinding / `baseURL` | `mcp-impl/tests/static_rules.rs` (brak TCP) | walidacja `baseURL` (P-13), egress lokalny (P-09) |
| S15 poświadczenia CLI / przeglądarki | `compliance-contract/tests/deny_props.rs`, `review.rs`; `flows::kernel_blocks_on_l4`; `platform-windows-impl/src/fs/guard_tests.rs`, `tests/review.rs`; `agent-backends-impl/tests/rules.rs`, `review.rs` | dowiązania twarde (P-04), ETW (F4-04) |
| S16 UI dostawców | `flows::kernel_blocks_on_l4` (`claude.ai`); SR-04, SR-09 `review.rs` | — |
| S17 most z wyzwalacza | `agent-backends-contract` `policy::tests::origins` | — |
| S18 opaque worker | `agent-backends-impl` (worktree, `--permission-prompt-tool`), `mcp-impl/tests/static_rules.rs` | — |
| S19 zatrucie pamięci | moduł `memory-*` (poza zakresem przeglądu) | — |
| S20 Ulepszacz | — | moduł nie istnieje (F8) |
| S21 Audyt | `safety-broker-impl/tests/audit.rs` (`chain_of_10k_events_verifies_and_survives_reopen`, `tampering_is_detected`); `app-safety/tests/windows_ports.rs` (katalog prywatny) | SR-08 bez testu Windows (propozycja) |
| S22 pętle / budżety | `cost-meter-*`, `agent-runtime-*` (poza zakresem) | — |
| S23 łańcuch dostaw / aktualizacje | `cargo deny`; `updater-impl/tests/launcher.rs`, `store.rs` (podpis, crash-loop) | — |
| S24 runner | proces (§4.2) | — |
| S25 tagi prywatności | `compliance-contract::contract_tests::private_session_policy` | — |
| S26 zrzuty/OCR | — | `tools-vision` nie istnieje (F6) |
| S27 usługa admina | `AdminConsent` (każdy poziom) w `risk-classifier-contract`; `negative.rs` | UAC/usługa opt-in — nie zbudowane |
| S28 ten sam SID | poza modelem | — |
| §7 wyłączenie Audytu/watchdoga/kill-switcha | `negative.rs::kernel_target_attempts`, `safety-broker-contract/tests/shell_guard.rs` | obfuskacja powłoki (P-03) |
| §7 format dysku / `%SystemRoot%` / bootloader | `tests/shell_guard.rs`, `tests/scopes.rs`; SR-07 (`C:\`) | — |
| §7 zmiana polityk Jądra tylko w Broker-UI | `negative.rs::policy_attempts`, `grants::policy_change_only_with_proof`; SR-07 (pliki Alfy) | — |
| §7 podniesienie poziomu przez agentkę | `negative.rs::self_escalation_attempts`; SR-02 | — |
| §7 `gui.control` procesów Alfy/Brokera | `flows::kernel_blocks_on_l4` | — |
| §7 odczyt poświadczeń / profili / Credential Manager | jak S15 | — |
| §7 automatyzacja UI dostawców | jak S16 | — |
| §7 destrukcja głosem bez potwierdzenia nie-głosem | `flows::voice_destruction_asks_on_l4` | — |
| §7 egress z sesji `tainted` | jak S01; SR-01 (plany) | — |
| §7 most z wyzwalacza bez jawnego włączenia | jak S17 | — |
| §7 zmiana Jądra przez Ulepszacza | — | moduł nie istnieje (F8) |

## 4. Utwardzenia zgłoszone przez autorki

- **(a) deny-listy kluczy — wykonane.** `DenyLists::baseline` (compliance): segmenty `.ssh`, `.gnupg`, `.aws`,
  `.azure`, `.kube`, `.npmrc`, `.pypirc`, `.netrc`, `_netrc`, `.git-credentials`; prefiksy
  `%USERPROFILE%\.docker\config.json`, `%APPDATA%\gh` (testy: `compliance-contract/tests/review.rs`). Polityka
  Brokera bierze listę z `compliance`, więc obowiązuje także jako `KernelRule::CredentialDenylist`. Ostatnia linia
  (`WinFs`) dostała całą listę (SR-05).
- **(b) potok MCP przez `SecurePipePort` — nie wykonane, wymaga decyzji** (P-05; uzasadnienie w SPEC `mcp`).
- **(c) wydzielenie `platform-windows-kernel-impl` — wykonane.** `src/kernel/` → nowy crate (lib + `win.rs`/
  `error.rs` — kopie podzbioru narzędzi FFI, bo `-impl` nie może zależeć od cudzego `-impl`), test
  `tests/kernel_windows.rs` przeniesiony, `app-safety` przepięte (`WinKernel`, `WinSessionLauncher`,
  `WinApprovalSurface`), `deny.toml`: `wrappers` dla `windows` += `platform-windows-kernel-impl` (jedyna zmiana).
  Rozmiary: `platform-windows-impl` 6 406 linii `.rs`, `platform-windows-kernel-impl` 2 066. API portów bez zmian.

## 5. Bramki

| Bramka | Wynik |
|---|---|
| `cargo fmt --all -- --check` | pliki z tego przeglądu czyste; różnice w `app-*` (inne sesje, nie dotykane) |
| `cargo clippy --workspace --all-targets -D warnings` | czysto z wyłączeniem 8 crate'ów zależnych od `memory-contract` (`app-agents`, `app-api`, `app-core`, `app-modules`, `app-voice`, `memory-*`), które w chwili przeglądu nie kompilują się przez pracę równoległej sesji pamięci |
| clippy `--target x86_64-pc-windows-msvc` per crate zakresu | czysto; `transfer-contract` i `transfer-impl` pominięte — zależności testowe budują `openssl-sys` (vendored), a środowisko krzyżowe nie ma `perl` |
| `cargo test --workspace` (bez 8 crate'ów jw.) | zielono, poza doctestami `search-fake`/`search-impl` (praca równoległej sesji wyszukiwania) i niestabilnym `voice-audio-impl::no_alloc` pod obciążeniem (osobno przechodzi) |
| `cargo deny check` | advisories/bans/licenses/sources ok |
| `scripts/check-deps.sh` | OK (786 krawędzi, zero naruszeń) |

## 6. Zmienione i nowe ścieżki (do commita przez koordynatora)

Zmienione: `Cargo.lock` (nowa krawędź `compliance-contract → idna`, nowy pakiet workspace), `deny.toml`,
`crates/safety-broker-contract/src/{guard.rs,lib.rs,policy.rs}`,
`crates/safety-broker-impl/src/{apply.rs,decide.rs,ipc.rs,state.rs,tokens.rs}`,
`crates/compliance-contract/Cargo.toml`, `crates/compliance-contract/src/deny/{domain.rs,mod.rs}`,
`crates/agent-backends-impl/src/workspace.rs`, `crates/transfer-contract/src/paths.rs`,
`crates/platform-windows-impl/{Cargo.toml,README.md,src/lib.rs,src/fs/guard.rs}`,
`crates/app-safety/{Cargo.toml,src/broker.rs,src/watchdog.rs,src/bin/alfa-broker.rs,src/bin/alfa-broker-ui.rs,tests/windows_ports.rs}`,
`docs/modules/{safety-broker,compliance,platform-windows,agent-backends,mcp,transfer}/SPEC.md`.

Usunięte (przeniesione): `crates/platform-windows-impl/src/kernel/` (cały katalog),
`crates/platform-windows-impl/tests/kernel_windows.rs`.

Nowe: `crates/platform-windows-kernel-impl/` (`Cargo.toml`, `README.md`, `src/{lib.rs,error.rs,win.rs,portable.rs,win_launch.rs,win_pipe.rs,win_sec.rs}`,
`src/surface/{mod.rs,input.rs,thread.rs,win.rs}`, `tests/kernel_windows.rs`),
`crates/safety-broker-impl/tests/review.rs`, `crates/compliance-contract/tests/review.rs`,
`crates/compliance-contract/tests/review.proptest-regressions`, `crates/platform-windows-impl/tests/review.rs`,
`crates/agent-backends-impl/tests/review.rs`, `docs/reviews/2026-10-security-review-1.md`.

Do uzupełnienia przez koordynatora (pliki poza zakresem tej sesji): `crates/README.md` — wiersz `platform-windows`
(dopisać `platform-windows-kernel-impl`: porty Jądra, drugi crate z windows-rs) i wiersz `app-safety` (składa
także `platform-windows-kernel-impl`).

## 7. Do decyzji człowieka

1. Przegląd zmian w Brokerze i politykach (AGENTS.md: `safety-broker`, polityki bezpieczeństwa — przez człowieka):
   SR-01, SR-02, SR-03, SR-07, SR-09 zmieniają semantykę decyzji (szczegóły w SPEC `safety-broker`).
2. P-01: kaskada obniżeń poziomu (semantyka ADR 15).
3. P-03: izolacja procesu powłoki ≤ L3 (Low IL / AppContainer) — THREAT_MODEL §11.
4. P-05: kontrakt potoku asynchronicznego dla MCP (zmiana `platform-contract`).
5. P-08: Authenticode i położenie obrazów Jądra (bramka #10).
6. P-09: polityka egressu do sieci lokalnej.
7. SR-07: potwierdzić, że żadne narzędzie agentek nie potrzebuje zapisu w `%APPDATA%\Alfa`/`%LOCALAPPDATA%\Alfa`
   (np. pobieranie modeli przez agentkę) — dziś zapisuje tam wyłącznie Alfa, nie narzędzia.
