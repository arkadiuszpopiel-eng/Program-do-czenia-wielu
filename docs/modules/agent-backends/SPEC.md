# agent-backends — SPEC (v0)

## Cel
Mosty abonamentowe (klasa C, PLAN §5.2) jako kontrakt `AgentBackend`: zadanie → strumień zdarzeń, zatwierdzenia, sterowanie, anulowanie, wznawianie. Most = sterowanie **oficjalnym, niezmodyfikowanym CLI** (`claude`, `codex`), do którego loguje się **wyłącznie użytkownik** (terminal ConPTY, F4 `ui-terminal`); „opaque worker" w worktree/kopii (§8.5). Moduł **nie** czyta, nie kopiuje, nie loguje i nie przechowuje tokenów CLI ani nie udaje `chat.completions` (§1.3, ADR 0005).

## Fala i priorytet
F4 (∥ F5), P1. v0: Claude Code + Codex. Później: Grok Build, Kimi Code, `agy` (po weryfikacji tras, §5.5).

## Kontrakt (`agent-backends-contract` — źródło prawdy)
- `trait AgentBackend`: `submit_task(TaskSpec) -> TaskHandle`, `events(&TaskId) -> AgentEventStream` (od początku, potem na żywo), `approve(&PermissionRequestId, ApprovalDecision)`, `steer(&TaskId, String)`, `cancel(&TaskId)`, `resume(SessionRef, TaskSpec)`.
- `TaskSpec { bridge, prompt, workdir: WorkdirSpec{source, mode: Worktree|Copy}, allowed_tools, disallowed_tools, budget{max_turns, wall_clock_ms, max_cost_micro_usd}, session: Option<SessionRef>, alfa_session, privacy: SessionTag, origin: LaunchOrigin, model }`.
- `LaunchOrigin { UserRequest, Scheduled{schedule_id}, Trigger{trigger_id}, Improver }`.
- `AgentEvent`: `Started`, `ColdStart{ms}`, `SessionStarted`, `Plan`, `Step`, `ToolRequest`, `ToolFinished`, `PermissionRequest`, `PermissionResolved{timed_out}`, `Output{Text|Markdown, partial}`, `FileChanged`, `Usage` (IR `providers-contract::Usage` + koszt), `Warning`, terminalne `Done{TaskResult}` | `Error{BackendError}`. Każde w `AgentEventEnvelope{seq, at_ms, unverified_by_alfa: true}`.
- Port `ApprovalSink::request(PermissionRequest) -> Option<ApprovalDecision>` (Broker-UI w F3/F4); port `Workspace` (`prepare`/`reuse`/`release`).
- Czyste reguły (`policy`): `check_origin`, `check_version` (lista przypiętych ∩ `cli_pinned_version` z rejestru), `child_env` (lista dozwolona, bez sekretów).
Zdarzenia magistrali: `agent.bridge.event` (koperta), `agent.bridge.refused`.

## Mosty (`agent-backends-impl`)
- **Claude Code**: `claude -p --output-format stream-json --input-format stream-json --verbose --permission-mode default --permission-prompt-tool mcp__alfa__approve --mcp-config <plik 0600> --strict-mcp-config [--include-partial-messages] [--allowedTools ..] [--disallowedTools ..] [--max-turns n] [--model m] [--resume id]`. Polecenie i sterowanie = wiadomości `user` na stdin; stdin zamykany, gdy liczba `result` = liczba wysłanych wiadomości. Parser NDJSON tolerancyjny (nieznane typy/pola → `Warning`).
- **Codex**: `codex app-server` (JSON-RPC po stdio, bez nagłówka `jsonrpc`): `initialize` → `thread/start {cwd, sandbox: workspace-write, approvalPolicy: on-request}` / `thread/resume` → `turn/start`; zatwierdzenia `item/commandExecution|fileChange/requestApproval` (accept/decline) i v1 `execCommandApproval`/`applyPatchApproval`; sterowanie = `turn/interrupt` + nowa `turn/start`. `codex exec --json` nie ma kanału zatwierdzeń — nieużywany.

