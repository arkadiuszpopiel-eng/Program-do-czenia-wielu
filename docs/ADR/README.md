# Rejestr decyzji architektonicznych (ADR)

Każda decyzja techniczna z `docs/PLAN.md` §1.2 ma swój ADR. Decyzje są odwracalne do końca Fali 0 (akceptacja ADR-ów to bramka ludzka #7). Format każdego ADR: tytuł, Status, Data, Kontekst, Decyzja, Alternatywy (odrzucone i dlaczego), Konsekwencje, Jak cofnąć, Powiązane sekcje planu.

| Nr | Tytuł | Status |
|---|---|---|
| [0001](0001-stos-rust-tauri-svelte.md) | Stos: Rust + Tauri 2 + Svelte 5 | Zaakceptowany |
| [0002](0002-mikrojadro-i-format-modulu.md) | Mikrojądro i format modułu (trójka crate'ów, manifest) | Zaakceptowany |
| [0003](0003-izolacja-brokera-i-broker-ui.md) | Izolacja Brokera i Broker-UI na wyższym poziomie integralności | Zaakceptowany (spike k) |
| [0004](0004-ml-runtime-bez-pythona.md) | ML runtime bez Pythona (whisper.cpp Vulkan/CUDA, ONNX CPU) | Zaakceptowany (spike e, h) |
| [0005](0005-agentbackend-vs-modelprovider.md) | Dwa kontrakty: `ModelProvider` i `AgentBackend` | Zaakceptowany |
| [0006](0006-historia-append-only-i-galezie.md) | Historia rozmowy append-only + gałęzie | **Tymczasowy** (spike g, po kluczu Anthropic) |
| [0007](0007-instalacja-launcher-side-by-side-webview2.md) | Instalacja: stały launcher, wersje side-by-side, stały folder WebView2 | Zaakceptowany (spike j) |
| [0008](0008-dane-sqlite-szyfrowane-sqlite-vec-fts5.md) | Dane: SQLite szyfrowane + sqlite-vec + FTS5 | Zaakceptowany (spike i) |
| [0009](0009-markdown-w-rust-iframe-dla-artefaktow.md) | Markdown renderowany w Rust; iframe tylko dla artefaktów | Zaakceptowany |
| [0010](0010-regula-skrotow-globalnych-altgr.md) | Reguła skrótów globalnych (AltGr) i kill-switch `Ctrl+Shift+F12` | Zaakceptowany |
| [0011](0011-silniki-glosu-v0-i-audio-wasapi-aec.md) | Silniki głosu v0 i audio (`wasapi`, AEC z własną referencją) | Zaakceptowany (Voice Lab) |
| [0012](0012-wtyczki-wasmtime-wit.md) | Wtyczki: wasmtime + własny WIT | Zaakceptowany |
| [0013](0013-kontrakty-i-codegen-ts.md) | Kontrakty i codegen TS (`tauri-specta`/`ts-rs`), JSON Schema, WIT | Zaakceptowany |
| [0014](0014-lokalny-llm-llama-cpp-zamiast-ollamy.md) | Lokalny LLM: llama.cpp (Vulkan/CUDA) zamiast Ollamy | Zaakceptowany |
| [0015](0015-model-uprawnien-tokeny-zdolnosci-l0-l4.md) | Model uprawnień: tokeny zdolności i poziomy L0–L4 | Zaakceptowany |

Statusy: **Zaakceptowany** — obowiązuje; dopisek „(spike …)" oznacza, że wartości liczbowe lub wykonalność potwierdza wskazany spike w F0 bez zmiany kierunku. **Tymczasowy** — kierunek przyjęty, ale ADR wraca do przeglądu po wskazanym spike'u.
