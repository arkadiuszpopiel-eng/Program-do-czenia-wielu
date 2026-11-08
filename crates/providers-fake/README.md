# providers-fake

Deterministyczna atrapa `ModelProvider` (docs/modules/providers-api/SPEC.md §Fake) — **tylko jako
`dev-dependency`** testów innych modułów (Router, agent-runtime, voice-dialog).

- `FakeProvider`: kolejka skryptów na kolejne żądania + skrypt domyślny; profil prywatności,
  cennik, możliwości modeli, limity czasu (`FakeTimeouts`) konfigurowalne; `health` z historii.
- `Script`/`Step`: `text`, `chunks` (z odstępami), `streamed` (TTFT + tokeny/s), `tool_call`,
  `thinking` (z podpisem), `refusal`, `max_tokens`, `http_error` (klasyfikacja jak w adapterach),
  `error`, `stall`; `Step::Delay` używa `tokio::time::sleep` → w testach z
  `#[tokio::test(start_paused = true)]` **wirtualny zegar** (deterministyczne TTFT i opóźnienia).
- Wstrzykiwanie błędów: `fail_next(ProviderError)` (429/5xx/timeout…), `Step::Stall` + limity.
- Weryfikacja: `requests()` (po walidacji, prywatności i projekcji historii — tak jak „na drucie"),
  `calls()` (wszystkie wywołania).
- Record/replay: `Recorder<P>` nagrywa kasety NDJSON z dowolnego dostawcy (np. adaptera na żywo
  po dodaniu klucza), `FakeProvider::from_cassette` je odtwarza z zachowaniem odstępów czasu.
- `embed`: deterministyczne wektory 8-wymiarowe (FNV-1a).

Testy: pełny zestaw kontraktowy w czasie wirtualnym i rzeczywistym + testy własne atrapy.
