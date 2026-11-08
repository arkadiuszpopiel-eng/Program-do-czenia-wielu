# mcp-fake

`FakeMcpServer` — zewnętrzny serwer MCP w procesie (duplex) ze sterowanymi narzędziami (zmiana
opisu z powiadomieniem albo bez, złośliwe opisy, rejestr wywołań). `FakeBridgeMcpHost` — host
mostów z prawdziwym kanałem lokalnym (bez TCP), ręcznym zegarem TTL i rejestrem próśb `approve`;
używany przez testy `agent-backends`, które nie mogą zależeć od `mcp-impl`.
