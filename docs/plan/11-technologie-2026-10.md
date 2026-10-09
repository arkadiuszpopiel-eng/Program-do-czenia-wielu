# 11 — Technologie, stan na 2026-10-09

Przegląd wykonany 2026-10-09. Świat AI zmienia się co kilka tygodni, więc każda pozycja ma etykietę pewności,
a przed rozpoczęciem zadania, które od niej zależy, trzeba ją sprawdzić ponownie.

| Etykieta | Znaczenie |
| --- | --- |
| **P** | potwierdzone tego dnia w źródle pierwotnym (rejestr pakietów, strona modelu, dokumentacja projektu) |
| **W** | znane ze źródeł wtórnych (artykuły, zestawienia) — do potwierdzenia przed wdrożeniem |
| **U** | niesprawdzone w tym przeglądzie — wiedza ogólna, wymaga sprawdzenia |

## 1. Uruchamianie modeli lokalnie

| Technologia | Stan | Pewność | Źródło | Wniosek dla Alfy |
| --- | --- | --- | --- | --- |
| llama.cpp `llama-server` | wywołania narzędzi (`--jinja`), obraz/dźwięk przez libmtmd (eksperymentalne), tryb routera wielu modeli, usypianie bezczynnych, gramatyki i JSON Schema, `/v1/embeddings`, `/v1/rerank`, `/v1/responses` | P | README serwera w repozytorium `ggml-org/llama.cpp` | zostaje jedynym silnikiem v1 (ADR 0014) |
| Wbudowane narzędzia i MCP w `llama-server` | istnieją (`--tools`, `--mcp-servers-config`), z ostrzeżeniem autorów | P | jw. | **nie włączać** — omijają Brokera |
| DirectML | tryb podtrzymania, bez nowych funkcji | P | learn.microsoft.com/windows/ai/directml | nie planować |
| Windows ML / Foundry Local | Windows ML ogólnie dostępne od 2025-09; Foundry Local od 2026-04 | P | blogi Microsoft (Windows Developer, Foundry) | opcja na później dla układów NPU (H.4) |
| mistral.rs, candle, Ollama, LM Studio (tryb bez okna) | istnieją i są rozwijane | W | — | nie zmieniamy silnika |
| vLLM, ONNX Runtime GenAI, ExecuTorch, MLC | — | U | — | poza zakresem |

## 2. Modele mieszczące się w 6 GB

| Model | Dane | Pewność | Źródło | Rola |
| --- | --- | --- | --- | --- |
| Bielik-Minitron-7B v3.0 Instruct | GGUF: Q4_K_M 4,5 GB, Q5_K_M 5,29 GB; Apache 2.0 | P | huggingface.co/speakleash | polski czat domyślny |
| Bielik-11B v3.0 Instruct | Apache 2.0; ok. 6,7 GB w Q4 (szacunek) | P / W | jw. | tryb „wolniej, ale lepiej” |
| Bielik-Guard 0.1B / 0.5B | klasyfikatory bezpieczeństwa | P | jw. | drugi strażnik w Brokerze (C2.4) |
| Qwen3.5 — 0.8B / 2B / 4B / 9B | opublikowane 2026-02-27/28 | P (istnienie i daty) | API Hugging Face, konto `Qwen` | 4B: narzędzia i obraz |
| Qwen3.5 — licencja, kontekst, wizja | Apache 2.0, 262 tys. tokenów, wizja wbudowana | W | artificialanalysis.ai | potwierdzić na karcie modelu |
| Gemma 4 — E2B / E4B / 12B / 26B / 31B | E2B i E4B od 2026-03-02; 12B od 2026-05-23; oficjalne GGUF QAT | P (istnienie i daty) | API Hugging Face, konto `google` | E4B: wejście dźwiękowe (eksperyment) |
| Gemma 4 — licencja Apache 2.0, dźwięk i obraz | — | W | the-decoder.com, datanorth.ai | potwierdzić |
| PLLuM (rodzina 2512) | Llama-PLLuM-8B i 70B; licencje różne | P (2025-12) | huggingface.co/CYFRAGOVPL | sprawdzić licencję wariantu przed użyciem |

Nie potwierdziliśmy: natywnego wywoływania narzędzi w Bieliku v3 (stąd C2.3 — wymuszanie schematem) ani
istnienia Bielika v3 mniejszego niż 5 mld parametrów (istotne dla telefonu — H.1).

## 3. Standardy agentów

