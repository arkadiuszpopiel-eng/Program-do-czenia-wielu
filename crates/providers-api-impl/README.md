# providers-api-impl

Adaptery `ModelProvider` dla API chmurowych (docs/modules/providers-api/SPEC.md, PLAN §5.2–5.6),
manifest `module.toml` (`providers-api`, `inproc`, `lazy`).

| Adapter | Protokół | Uwagi |
|---|---|---|
| `AnthropicProvider` + `AnthropicOptions::native()` | Messages API, SSE | `x-api-key`, `anthropic-version: 2023-06-01`; `thinking: adaptive` (dla Opus 5.5 nigdy `disabled`), `output_config.effort` jawnie (domyślnie `medium`), `tool_choice any/tool` → `auto` gdy model nie wymusza, `strict` + `eager_input_streaming` na narzędziach, `temperature` pomijana bez próbkowania, cache `tools → system → messages` (`cache_control` na systemie i ostatnim bloku), `fallbacks: "default"` (beta `server-side-fallback-2026-07-01`), blok `fallback` → `ModelSwitched`, `refusal` + `stop_details`, 429 `retry-after`, 529 overloaded, jednorazowe ponowienie bez myślenia po 400 „bound to a different conversation"; Models API ze stronicowaniem |
| `AnthropicProvider` + `AnthropicOptions::compatible()` | Messages API | endpointy zgodne (Z.ai…): bez bet, `fallbacks`, `eager_input_streaming`; nieznany model = nic opcjonalnego |
| `OpenAiProvider` + `OpenAiOptions::native()` | Responses API | `store: false`, `include: reasoning.encrypted_content` → blok myślenia z podpisem `<id>:<encrypted>` odsyłany bez zmian |
| `OpenAiProvider` + `native_chat()`/`compatible()` | Chat Completions | `max_completion_tokens`/`max_tokens`, `stream_options.include_usage`, `reasoning_content` → myślenie, równoległe narzędzia, `insufficient_quota` → `Auth` |
| `build_provider(CatalogEntry, AccountProfile)` | wg `compat` | adapter generyczny z `providers-catalog/<id>.toml`: `base_url` (konto > katalog > natywny domyślny), auth, prywatność/jurysdykcja, możliwości modelu domyślnego z tribooli |

Wspólny silnik (`engine.rs`, `run.rs`): zadanie tła na żądanie; ponawianie z backoffem i jitterem
**tylko przed pierwszym tokenem** i tylko dla odrzuceń idempotentnych (429/5xx/529, błąd połączenia;
`retry-after` ≤ 1 s, budżet 1,5 s); limity connect / first-token / idle; anulowanie i upuszczenie
strumienia zrywają połączenie (test: < 100 ms po stronie serwera); klucz z `SecretSource` w chwili
wywołania, nagłówek wrażliwy, treść błędów redagowana; zdrowie z historii wywołań. Koszt i
oszacowanie wyłącznie z cennika profilu. `ObservedProvider` publikuje `provider.call.started/finished`
(`EventKind::ModelCall` + `Cost`), `provider.error`, `provider.refusal` — bez treści rozmowy.

Testy (`tests/`): serwer fixture HTTP/1.1 na surowym TCP (strumieniowanie z pauzami, wykrywanie
rozłączenia) + nagrania SSE zbudowane wg dokumentacji; zestaw kontraktowy na 5 wariantach adapterów;
kształt żądań, błędy, ponowienia, Models API, osadzenia, katalog, moduł i zdarzenia magistrali,
symulacja fallbacku Routera ≤ 2 s. Bez internetu i bez kluczy.
