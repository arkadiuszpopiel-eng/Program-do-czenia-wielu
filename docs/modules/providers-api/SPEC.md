# providers-api — SPEC (v1)

## Cel
Adaptery `ModelProvider` dla chmurowych API: Anthropic, OpenAI oraz **adapter generyczny** „endpoint zgodny z OpenAI/Anthropic" (Gemini, xAI, DeepSeek, Kimi, Qwen, Z.ai, MiniMax, OpenRouter, Mistral, Ollama, LM Studio…) parametryzowany wpisem `providers-catalog/<id>.toml`. Strumieniowanie z anulowaniem, health, koszt z konfiguracji, rendering historii append-only per dostawca (PLAN §5.1–5.3, §5.5, §6.5).

## Fala i priorytet
F1 (Anthropic, OpenAI Chat Completions + Responses, generyczny; chat + embeddings; fixture'y syntetyczne). P0. STT/TTS chmurowe przez ten sam kontrakt — F2 (`voice-stt/tts` jako klienci).

## Kontrakt — `providers-contract` (wspólny dla providers-api, providers-local, atrapy)
Rozstrzygnięcie otwartego pytania v0: **osobny crate `providers-contract`** (nie `providers-api-contract`), bo `providers-local` implementuje ten sam trait.
```rust
pub trait ModelProvider: Send + Sync {
    fn id(&self) -> &ProviderId;
    fn capabilities(&self) -> ProviderCapabilities;              // modele, ThinkingSupport, strict/forced tools, effort, sampling, cache, prywatność
    fn stream(&self, req: ChatRequest, cancel: CancellationToken) -> ProviderStream; // Stream<ProviderEvent>
    fn health(&self) -> ProviderHealth;                          // bez sieci; Unconfigured bez klucza
    fn estimate_cost(&self, req: &ChatRequest) -> Option<CostEstimate>;
    fn cost(&self, model: &str, usage: &Usage) -> Option<Cost>;  // z cennika konfiguracji
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError>; // Models API → accounts-hub
    async fn embed(&self, req: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError>; // domyślnie Unsupported
}
```
IR: `Message{role: User|Assistant|System, content: [Text | Image | Thinking{text, signature, provider_origin} | RedactedThinking | ToolUse | ToolResult], interruption}`; `ToolSpec{input_schema, strict}`; `GenerationParams{max_tokens, temperature, effort, stop, thinking{enabled, display}}`; `RequestMeta{privacy{tag, jurisdiction_allow}, session}`. Zdarzenia: `Started`, `TextDelta`, `ThinkingDelta`, `ThinkingSignature`, `RedactedThinking`, `ToolCallStart/Delta/End`, `ModelSwitched`, `Usage{input, output, cache_read, cache_write}`, `Stop{EndTurn|MaxTokens|ToolUse|StopSequence|Refusal|Cancelled|PauseTurn|ContextWindowExceeded}`, `Error{RateLimited{retry_after}|Overloaded|Auth|InvalidRequest|Network|Timeout{phase}|Server{status}|PrivacyBlocked|Unsupported|Protocol}` — dokładnie jedno zdarzenie końcowe. `TurnAccumulator` składa turę do historii.
Zdarzenia magistrali (`ObservedProvider`): `provider.call.started/finished` (`EventKind::ModelCall`, tokeny, koszt, TTFT, opóźnienie), `provider.error`, `provider.refusal` — bez treści rozmowy. `provider.circuit.opened` publikuje Router.

## Zależności
`core-bus-contract`, `core-registry-contract`, `providers-contract`. Klucz przez `SecretSource` (implementuje `accounts-hub`). Zewnętrzne: `reqwest 0.12` (rustls + ring, certyfikaty systemu; bez OpenSSL/aws-lc — `docs/vendor/reqwest.md`), `bytes`, własny parser SSE.

## Niezmienniki (zweryfikowane z dokumentacją API przez skill `claude-api`, 2026-09-30)
- Historia append-only: przerwana tura idzie w pełnej postaci + notka „Użytkownik usłyszał tylko: „…" i przerwał." jako **nowa** wiadomość użytkownika (`render_interrupted_turn`); instrukcje systemowe ze środka rozmowy — deterministycznie jako tekst użytkownika z prefiksem; narzędzia sortowane po nazwie.
- Anthropic Opus 5.5: myślenia nie da się wyłączyć (`disabled`/`budget_tokens` → 400) → zawsze `adaptive`, sterowanie `output_config.effort` (jawnie, domyślnie `medium`); `tool_choice any/tool` → 400 → `auto` + `strict`; `temperature` → 400 → pomijana; bloki myślenia związane z modelem i rozmową → własne podpisane bloki odsyłane bajt w bajt, obce/niepodpisane pomijane; po 400 „bound to a different conversation" jedno ponowienie bez bloków myślenia.
- Cache promptu: `tools → system → messages`, `cache_control` na systemie (lub ostatnim narzędziu) i ostatnim bloku ostatniej wiadomości.
- `stop_reason: refusal` = `Stop(Refusal)` z `stop_details.category`; serwerowy fallback (`fallbacks: "default"`) → `ModelSwitched`, myślenie/narzędzia sprzed przełączenia nie trafiają do historii.
- Ponawianie z backoffem i jitterem **tylko przed pierwszym tokenem** i tylko dla odrzuceń idempotentnych (429/5xx/529, błąd połączenia); `retry-after` > 1 s → błąd od razu (Router przełącza); budżet 1,5 s.
- Klucz pobierany w chwili wywołania; nagłówek wrażliwy; treść błędów redagowana; `ApiKey` bez `Serialize`.
- Żądanie „prywatne" do tagu CN/„może trenować"/`unknown` → `PrivacyBlocked` przed siecią (obrona w głąb).
- Anulowanie/upuszczenie strumienia zrywa połączenie HTTP ≤ 100 ms.
- Ceny wyłącznie z konfiguracji (`Pricing`, USD/MTok); tabela znanych modeli Anthropic zawiera tylko możliwości.

## Zdolności / uprawnienia
`net.egress(providers-catalog)` — hosty z katalogu/konta (egress-allowlista Jądra).

## Izolacja
`inproc`, `lazy`.

## Budżet zasobów
RAM ≤ 4 MB per aktywny adapter (manifest: 16 MB na moduł); narzut na TTFT ≤ 20 ms; przełączenie fallback ≤ 2 s bez utraty wiadomości.

## Konfiguracja (klucze TOML)
`[providers.api.<id>] base_url, default_model, effort = "medium", timeouts = {connect_ms = 5000, first_token_ms = 20000, idle_ms = 30000}, retry = {max_retries = 2, max_retry_after_ms = 1000, budget_ms = 1500}, api = "responses"|"chat"` (OpenAI), `[providers.api.<id>.pricing.<model>] input_per_mtok_usd, output_per_mtok_usd, cache_read_per_mtok_usd, cache_write_per_mtok_usd`, `[providers.api.<id>.models.<model>]` — nadpisania `ModelCapabilities`.

## Wkład do UI
Chip profilu modelu, szczegóły wiadomości (model, tokeny, koszt, TTFT), stany 429/wyczerpane okno (§14.4), „dodaj klucz, aby odblokować" (`HealthState::Unconfigured`).

## Testy akceptacyjne
- `ACC-F1-providers-api-01`: zestaw kontraktowy zielony na 5 wariantach (Anthropic natywny i zgodny, OpenAI Chat natywny i zgodny, OpenAI Responses) na nagraniach SSE z serwerem fixture — **spełnione**; na żywo = nightly po dodaniu klucza (`Recorder` → kasety NDJSON).
- `ACC-F1-providers-api-02`: 5xx/timeout → sklasyfikowany `Error` (`should_fallback`) ≤ 2 s, ta sama historia u celu zapasowego — test `router_style_fallback_within_two_seconds` + kontrakt.
- `ACC-F1-providers-api-03`: przerwana tura append-only dla każdego adaptera — przypadek kontraktowy `interrupted_turn_is_rendered_append_only` + property-test projekcji.

## Fake
`providers-fake`: skrypty (TTFT, tokeny/s, wirtualny zegar tokio), błędy 429/5xx/timeout/odmowa, weryfikacja żądań po projekcji, record/replay kaset NDJSON.

## Otwarte pytania
- Spike (g) (ADR 0006): czy model respektuje notkę o usłyszanym prefiksie; czy włączyć jawne `block_binding` (`PrefixMismatch::Error`) domyślnie — po uzyskaniu klucza.
- Realtime (S2S, natywne `truncate`) — osobny adapter F2 (`InterruptionRendering::NativeTruncate` już w kontrakcie).
