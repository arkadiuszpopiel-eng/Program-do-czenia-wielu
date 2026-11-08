# tools-common — SPEC (v0, zaimplementowany)

## Cel
Wspólny kontrakt narzędzi agentek (`tools-fs`, `tools-shell`, `tools-clipboard`; w przyszłości MCP): jeden format manifestu dla modelu i Brokera, jedno wywołanie, jeden protokół zgody Brokera, wspólne zasady ścieżek i niezaufanej treści (PLAN §7.2, §8.7, §9.1). Tylko kontrakt (`tools-common-contract`) — bez `-impl`/`-fake`; atrapą narzędzia jest `ScriptedTool`.

## Fala i priorytet
F3. P0.

## Kontrakt (`tools-common-contract`)
```rust
pub struct ToolManifest { name /* ^[a-zA-Z0-9_-]{1,64}$, np. fs_read */, id /* tools-fs.read */, title /* PL, UI */, description /* PL, dla modelu */,
                          input_schema, output_schema /* JSON Schema, obiekt zamknięty */, reversible: Reversibility /* yes|scoped|no */,
                          capabilities: Vec<String> /* fs.read, fs.write, shell.exec, net.egress, gui.control */, groups: Vec<String> /* grupy ról */,
                          mutating: bool, untrusted_output: Option<TaintSource> }
impl ToolManifest { fn allowed_for(&self, role_groups: &[String], read_only: bool) -> bool; fn validate(&self) -> Result<(), ManifestError> }
pub struct ToolCtx { holder: Holder /* sesja, agentka, rola */, origin: CommandOrigin, run: Option<RunId>, step: u32, label: String /* tekst „Cofnij” */,
                     workdir: Option<String>, untrusted_args: bool, cancel: CancellationToken, approval_timeout: Duration, observer: Option<Arc<dyn ToolObserver>> }
pub struct ToolOutcome { status: ToolStatus, text /* dla modelu */, data, images, untrusted: Option<TaintSource>, undo: Option<UndoRef>,
                         intent: Option<ToolIntent>, approval: Option<ApprovalId>, truncated: bool }
pub enum ToolStatus { Ok, Denied { reason: DenialReason }, NeedsConfirmation, Failed { error: ToolErrorKind }, Cancelled }
pub enum DenialReason { KernelBlock { rule }, DenyList, OwnerDenied { approval }, ApprovalExpired { approval }, ApprovalTimeout { approval },
                        TokenRejected, AuditUnavailable, Policy }          // describe() → zdanie PL dla modelu i UI
pub struct UndoRef { service: UndoService /* Journal | Clipboard | System (zmienna użytkownika) */, id: u64, text: String }
pub struct ToolIntent { kind /* shell.open_in_terminal | fs.confirm_delete_permanent */, title, details: Value }
#[async_trait] pub trait Tool: Send + Sync { fn manifest(&self) -> &ToolManifest; async fn call(&self, args: Value, ctx: &ToolCtx) -> ToolOutcome; }
pub trait ToolObserver { fn approval_requested(&self, &ApprovalTicket); fn approval_resolved(&self, ApprovalId, bool); }
pub struct BrokerGate { .. }   // decide → Allow(token) | NeedsApproval (odpytywanie co 250 ms, limit, anulowanie) | Deny → verify → unieważnienie
pub mod paths { resolve_path, has_credential_segment, exact_scope, tree_scope }   // ścieżki od modelu
pub mod text { /* redakcja sekretów, obcinanie, delimitacja <<<NIEZAUFANE … <<<KONIEC NIEZAUFANE */ }
```
Zdarzenia: `tool.<narzędzie>.*` na magistrali (`tool_event`, kontekst sesji/agentki/kroku); Audyt prowadzi Broker.

## Zależności
`safety-broker-contract` (zgody, tokeny), `risk-classifier-contract` (fakty, odwracalność), `compliance-contract` (deny-listy, `PathEnv`), `core-bus-contract`, `providers-contract` (anulowanie).

## Niezmienniki
- Każda akcja przez `BrokerGate`: token jednorazowy (TTL 5 min), `verify` przy użyciu, unieważnienie po akcji; brak zgody = `Denied` z powodem, nigdy obejście ani ponowienie tej samej akcji.
- Czekanie na zgodę ograniczone `approval_timeout` (domyślnie 5 min; aplikacja bez okna Brokera ≤ 60 s) i anulowalne; po czasie `ApprovalTimeout`.
- Ścieżki od modelu: względne rozwiązywane wobec `workdir`; odrzucane `..`, ADS, nazwy urządzeń, segmenty poświadczeń (`.ssh`, `.claude`, `.codex`, `Credentials`, `Login Data`, `Cookies`) — przed Brokerem.
- Wynik z danymi z zewnątrz oznaczony `untrusted` (taint sesji) i delimitowany w prompcie; sekrety redagowane w tekście dla modelu i logach.
- Manifest zamknięty (`additionalProperties: false`), argumenty parsowane ściśle (`parse_args`).
- Narzędzie przysługuje roli, gdy jej grupa pokrywa grupę manifestu (`fs` ⊇ `fs.read`); rola tylko-do-odczytu nie dostaje narzędzi `mutating`.

## Testy
`contract_tests` (uruchamiane dla `ScriptedTool` i każdego `tools-*-impl`): manifesty walidne, ścisłe argumenty, odmowa bez wykonania, anulowanie, redakcja.

## Otwarte pytania
- Rodzina zdolności `clipboard.*` w Brokerze (dziś `gui.control(clipboard.exe)`) — SPEC v1 `safety-broker`.
- Narzędzia MCP w tym samym formacie (mapowanie schematów, taint) — F4.

## Poprawki po recenzji PR #1 (2026-10-04)
- **Q-1:** `paths::resolve_links` (dowiązania — symlink, junction, inne punkty ponownej analizy — najdłuższego
  istniejącego prefiksu rozwiązywane przez `canonicalize`, prefiks `\\?\`/`\\?\UNC\` zdejmowany, nieistniejąca reszta
  dołączana) i `paths::protected_with_links(path, is_protected)` — sprawdzenie w postaci podanej **albo** rozwiązanej;
  istniejący komponent nierozwiązywalny (zerwane dowiązanie, brak dostępu) = chroniona (fail-closed). Używają jej
  `tools-fs`, `tools-shell` (przy każdym wywołaniu) i `app-modules::workdir` (przy wyborze katalogu). Nowy wariant
  `PathError::Unresolvable`. Test: `paths::tests::links_are_resolved_before_checks`.
- **Q-8:** `paths::resolve_path_dots` — jak `resolve_path`, ale `.`/`..` zwijane leksykalnie (cel polecenia powłoki
  względem katalogu roboczego); `..` ponad korzeń dysku/udziału UNC → błąd. `resolve_path` dalej odrzuca `..`.
- **Przegląd fali 3, W3-03 (`docs/reviews/2026-10-wave3-review.md`):** `resolve_path`/`resolve_path_dots` odrzucają ścieżki
  sieciowe (UNC `\\serwer\udział`, `//serwer/udział`, WebDAV `\\host@SSL\…`) — nowy wariant `PathError::Network` —
  chyba że leżą w sieciowym katalogu roboczym sesji (wybór właściciela). Wcześniej sprawdzenie dowiązań
  (`symlink_metadata`/`canonicalize`) łączyło się z serwerem jeszcze przed Brokerem (SMB/WebDAV, NTLM konta
  właściciela). Moduł `netpath` (`is_network_path`); test Q-8 przepięty na katalog roboczy na udziale.
