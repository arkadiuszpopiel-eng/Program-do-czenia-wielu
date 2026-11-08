# mcp-impl

`StdioMcpClient` — klient serwerów MCP przez stdio (środowisko z listy dozwolonej, odświeżenie listy
narzędzi przed każdym wywołaniem, blokada po zmianie opisu do ponownej zgody użytkownika,
ostrzeżenia). `LocalMcpHost` — serwer MCP Alfy dla mostów: kanał lokalny (named pipe na Windows,
gniazdo Unix 0600 gdzie indziej; **bez TCP**), tokeny z TTL, narzędzia v0 przez porty
`platform-contract`; v1 (`start_with` + `McpV1`): UIA i zrzuty przez narzędzia agentek oraz rejestr
tylko do odczytu — przez Brokera z podmiotem „most CLI”, wyniki `unverified_by_alfa` (`src/v1.rs`). Bin `alfa-mcp-proxy` — pompa bajtów stdio ↔ kanał z powitaniem z tokenem.
Testy: kontrakt hosta, TTL, proxy od końca do końca, klient ↔ fałszywy serwer (rug pull, injection),
statyczny zakaz nasłuchu sieciowego i ścieżek poświadczeń, v1 (`tests/v1.rs`, `tests/v1_registry.rs`:
0 odczytów sekretów w 200 losowych próbach). Manifest: `module.toml`.
