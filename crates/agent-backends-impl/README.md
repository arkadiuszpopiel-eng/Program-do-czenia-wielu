# agent-backends-impl

Mosty CLI jako „opaque worker” (PLAN §5.2 klasa C, §8.5). `BridgeBackend` implementuje
`AgentBackend`: bramka (`gate`: pochodzenie → `compliance.route_allowed` → przypięta wersja
`--version` → hash) przed jakimkolwiek procesem, izolowany katalog (`GitWorkspace`: `git worktree`
albo kopia bez dowiązań), most **Claude Code** (`claude -p` stream-json, `--permission-prompt-tool
mcp__alfa__approve` przez serwer MCP Alfy z `mcp-contract::BridgeMcpHost`) i most **Codex**
(`codex app-server`, zatwierdzenia serwera). Procesy CLI dostają środowisko z listy dozwolonej,
anulowanie zabija drzewo procesów (`TreeKiller`). Kod nie czyta katalogów poświadczeń CLI
(`tests/rules.rs`). Manifest: `module.toml`. Testy od końca do końca z fałszywym CLI są
w `agent-backends-fake/tests` (binarium testowe żyje w tamtym crate'cie).