| Standard | Stan | Pewność | Źródło | Wniosek |
| --- | --- | --- | --- | --- |
| MCP, specyfikacja 2026-07-28 | przebudowa niezgodna wstecz: bezstanowa, bez `initialize`, obowiązkowe `server/discover`, zadania jako rozszerzenie; wycofywane: Roots, Sampling, HTTP+SSE | P | modelcontextprotocol.io/specification/2026-07-28/changelog | obsłużyć obie wersje (C2.5); nie budować na funkcjach wycofywanych |
| MCP Apps | rozszerzenie oficjalne (SEP-1865) | P | modelcontextprotocol.io/seps/1865 | H.2 |
| SDK Rust `rmcp` | 3.5.1 (2026-10-05) | P | crates.io | używać |
| Zarządzanie MCP | Agentic AI Foundation (Linux Foundation) od 2025-12 | P | blog.modelcontextprotocol.io | standard stabilny organizacyjnie |
| Agent Skills (`SKILL.md`) | otwarty standard od 2025-12, ok. 40 klientów | W | agentskills.io, zestawienia | format umiejętności Alfy (C2.6) |
| AGENTS.md | konwencja powszechna | W | — | już używamy |
| A2A | wersja 1.0 w 2026, w AAIF od 2026-08 | W | aaif.io | odłożone (H.6) |
| ACP (protokół agent–edytor) | Zed, JetBrains | P (2025) | zed.dev/acp | H.5 |
| OpenAI: Assistants API | wyłączone 2026-08-26; zostaje Responses API | P | community.openai.com | adaptery tylko na Responses / Chat |

## 4. Głos

| Technologia | Stan | Pewność | Źródło | Wniosek |
| --- | --- | --- | --- | --- |
| Whisper large-v3 / turbo (whisper.cpp) | najlepszy otwarty model dla polskiego wg badania z 2026-03 | P | arxiv.org/abs/2603.02246 | punkt odniesienia |
| NVIDIA Parakeet-TDT-0.6b-v3 | 25 języków w tym polski (WER ok. 7,3 % na FLEURS), CC-BY-4.0 | P | huggingface.co/nvidia/parakeet-tdt-0.6b-v3 | kandydat — rozstrzyga pomiar na własnym korpusie |
| Silero VAD v6 | dostępny w ONNX | W | — | już używamy Silero |
| Piper — głosy polskie | `gosia`, `darkman`, `mc_speech` i inne | P | huggingface.co/rhasspy/piper-voices | zostaje |
| Piper — licencja | pierwotne repozytorium zarchiwizowane 2025-10; następca na GPL-3.0 | W | — | **ADR 0024** |
| Kokoro, Kyutai, Moonshine | bez polskiego | W / P | kyutai.org | nie |
| Pełny dupleks (PersonaPlex, Moshi) | angielski, 16–24 GB | W | — | H.3 |
| Słowo wywoławcze | Porcupine: koniec darmowego planu 2026-06; openWakeWord: ostatnie wydanie 2024 | W | — | opcja eksperymentalna; domyślnie „naciśnij i mów” |

## 5. Pamięć i wyszukiwanie

| Technologia | Stan | Pewność | Wniosek |
| --- | --- | --- | --- |
| Wyszukiwanie hybrydowe (FTS5 + wektory, RRF) + reranker | praktyka powszechna | W | już tak działa moduł `search` |
| sqlite-vec | wyszukiwanie pełne stabilne; indeksy przybliżone w wersji alfa | P (github.com/asg017/sqlite-vec) | zostaje (ADR 0008); wystarcza do ok. 100 tys. fragmentów |
| LanceDB | wbudowana, Rust, Apache 2.0 | P (docs.lancedb.com) | zapas za portem, gdy skala urośnie |
| Embeddery: Qwen3-Embedding 0.6B, EmbeddingGemma-300m, BGE-M3 | dostępne lokalnie | P (2025) | wybór po pomiarze recall@5 po polsku |
| Docling, MarkItDown | rozwijane; oba w Pythonie | W | tylko jako opcjonalny dodatek (ADR 0004: bez Pythona w rdzeniu) |

## 6. Sterowanie komputerem i bezpieczeństwo agentów

| Temat | Stan | Pewność | Wniosek |
| --- | --- | --- | --- |
| Modele GUI o otwartych wagach | ok. 66–77 % OSWorld-Verified (dane autorów), słabo przenoszą się między zestawami | W | nie obiecywać autonomii na 6 GB |
| Podejście: najpierw drzewo dostępności, zrzut jako zapas | praktyka wspólna dla Playwright MCP i Chrome DevTools MCP | W | zgodne z projektem `tools-uia` |
| OWASP Top 10 dla aplikacji agentowych 2026 (ASI01–ASI10) | opublikowane 2025-12 | W (genai.owasp.org) | mapować na obrony Brokera w zestawie ataków |
| OWASP LLM Top 10, wydanie 2026 | opublikowane ok. 2026-08 | W | przeczytać oryginał przed pentestem (C1.5) |
| Wstrzykiwanie poleceń | nierozwiązane; obrona architektoniczna (plan-potem-wykonanie, znakowanie treści, zgody) | P (arxiv.org/abs/2506.08837) | zgodne z modelem Brokera |
| Warstwa agentowa w Windows 11 | podgląd, domyślnie wyłączona | W | obserwować |

## 7. Wiele platform

