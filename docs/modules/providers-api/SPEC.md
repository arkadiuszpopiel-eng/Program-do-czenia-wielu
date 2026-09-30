# providers-api — SPEC (szkic v0)

## Cel
Adaptery `ModelProvider` dla chmurowych API: Anthropic, OpenAI oraz **adapter generyczny** „endpoint zgodny z OpenAI/Anthropic" (Gemini, xAI, DeepSeek, Kimi, Qwen, Z.ai, MiniMax, OpenRouter, Mistral…). Strumieniowanie z anulowaniem, health, koszt, rendering historii append-only per dostawca (PLAN §5.1–5.3, §5.5).

## Fala i priorytet
F1 (Anthropic, OpenAI, generyczny; chat + embeddings; fixture'y syntetyczne). P0. STT/TTS chmurowe przez ten sam kontrakt — F2 (`voice-stt/tts` jako klienci).

## Kontrakt (szkic Rust)
```rust
// providers-api-contract — SZKIC (wspólny kontrakt ModelProvider w providers-common?)
pub enum ModelKind { Chat, Stt, Tts, Embeddings, Vision, S2S }
pub struct Capabilities { pub kinds: Vec<ModelKind>, pub tools: bool, pub vision: bool, pub context: u32,
                          pub thinking: ThinkingSupport, pub native_truncate: bool }
pub struct ChatRequest { pub model: ModelId, pub history: HistoryView /* projekcja gałęzi, append-only */,
    pub tools: Vec<ToolSpec>, pub effort: Option<Effort>, pub max_tokens: u32, pub cache_prefix: bool, pub privacy: PrivacyTag }
pub enum ChatEvent { TextDelta(String), ThinkingDelta, ToolCall(ToolCall), Usage(Usage), StopReason(Stop /* End | Refusal | ToolUse | MaxTokens */), Error(ProviderError) }
pub trait ModelProvider: Send + Sync {
    fn capabilities(&self) -> Capabilities;
    fn stream(&self, req: ChatRequest, cancel: CancelToken) -> BoxStream<ChatEvent>;
    fn health(&self) -> Health;
    fn cost(&self, usage: &Usage) -> Cost;   // z tabeli cen w konfiguracji, nie z kodu
}
```
Zdarzenia: `provider.call.started/finished` (dostawca, model, tokeny, koszt, opóźnienie; strumień ModelCalls), `provider.error` (429/5xx/timeout), `provider.refusal`, `provider.circuit.opened`.

## Zależności
`core-bus/config/log-contract`, `accounts-hub-contract` (SecretRef, katalog), `cost-meter-contract`, `compliance-contract`, `sessions-contract` (HistoryView). Zewnętrzne: klient HTTP (do ustalenia w `docs/vendor/`).

## Niezmienniki
- Historia jest append-only: adapter renderuje przerwaną turę jako pełną turę + notka „użytkownik usłyszał tylko: „…" i przerwał" (Anthropic) albo używa natywnego truncate (PLAN §6.5); nigdy nie edytuje wcześniejszych tur.
- Anthropic: `effort` ustawiany jawnie (domyślnie `medium`), `tool_choice: auto` + `strict: true` (nie `any/tool`), obsługa `stop_reason: refusal`, stabilny prefiks cache (narzędzia → system → wiadomości) — do weryfikacji w spike.
- Klucz pobierany z `SecretRef` w momencie wywołania; nigdy nie logowany.
- Żądanie z tagiem „prywatne" nie idzie do dostawcy z tagiem CN/„może trenować" (egzekwuje Router; adapter dodatkowo odmawia — obrona w głąb).
- Anulowanie kończy strumień ≤ 100 ms i przerywa połączenie HTTP.
- Ceny z konfiguracji (tabela w katalogu dostawców), nie z kodu.

## Zdolności / uprawnienia
`net.egress(host)` per dostawca (host z katalogu; egress-allowlista Jądra).

## Izolacja
`inproc`, `lazy` (adapter ładowany przy pierwszym użyciu).

## Budżet zasobów
RAM ≤ 4 MB per aktywny adapter; narzut adaptera na TTFT ≤ 20 ms; przełączenie fallback ≤ 2 s bez utraty wiadomości (ACC F1).

## Konfiguracja (klucze TOML)
`[providers.api.<id>] base_url, default_model, effort = "medium", timeout_s = 60, max_retries = 2`, `[providers.api.<id>.pricing] input_per_mtok_usd, output_per_mtok_usd` (z katalogu, nadpisywalne).

## Wkład do UI
Chip profilu modelu w composerze, szczegóły wiadomości (model, tokeny, koszt, opóźnienie), stany 429/wyczerpane okno (§14.4), karta „dodaj klucz, aby odblokować".

## Testy akceptacyjne
- `ACC-F1-providers-api-01`: ≥ 3 adaptery zielone na fixture'ach syntetycznych ze schematów API (nagrywane automatycznie przy pierwszym kluczu; walidacja na żywo = nightly, nieblokująca).
- `ACC-F1-providers-api-02`: sztuczny 5xx/timeout → fallback (Router) ≤ 2 s, 0 utraconych wiadomości.
- `ACC-F1-providers-api-03`: rendering historii z barge-in (prefiks) zgodny z regułą append-only dla każdego adaptera.

## Fake
`providers-api-fake`: record/replay (kasety NDJSON), skryptowane błędy (429, 5xx, timeout, refusal), sterowana prędkość strumienia (tokeny/s) i TTFT z wirtualnym zegarem.

## Otwarte pytania
- Umiejscowienie wspólnego kontraktu `ModelProvider` (osobny crate `providers-common-contract`?) — do ustalenia w SPEC v1 / ADR (5).
- Spike (g) bloki myślenia Anthropic — odroczony do klucza; ADR (6) tymczasowy.
