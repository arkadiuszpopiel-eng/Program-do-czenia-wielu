# lib-openai-compat

Wspólna biblioteka (`lib-*`, bez logiki modułu) klienta HTTP/SSE dla endpointów zgodnych z OpenAI.
Wydzielona z `providers-api-impl` (refaktoryzacja bez zmiany zachowania — wszystkie testy
`providers-api-impl` zielone), żeby `providers-local-impl` (sidecar `llama-server`) korzystał
z tego samego silnika bez zależności od cudzego `-impl` (`scripts/check-deps.sh`).

| Element | Zawartość |
|---|---|
| `Engine<C: WireCodec>` | strumień w zadaniu tła; ponawianie z backoffem i jitterem **tylko przed 1. tokenem** i tylko dla odrzuceń idempotentnych; limity connect / first-token / idle; anulowanie i upuszczenie strumienia zrywają połączenie (≤ 100 ms); klucz z `SecretSource` w chwili wywołania (nagłówek wrażliwy, redakcja błędów); zdrowie z historii; `request_json` (Models API, osadzenia) |
| `WireCodec` / `StreamDecoder` / `WireRequest` / `BuildOptions` | punkt rozszerzeń adapterów (Anthropic, OpenAI Responses w `providers-api-impl`; `LlamaCodec` w `providers-local-impl`) |
| `sse` | parser Server-Sent Events odporny na podział porcji w dowolnym bajcie |
| `chat` | Chat Completions: `build_body` (+ `ChatOptions`, `MaxTokensField`) i `ChatDecoder` |
| `common` | klasyfikacja błędów OpenAI (`insufficient_quota` → `Auth`, `retry-after-ms`), zużycie bez cache, obrazy, wysiłek |
| konfiguracja | `HttpConfig`, `AuthScheme`, `Timeouts`, `RetryPolicy`, `ProviderProfile`, `ConfigError` (re-eksportowane przez `providers-api-impl` bez zmian API) |

Zależności: `providers-contract` + `reqwest 0.12.28` (rustls/ring), `tokio`, `serde_json` (wersje jak w `providers-api-impl`).