| Technologia | Stan | Pewność | Źródło | Wniosek |
| --- | --- | --- | --- | --- |
| Tauri | 2.12.2 (2026-10-09); Android i iOS stabilne od 2.0 | P | crates.io, tauri.app | zostajemy przy 2.x |
| Tauri 3 | alfa (środowisko CEF) | W | — | nie planować (AGENTS.md) |
| Tauri na Linuksie | WebKitGTK; udokumentowane kłopoty z kartami NVIDIA | P | v2.tauri.app/develop/debug/linux-graphics | test w CI (G.8) |
| Tauri na Androidzie — praca w tle | brak gotowej wtyczki usługi pierwszoplanowej | U | — | własne wtyczki Kotlin (F.2, F.5) |
| LiteRT-LM, AICore / Gemini Nano | dostępne na wybranych telefonach | P | developers.google.com | H.1 |
| Flatpak / AppImage / deb | Flatpak niesie własne środowisko; AppImage bywa kruchy z WebKitGTK | W | — | deb i rpm najpierw |

## 8. Dostęp zdalny i dane między urządzeniami

| Technologia | Stan | Pewność | Źródło | Wniosek |
| --- | --- | --- | --- | --- |
| iroh | 1.3.0 (2026-09-28); połączenia po kluczu publicznym, QUIC, własny przekaźnik | P | crates.io | opcja dla klientów natywnych |
| Noise (`snow`), WebAuthn / klucze dostępu, WebRTC, Tailscale, Cloudflare Tunnel | dojrzałe, powszechnie używane | U | — | sprawdzić wersje i ograniczenia przed D |
| Web Push w Chrome na Androidzie | działa; synchronizacja okresowa tylko dla zainstalowanych PWA | P | developer.chrome.com | powiadomienia bez treści (E.5) |
| Loro | 1.16.2 (2026-09-21) | P | crates.io | H.7 |
| Automerge 3 | znacznie mniejsze zużycie pamięci | P | automerge.org | H.7 |

## 9. Trendy w asystentach (do porównania zakresu)

Źródła wtórne (**W**). W 2026 roku wiodące asystenty mają: zadania zaplanowane i agentów w tle, pamięć, którą
użytkownik może przeglądać i kasować, łączniki MCP, umiejętności w formacie `SKILL.md`, kanwę / artefakty,
podział pracy na podagentów, zgody i dziennik działań. Alfa ma już w kodzie odpowiedniki większości (harmonogram,
pamięć z inspektorem, MCP, umiejętności, Broker z dziennikiem); plan v6 dodaje brakujące (C2.5–C2.8).
Wyróżnik Alfy, który jest realny na sprzęcie bazowym: działanie w całości lokalnie po polsku i jawny Broker.

## 10. Kamery i obraz (dopisane 2026-10-09 — bez przeglądu źródeł)

Ten rozdział powstał razem ze zleceniem C3 i **nie był sprawdzany w źródłach** — wszystko ma etykietę U albo
odwołuje się do pozycji sprawdzonych wyżej. Przegląd źródeł jest pierwszym krokiem zadań C3.5, C3.6 i C3.8.

| Technologia | Stan | Pewność | Wniosek dla Alfy |
| --- | --- | --- | --- |
| Media Foundation (kamery USB na Windows) | interfejs systemowy Windows | U | przez `windows-rs` wyłącznie w `platform-windows-impl`, za portem `CameraPort` |
| RTSP (strumień kamer sieciowych) | powszechny w kamerach IP i Wi-Fi | U | dekoder w osobnym procesie; kandydat: `ffmpeg` (już sidecar `tools-media`) albo biblioteka w Rust |
| ONVIF (wyszukiwanie i opis kamer w sieci) | standard branżowy | U | tylko sieć lokalna; zakres uzgodnić z regułą „bez nasłuchu” |
| Obraz w `llama-server` | istnieje, eksperymentalny (patrz rozdział 1) | P | droga lokalna opisu obrazu; pomiar pamięci na 6 GB |
| Modele lokalne z wejściem obrazowym na 6 GB | patrz rozdział 2 (Qwen3.5-4B do obrazu) | W | wybór po pomiarze (C3.6) |
| Modele chmurowe z wejściem obrazowym | dostępne u głównych dostawców | U | droga za zgodą; sprawdzić ceny i zasady przechowywania obrazów |
| Wykrywanie twarzy i wzorzec twarzy w ONNX | wiele otwartych modeli | U | **licencje**: część popularnych modeli jest tylko do badań — sprawdzić przed C3.8 |
| Wykrywanie żywej twarzy (ochrona przed zdjęciem) | istnieją modele i metody | U | decyzja w SPEC v1 `person-recognition` |

## Do sprawdzenia przed wdrożeniem

1. Bielik v3: długość kontekstu, zachowanie przy wywołaniach narzędzi, tok/s na RTX 4050.
2. Polskie zestawy do oceny wyszukiwania (PIRB, MTEB-pl) — wybór embeddera i rerankera.
3. Praca w tle i usługa pierwszoplanowa w Tauri na Androidzie.
4. Licencja następcy Pipera i licencje poszczególnych głosów.
5. Oryginalne teksty: OWASP LLM Top 10 2026, rozporządzenia z [`10-zgodnosc-prawna.md`](10-zgodnosc-prawna.md).
6. Wersje i ograniczenia: `snow`, biblioteki WebAuthn, WebRTC w WebView Androida.
7. Dokładny numer wydania llama.cpp, w którym są tryb routera i `/v1/rerank` — przed podbiciem wersji silnika.
