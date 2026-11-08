# app-bridges

Mosty CLI i MCP w aplikacji (kategoria `app-*`).

- `BridgesApp` — karty zgodności tras z rejestru `compliance` (zielona/szara/zabroniona, wpis nieświeży,
  wersja wykryta vs przypięta, źródła regulaminu, data weryfikacji), wyłącznik trasy (tylko
  użytkownik; zabronionej nie da się włączyć), przypięcie **wykrytej** wersji
  (`agent_backends.pins_<most>`), jawna zgoda na harmonogram (`agent_backends.schedule_<most>`, ≤ 24/dobę),
  „Zaloguj w terminalu" — terminal w katalogu domowym i polecenie (`claude /login`, `codex login`) do
  skopiowania; Alfa niczego nie wykonuje i nie czyta tokenów CLI (`~/.claude`, `~/.codex`).
- `BridgeHandle` — `AgentBackend` nad `agent-backends-impl::BridgeBackend` przebudowywanym po zmianie
  ustawień (zadania trafiają do instancji, która je przyjęła).
- `LazyMcpHost` — serwer MCP Alfy v0 (`mcp-impl::LocalMcpHost`, kanał lokalny bez TCP) uruchamiany
  dopiero przy pierwszej rejestracji zadania mostu; `proxy_program()` — `alfa-mcp-proxy` obok aplikacji.
- `parse_delegation` — „Delta, zleć to Claude Code", „przekaż Codexowi: …" (tylko jawny czasownik +
  nazwa mostu).

Testy integracyjne: `crates/app-core/tests/bridges.rs` (na `agent-backends-fake`).
