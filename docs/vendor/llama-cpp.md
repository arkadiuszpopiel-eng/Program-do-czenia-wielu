# llama.cpp — sidecar `llama-server` (`providers-local-impl`)

- Wersja: wydanie **b11559** (`LLAMA_TAG` w `crates/app-models/src/data.rs`; archiwa Vulkan / CUDA / CPU z przypiętymi
  SHA-256). Repozytorium: https://github.com/ggml-org/llama.cpp (tag `b11559`).
- Zweryfikowane 2026-10-18 **lekturą źródeł wydania** (nie kompilacją): `tools/server/README.md`,
  `docs/function-calling.md`, `tools/server/server-common.cpp` (`oaicompat_chat_params_parse`),
  `common/chat.cpp`, `common/chat-auto-parser-generator.cpp`, `common/jinja/caps.h`.

## Uruchomienie (`LaunchPlan::args`)
`llama-server --host 127.0.0.1 --port <wolny> --api-key <losowy> -m <gguf> --alias <id> -c <ctx> -ngl <warstwy>
--threads <n> -np 1 [--device <id>] [--jinja]`. `--jinja` jest w b11559 domyślnie włączone (podajemy jawnie dla
modeli z narzędziami). `--list-devices` — lista urządzeń (`devices.rs`).

## HTTP, którego używamy
- `GET /health` → 200 albo 503 (ładowanie). `GET /v1/models`.
- `POST /v1/chat/completions` (`stream: true`, `stream_options.include_usage`, `max_tokens`): SSE
  `chat.completion.chunk` jak OpenAI; `delta.content`, `delta.tool_calls`, `reasoning_content`.

## Narzędzia i wymuszanie schematu (C2.3)
- `tools` wymagają `--jinja`. `tool_choice` czytany jako napis: `auto` | `none` | `required`; inna wartość →
  błąd „Invalid tool_choice”; obiekt (`{"type":"function",…}`) nie jest obsługiwany.
- Format wywołań wykrywa **autoparser szablonu czatu** (`chat-auto-parser-generator.cpp`). Gramatyka argumentów ze
  schematów narzędzi powstaje, gdy szablon ma rozpoznany format narzędzi i: `tool_choice = auto` ze znacznikiem
  wywołania (gramatyka **leniwa** — działa dopiero od znacznika) albo `required` (cała odpowiedź). Szablon bez
  formatu narzędzi → bez gramatyki i bez parsowania wywołań (tekst wywołania trafia do treści). „Template seems to
  support tool calls, but failed to determine tool format” w logu = to samo.
- `parallel_tool_calls` — domyślnie z możliwości szablonu.
- `response_format`: `{"type": "json_object"}`, `{"type": "json_object", "schema": …}` albo
  `{"type": "json_schema", "json_schema": {"schema": …}}` → schemat zamieniany na gramatykę całej odpowiedzi
  (niezależnie od szablonu). Gdy jest `response_format`, parser odpowiedzi ma **pierwszeństwo** przed narzędziami.
- `grammar` razem z `tools` (i `tool_choice` ≠ `none`) → błąd „Cannot use custom grammar constraints with tools”;
  `grammar` razem z `json_schema` → błąd.
- Wniosek dla Alfy: koperta przez `response_format` **bez** pola `tools` (`providers-local-impl/src/envelope.rs`) —
  gwarancja poprawnego JSON-u niezależna od szablonu Bielika; wyłączenie (`engines.llm.tool_schema`) wraca do `tools`.
- Konwerter JSON Schema → gramatyka (`common/json-schema-to-grammar.cpp`) układa pola obiektu w kolejności ze
  schematu (najpierw wymagane, potem opcjonalne), a `common_json` zachowuje kolejność kluczy z żądania; `serde_json`
  w Alfie układa klucze alfabetycznie — stąd `name` przed `parameters` w kopercie. `const` i `enum` są obsługiwane.
  Brak `additionalProperties` konwerter b11559 traktuje jak brak dodatkowych pól (wniosek z kodu konwertera) —
  Alfa i tak domyka obiekty jawnie, żeby walidacja argumentów oceniała to samo co gramatyka.

## Pomiar na prawdziwym modelu (2026-10-29, laptop RTX 4050, `llama-cuda` b11559, Bielik 4.5B Q8_0)
- Gramatyka działa: każde wywołanie w 35 zadaniach F3 miało argumenty zgodne ze schematem; ok. 33 tok/s, pierwsza
  odpowiedź w kopercie ok. 2 s (próba bezpośrednia na serwerze).
- Koperta `{"response", "tool_call"?}` z `response` na początku → model wpisywał wywołanie jako tekst do `response`
  (0/35). Wywołanie jako wybór na początku (`anyOf`: obiekt z wymaganym `tool_call` albo sam `response`) → 5/35;
  z poprawkami runtime (ścieżki względne, wykrywanie powłoki, podpowiedź przy powtórzeniu) → 11/35 (`fs` 9/24,
  `shell` 2/11). Szczegóły i pozostałe przyczyny porażek: `docs/plan/44-raport-2026-10-30-noc.md`.
- `anyOf` w korzeniu schematu konwerter b11559 przyjmuje (gramatyka z alternatywą).

## Czego nie sprawdziliśmy
- Bielika 1.5B i innych kwantyzacji niż Q8_0 w kopercie; backendu Vulkan (iGPU) z kopertą.
- Pełnej listy słów kluczowych JSON Schema obsługiwanych przez konwerter gramatyki; schemat, którego nie przetworzy,
  kończy się błędem żądania.
