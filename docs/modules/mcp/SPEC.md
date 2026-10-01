# mcp — SPEC (v0)

## Cel
Alfa jako **klient MCP** (serwery zewnętrzne przez stdio, odcisk opisów narzędzi, poziomy zaufania) i **serwer MCP Alfy v0** dla mostów CLI: narzędzia **tylko** schowka i okien + wewnętrzne `approve` (prośby `--permission-prompt-tool`), bez fs/shell (PLAN §8.5, §8.7, §9.7). Transport serwera wyłącznie stdio-proxy + kanał lokalny z tokenem; **zero nasłuchu sieciowego**.

## Fala i priorytet
F4, P1. v0: schowek, okna, `approve`. F6 (v1): UIA, zrzuty, rejestr.

## Protokół
MCP **2025-06-18** (akceptowane także 2025-03-26, 2024-11-05), JSON-RPC 2.0, jedna wiadomość na linię, bez paczek. Serwer: `initialize` (negocjacja wersji), `ping`, `tools/list`, `tools/call`; nieznana metoda → **-32601**; żądanie przed `initialize` → -32002; nieznane narzędzie / złe argumenty → -32602; odmowa polityki → -32001; błąd wykonania narzędzia → `isError: true`.

## Kontrakt (`mcp-contract` — źródło prawdy)
- `jsonrpc` (`Dialect::Strict` dla MCP, `Lenient` dla `codex app-server`), `protocol` (typy MCP), `server_core::ServerSession` + `ToolHandler` (wspólne `-impl`/`-fake`).
- `fingerprint(Tool)` = SHA-256 kanonicznego JSON (nazwa, tytuł, opis, schematy, adnotacje); `scan_tool` — heurystyki injection (nadpisanie instrukcji, ukrywanie, `<IMPORTANT>`, ścieżki poświadczeń, eksfiltracja, znaki niewidoczne, długość) także w opisach parametrów.
- `TrustLevel {Untrusted (domyślny), Reviewed, Trusted}`; `assess`: zgoda z tym samym odciskiem → `Approved`; inny odcisk → `NeedsConsent(Changed)` + ostrzeżenie (także na serwerze zaufanym); brak zgody → `AutoApproved` tylko dla `Trusted` **bez sygnałów**, inaczej `NeedsConsent(New)`; sygnały = `untrusted`, nigdy autozgoda.
- `trait McpClient`: `tools`, `consent_tool(.., ConsentOrigin::User)` (agentka → `ConsentNotPermitted`), `call_tool` (odświeża listę przed każdym wywołaniem — rug pull bez powiadomienia też blokuje), `take_warnings`, `shutdown`.
- Serwer Alfy: `AlfaTool {clipboard_read, clipboard_write, windows_list, windows_focus, approve}` (nazwy z `_` — zgodne z regexem nazw narzędzi API modeli; zdolności `clipboard.read` itd.); okna procesów chronionych (Alfa, Broker, Broker-UI, watchdog, proxy) ukryte i nieaktywowalne.
- Host mostów `BridgeMcpHost::register(BridgeScope, Option<Arc<dyn ApprovalRouter>>) -> BridgeRegistration{launch: McpServerLaunch}` / `revoke`. `approve`: wejście `{tool_name, input, tool_use_id}`, wynik tekst JSON `{"behavior":"allow","updatedInput":..}` | `{"behavior":"deny","message":..}`.
- Token sesyjny: 2×UUIDv4 (CSPRNG), porównanie w stałym czasie, TTL na nawiązanie połączenia (domyślnie 4 h), `revoke` zamyka połączenia. Pierwsza linia proxy: `{"type":"alfa-mcp-hello","version":1,"token":".."}` (≤ 4 KiB, ≤ 5 s).

## Implementacja (`mcp-impl`)
`StdioMcpClient` (środowisko: lista dozwolona + jawne wpisy konfiguracji; serwer bez `sampling`/`roots` — żądania serwera odrzucane), `LocalMcpHost` + `AlfaToolHandler` (porty `ClipboardPort`, `WindowPort` z `platform-contract`), bin `alfa-mcp-proxy` (pompa bajtów stdio ↔ kanał; `ALFA_MCP_ENDPOINT` = `pipe:\\.\pipe\..` | `unix:..`; inne schematy odrzucane). Windows: named pipe `first_pipe_instance`, `reject_remote_clients`; inne OS: gniazdo Unix 0600 w katalogu 0700.

## Zależności
`mcp-contract`, `platform-contract`, `core-bus-contract`.

## Niezmienniki
- Brak `TcpListener`/`TcpStream`/`UdpSocket` w źródłach (test `no_network_listeners_in_sources`).
- Narzędzie bez aktualnej zgody nigdy nie jest wywoływane; zgoda wiąże odcisk; opis jest treścią niezaufaną.
- Zakres rejestracji ogranicza widoczne narzędzia; `approve` tylko z routerem; brak fs/shell.

## Zdolności / uprawnienia
`clipboard.read`, `clipboard.write`, `windows.list`, `windows.focus` (tokeny Brokera w F3+); zgody na narzędzia zewnętrzne — wyłącznie użytkownik.

## Izolacja
`inproc`, `on-demand`; proxy jako proces potomny CLI.

## Budżet zasobów
RAM ≤ 4 MB; ≤ 16 równoległych wywołań narzędzi na połączenie.

## Konfiguracja (klucze TOML)
`[mcp.servers.<id>] command, args, env, trust = "untrusted"`, `request_timeout_ms = 30000`; `[mcp.alfa] token_ttl = "4h"`, `proxy = "alfa-mcp-proxy"`; zgody: `[mcp.pins] "<serwer>/<narzędzie>" = "sha256:…"`.

## Testy akceptacyjne
F4-07: `host_passes_contract` (impl i fake), `proxy_binary_end_to_end_with_platform_ports`, `expired_and_garbage_hello_are_rejected`, `consent_is_bound_to_fingerprint_and_rug_pull_blocks`, `injected_description_is_untrusted_and_never_auto_approved`, `no_network_listeners_in_sources`.

## Fake
`FakeMcpServer` (serwer w procesie przez duplex; zmiana opisu z powiadomieniem lub bez; rejestr wywołań), `FakeBridgeMcpHost` (prawdziwy kanał lokalny, ręczny zegar TTL, rejestr `approve`).

## Otwarte pytania
- Jawny DACL named pipe na SID użytkownika wymaga windows-rs → port w `platform-windows-impl` (dziś: domyślny DACL — zapis tylko właściciel/SYSTEM/Administratorzy — + token). Przegląd 2026-10 (utwardzenie b): `SecurePipePort` z `platform-contract` jest blokujący i półdupleksowy (synchroniczny uchwyt serializuje `ReadFile`/`WriteFile`), a host MCP potrzebuje pełnego dupleksu (równoległe `tools/call`); klient `alfa-mcp-proxy` otwiera potok przez tokio z `GENERIC_WRITE`, czego DACL `PipeSecurity` (klient `0x12008B`) nie przyzna. Wymaga decyzji: (1) wariant asynchroniczny/overlapped w kontrakcie (`SecureAsyncPipePort` albo `PipeConnection::try_clone` na uchwycie `FILE_FLAG_OVERLAPPED`) + proxy łączące się przez port, albo (2) dwa połączenia półdupleksowe na sesję (osobno w górę i w dół, parowane tokenem).
- Trwały magazyn zgód (`PinStore`) w `core-config`; UI karty serwera MCP — F4 `ui-shell`.
- Limit częstości `approve` (zmęczenie zatwierdzeniami) — z Brokerem w F3/F4.
