# mcp-impl

`StdioMcpClient` — klient serwerów MCP przez stdio (środowisko z listy dozwolonej, odświeżenie listy
narzędzi przed każdym wywołaniem, blokada po zmianie opisu do ponownej zgody użytkownika,
ostrzeżenia). `LocalMcpHost` — serwer MCP Alfy v0 dla mostów: kanał lokalny (named pipe na Windows,
gniazdo Unix 0600 gdzie indziej; **bez TCP**), tokeny z TTL, narzędzia przez porty
`platform-contract`. Bin `alfa-mcp-proxy` — pompa bajtów stdio ↔ kanał z powitaniem z tokenem.
Testy: kontrakt hosta, TTL, proxy od końca do końca, klient ↔ fałszywy serwer (rug pull, injection),
statyczny zakaz nasłuchu sieciowego i ścieżek poświadczeń. Manifest: `module.toml`.
