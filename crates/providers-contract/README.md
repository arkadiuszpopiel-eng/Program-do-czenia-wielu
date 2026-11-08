# providers-contract

Wspólny kontrakt `ModelProvider` (docs/PLAN.md §5.1–5.3, ADR 0005, ADR 0006) — rozstrzyga otwarte
pytanie SPEC providers-api o umiejscowienie kontraktu: jeden crate dla `providers-api`,
`providers-local` i atrapy. Tylko typy, trait i czyste funkcje (bez sieci).

| Obszar | Typy / funkcje |
|---|---|
| Trait | `ModelProvider`: `id`, `capabilities`, `stream(req, CancellationToken) -> ProviderStream`, `health`, `estimate_cost`, `cost`, `list_models`, `embed` (domyślnie `Unsupported`); `Arc<P>` też jest dostawcą |
| IR rozmowy | `ChatRequest` (model, `system`, `messages`, `tools`, `tool_choice`, `GenerationParams{max_tokens, temperature, effort, stop, thinking}`, `CachePolicy`, `RequestMeta{privacy, session}`), `Message{role, content, interruption}`, `ContentBlock` (tekst, obraz, `Thinking{text, signature, provider_origin}`, `RedactedThinking`, `ToolUse`, `ToolResult`), `ToolSpec{input_schema, strict}` |
| Strumień | `ProviderEvent`: `Started`, `TextDelta`, `ThinkingDelta`, `ThinkingSignature`, `RedactedThinking`, `ToolCallStart/Delta/End`, `ModelSwitched`, `Usage`, `Stop{reason, details}`, `Error` — dokładnie jedno zdarzenie końcowe; `TurnAccumulator` składa turę do historii append-only |
| Błędy | `ProviderError{kind, status, provider_code, request_id, after_output}`; `ProviderErrorKind`: `RateLimited{retry_after_ms}`, `Overloaded`, `Auth`, `InvalidRequest`, `Network`, `Timeout{phase}`, `Server{status}`, `PrivacyBlocked`, `Unsupported`, `Protocol`; `is_retryable()` (adapter, przed 1. tokenem), `should_fallback()` (Router) |
| Możliwości | `ModelCapabilities` (rodzaje, kontekst, narzędzia/`strict`/wymuszanie, wizja, `ThinkingSupport{None, Optional, AlwaysOn}`, `effort`, próbkowanie, cache, natywne obcięcie) — domyślne ostrożne |
| Koszt | `Pricing` (USD/MTok z konfiguracji), `Cost{nano_usd}`, `CostEstimate`, `estimate_input_tokens` |
| Prywatność | `RequestPrivacy{tag, jurisdiction_allow}`, `ProviderPrivacy{tag, jurisdiction}`, `check_privacy` (sesja prywatna ≠ CN/„może trenować"/`unknown`) |
| Przerwanie | `render_interrupted_turn(full, heard, approximate)`, `project_history`, `interruption_note` — pełna tura bez zmian + notka jako nowa wiadomość |
| Sekrety | `ApiKey` (zeroize, redakcja w `Debug`/`Display`, bez `Serialize`), `SecretSource`, `StaticKey` |
| Schemat | `chat_request_schema()`, `provider_event_schema()` (JSON Schema IR, v1) |

Feature `contract-tests`: `contract_tests::run_all(&harness)` — 13 przypadków (gramatyka strumienia,
narzędzia, podpisane myślenie odsyłane bez zmian, klasyfikacja 400/401/429/500/529 w ≤ 2 s, odmowa,
`max_tokens`, timeout, anulowanie ≤ 100 ms, anulowanie przed startem bez sieci, prywatność przed
siecią, walidacja lokalna, przerwana tura append-only, koszt z cennika). Uruchamiają je
`providers-fake` i każdy adapter `providers-api-impl` (serwer fixture).
