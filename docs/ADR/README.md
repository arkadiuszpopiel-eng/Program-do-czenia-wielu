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
| [0016](0016-jeden-wykonawca-recenzja-w-swiezym-kontekscie.md) | Jeden wykonawca (Claude Code); recenzja w świeżym kontekście zamiast drugiego modelu | Zaakceptowany |
| [0017](0017-strategia-wieloplatformowa.md) | Strategia wieloplatformowa: Windows 11 pierwszy, potem panel WWW, Android 15+ i Linux | **Proponowany** |
| [0018](0018-fabryka-platform-i-kontrakty-niezalezne-od-systemu.md) | Fabryka platform (`platform-select`) i kontrakty Brokera niezależne od systemu | **Proponowany** |
| [0019](0019-protokol-niezalezny-od-transportu.md) | Protokół klient–host niezależny od transportu (ramki z wersją, `seq`, ACL, typy TS generowane) | **Projekt zaakceptowany12.11.2026; implementacja do przeglądu** |
| [0020](0020-host-bez-okna.md) | Host bez okna: binarka `alfa-host`, okno Tauri jako klient lokalny | **Projekt zaakceptowany12.11.2026; implementacja do przeglądu** |
| [0021](0021-dostep-zdalny-przekaznik-e2e-i-parowanie.md) | Dostęp zdalny: przekaźnik, kanał end-to-end i parowanie urządzeń | **Projekt zaakceptowany12.11.2026; implementacja do przeglądu** |
| [0022](0022-zgody-zdalne-dowod-urzadzenia-sparowanego.md) | Zgody zdalne: dowód urządzenia sparowanego (`PairedDevice`) | **Projekt zaakceptowany12.11.2026; implementacja do przeglądu** |
| [0023](0023-tryb-bez-komputera-klucz-api-i-scalanie-rozmow.md) | Tryb bez komputera: klucz API na urządzeniu i scalanie rozmów | **Projekt zaakceptowany12.11.2026; implementacja do przeglądu** |
| [0024](0024-licencja-silnika-mowy.md) | Licencja silnika mowy (następca Pipera na GPL-3.0) | **Proponowany** |
| [0025](0025-kamery-i-biometria-twarzy.md) | Kamery i biometria twarzy: zgoda, dane tylko lokalnie, rozpoznanie nie jest autoryzacją | **Zaakceptowany** (2026-10-11) |
| [0026](0026-glos-z-chmury-jako-opcja.md) | Głos agentek z chmury jako opcja obok głosu lokalnego (domyślnie wyłączony, nigdy w sesji prywatnej) | **Proponowany** |
| [0027](0027-wielu-uzytkownikow-konta-i-role.md) | Wielu użytkowników: konta, logowanie i role | **Proponowany** |
| [0028](0028-directml-dla-glosu-i-mowa-na-kartach-amd-intel.md) | DirectML dla głosu Supertonic (opcjonalnie, karta ≥ 12 GB) i Qwen3-ASR przez Vulkan na kartach AMD/Intel | **Proponowany** |
| [0029](0029-konta-dzieci.md) | Konta dzieci: przedziały wieku, podgląd rodzica, zgoda na chmurę, odznaki osobiste | **Przyjęty** |

| [0030](0030-sqlcipher-rustsec-2026-0459.md) | Naprawiony SQLCipher bez wyjątku advisory RUSTSEC-2026-0459 | **Proponowany; wąska łatka licencji zatwierdzona** |
| [0031](0031-wbudowana-przegladarka-webview2.md) | Wbudowana przeglądarka WebView2, dodatki i rzeczywiste wyszukiwanie | **Proponowany; pierwsza ścieżka bezAPI wybrana12.11** |

Statusy: **Zaakceptowany** — obowiązuje; dopisek „(spike …)" oznacza, że wartości liczbowe lub wykonalność potwierdza wskazany spike w F0 bez zmiany kierunku. **Tymczasowy** — kierunek przyjęty, ale ADR wraca do przeglądu po wskazanym spike'u. **Proponowany** — projekt przygotowany przez sesję AI; nie obowiązuje do podpisu właściciela.