## Zależności
`agent-backends-contract`, `compliance-contract` (`route_allowed`, przypięcie z rejestru), `mcp-contract` (`BridgeMcpHost`, `ApprovalRouter`, JSON-RPC), `providers-contract` (`Usage`, `CancellationToken`), `accounts-hub-contract` (`parse_version`), `core-bus-contract`.

## Niezmienniki
- Kolejność bramki: specyfikacja → pochodzenie → trasa w `compliance` → program → `--version` przypięta → hash (opcjonalny) → dopiero wtedy worktree i proces. Odmowa = 0 procesów zadania.
- `Trigger` i `Improver` — zawsze odmowa; `Scheduled` — tylko z `[agent_backends.launch.scheduled.<trasa>] max_per_day > 0`, licznik dobowy.
- Proces CLI dostaje wyłącznie listę dozwoloną środowiska (bez `*_API_KEY`, `*TOKEN*`, `ALFA_*`, `ANTHROPIC_*`, `OPENAI_*`, `CODEX_*`…); praca tylko pod katalogiem worktree Alfy; kopia pomija dowiązania.
- Dokładnie jedno zdarzenie końcowe; brak decyzji w `approval_timeout` = odmowa; anulowanie zabija drzewo procesów (grupa procesów / `taskkill /T`; port `TreeKiller` na Job Object) i odrzuca oczekujące prośby.

## Zdolności / uprawnienia
`process.spawn(cli-bridge)`, `fs.write(worktrees)`; narzędzia Windows dla mostu przez serwer MCP Alfy (`mcp`).

## Izolacja
CLI jako proces potomny (`process`), `on-demand`; nigdy na ścieżce głosu (zimny start w sekundach).

## Budżet zasobów
RAM modułu ≤ 8 MB (bez procesów CLI); postęp ≤ 1 s, anulowanie ≤ 2 s (F4-01/02).

## Konfiguracja (klucze TOML)
`[agent_backends.bridges.claude_code] program = "claude"`, `pinned_versions = []`, `sha256` (opcjonalnie); to samo dla `codex`; `[agent_backends.launch.scheduled]`, `approval_timeout = "10m"`, `max_line_bytes = 4194304`, `claude_partial_messages = true`, `worktrees_dir`, `runtime_dir`.

## Wkład do UI
Karta delegowanego zadania (plan, kroki, wyjście, koszt), karta „czeka na zatwierdzenie", oznaczenie „niezależnie niezweryfikowane", komunikaty odmów (trasa/wersja/pochodzenie).

## Testy akceptacyjne
- F4-01/02 (`progress_latency_within_budget`, `cancel_kills_process_tree`, kontrakt `cancel_within_budget`) — ściśle przy `ALFA_PERF_BUDGETS=1`.
- F4-03: 20/20 próśb → `ApprovalSink` (kontrakt `permissions_reach_sink` dla obu mostów + `twenty_of_twenty…` przez narzędzie MCP `approve`).
- F4-04: statyczny skan źródeł (brak ścieżek poświadczeń); monitor ETW — self-hosted (do zrobienia z `platform-windows`).
- F4-05: `disabled_route_means_zero_calls_0_of_100`; F5-04: `bridges_never_start_from_trigger_or_improver_0_of_100`.
- F4-08: fixture'y nagrane z prawdziwych CLI (`evals/F4/bridge-fixtures/`) — po spike'u (b).

## Fake
`FakeAgentBackend` (scenariusze w pamięci) i binarium `alfa-fake-agent-cli` (udaje `claude -p` stream-json z `approve` przez kanał MCP oraz `codex app-server`); scenariusze ze znacznika `[alfa-fake:<nazwa>]` (`contract_tests::scenario`).

## Otwarte pytania (spike (b), maszyna z zalogowanym CLI)
- Claude: `--include-partial-messages`, `--strict-mcp-config`, kolejkowanie wiadomości stream-json w trakcie tury; czy `mcp__alfa__approve` dodać do `--disallowedTools` (model nie powinien wołać go sam).
- Codex: nazwy metod/pól app-servera v2, `sandbox`/`approvalPolicy` w `thread/start`, natywne `turn/steer`; MCP Alfy dla Codex (`-c mcp_servers…`).
- Przypięte wersje i hashe (`cli_pinned_version` w rejestrze) — wynik spike'u.
