# models — SPEC (v1, menedżer modeli i silników, `app-models`)

## Cel
Jedno miejsce w aplikacji do pobrania, sprawdzenia i usunięcia wszystkiego, czego Alfa potrzebuje lokalnie: modele
rozmowy (GGUF z `providers-local`), STT (whisper GGML), głosy TTS (Piper; Pocket — ręcznie), VAD (Silero), cechy
słów wywoławczych (openWakeWord), model mówcy (ECAPA/WeSpeaker), embeddingi (`lib_embed::CATALOG`) i sidecary
(`llama-server`, `whisper-server`, `piper`). Do tego wybór embeddera wyszukiwania i przebudowa wektorów (F7-02).
Odblokowuje F3-12 (instalator modeli/sidecarów) i pomiary głosu w aplikacji (PLAN §1.2, §8.7, §10).

## Fala i priorytet
F3 (MVP bez kluczy) + F7 (embedder). P0. Kategoria `app-*` (korzeń kompozycji): crate `app-models`, bez trójki
`-contract/-impl/-fake` — kontraktem dla UI są DTO `app-api` (`dto/models.rs`) i komendy z `COMMANDS.md`.

## Kontrakt
```rust
// app-models
pub struct ItemSpec { id, kind: ModelItemKind, name, license, source, root: Root /* Models | Sidecars */, dir,
                      files: Vec<FileSpec { name, url, size, sha256: Option<String> }>, install: Install,
                      confirmed: bool, note: LocalizedText }
pub enum Install { Files, Gguf /* + <plik>.sha256 */, Embed(EmbedManifest) /* + embed.json */,
                   Speaker { manifest, template }, Pick(Vec<Pick>) /* wpisy z ZIP */,
                   Tree { strip, require } /* całe drzewo ZIP */, Manual(Vec<String>) }
impl ModelsApp { list, download, cancel, verify, remove, trust_hash, activate_embedder,
                 reindex_start, reindex_cancel, reindex_status }
pub fn startup_embedder(models, setting, lexical) -> Arc<dyn Embedder>;   // wybór przy budowie `search`
```
Komendy: `models_list`, `models_download {itemId}`, `models_cancel`, `models_verify`, `models_remove`,
`models_trust_hash {itemId, hashes}`, `embed_model_activate {model}`, `search_reindex_start/cancel/status`.
Zdarzenia UI: `ModelProgress {item_id, file, done, total}` (co ≥ 200 ms), `ModelChanged {item}`,
`ReindexStatus {status}`; magistrala: `search.reindex.{started,progress,done}` z `search-impl` (tylko liczniki).

## Stany pozycji
`missing → queued → downloading → (needs_trust) → installing → installed`; `paused` (plik `.part` czeka na
wznowienie), `failed` (ostatni błąd), `corrupt` (weryfikacja), `external` (pliki skopiowane ręcznie, bez rekordu).
Stan wykrywany tanio (bez hashowania): zadanie w toku → błąd → rekord `state\models\<id>.json` →
`downloads\<id>\pending.json` → plik `.part` → obecność plików wynikowych.

## Niezmienniki
- Tylko `https://` (przekierowania też); `http://127.0.0.1` wyłącznie w testach (`loopback_http`). Stały
  `User-Agent` bez wersji, bez ciasteczek, bez telemetrii; żadnych sekretów w plikach stanu.
- Pobieranie do katalogu roboczego `%LOCALAPPDATA%\Alfa\downloads\<id>\` (`.part`, `Range: bytes=n-`, ≤ 3
  automatyczne wznowienia, twardy limit 2× rozmiar z katalogu, min. +64 MiB); na miejsce docelowe trafia dopiero
  plik zweryfikowany. Przypięty SHA-256 niezgodny → plik usunięty, stan `failed`.
- Plik bez przypiętego hasha → `needs_trust`: UI pokazuje policzony SHA-256 i licencję; instalacja wyłącznie po
  `models_trust_hash` z hashami równymi policzonym, a przed instalacją pliki są hashowane ponownie (podmiana po
  pobraniu → usunięcie). Rekord instalacji zapamiętuje `trusted: true`.
- Rozpakowanie ZIP: `updater_contract::validate_package_path` (bez `..`, `\`, `:`, nazw urządzeń, kropki/spacji
  na końcu), bez dowiązań, bez duplikatów (wielkość liter), `PackageLimits` (wpisy, rozmiar wpisu i całości,
  stopień kompresji), rzeczywiste bajty ≤ zadeklarowane; drzewo do `.staging-<katalog>` i `rename`; błąd nie rusza
  poprzedniej instalacji. Wpis wybrany z paczki (PyPI) sprawdzany przypiętym SHA-256.
- Limit równoległych pobrań (domyślnie 2), anulowanie czeka na koniec zadania (bez dwóch piszących do `.part`).
- Aktywnego embeddera nie można usunąć; zmiana embeddera = `SqliteSearch::set_embedder` (migawka na operację)
  + przebudowa wektorów w tle (bazy sesji + bazy zakresów pamięci); w trakcie wyszukiwanie wektorowe działa jak FTS.

## Zależności
`app-api` (DTO, `EventHub`, `AppPaths`), `search-impl` (`set_embedder`, `spawn_reindex`), `memory-impl`
(`ScopeDbs`, `index_label`), `lib-embed` (katalog, `OnnxEmbedder`, manifest), `providers-local-impl` (manifest
GGUF, rekord `.sha256`), `updater-contract` (reguły ścieżek, limity), `model-residency-contract`, `core-config-contract`.
Zewnętrzne (już w `Cargo.lock`): `reqwest` (rustls), `zip`, `flate2`, `sha2`, `tokio-util`.

## Konfiguracja
`[search.embedder] model = "multilingual-e5-small" | "lexical"` (warstwa wspólna; brak = model domyślny, używany,
gdy jest zainstalowany). Katalogi: `%LOCALAPPDATA%\Alfa\{models,sidecars,downloads,state\models}`. Opcje kompozycji
(`ModelsOptions`): `parallel = 2`, wybór embeddera 20 s po uruchomieniu; pełny przebieg przebudowy na starcie tylko,
gdy ostatni bezbłędny był dla innego embeddera albo ponad 7 dni temu (`state\models\reindex.json`: identyfikator
embeddera i czas — przebieg otwiera wszystkie bazy sesji, które zostają w pamięci podręcznej `sessions`).

## Katalog — stan danych (2026-10-04)
Potwierdzone (PyPI, hash archiwum z API i hash wpisu policzony lokalnie): Silero VAD 6.2.3 (`silero_vad_op18_ifless.onnx`
= `voice_vad_impl::KNOWN_MODELS`), openWakeWord 0.5.1 (`melspectrogram.onnx`, `embedding_model.onnx`).
**Do potwierdzenia przez człowieka** (HF/GitHub zablokowane w sesji, SHA-256 nieprzypięte): Bielik GGUF, e5-small
i MiniLM (ONNX + tokenizer), whisper `large-v3-turbo-q5_0` / `small-q5_1`, Piper `pl_PL-gosia-medium` (licencja
głosu), WeSpeaker ResNet34 (warunki VoxCeleb; okno 200 ramek), `llama-server` (wydanie `b6710` — numer do
przypięcia, układ ZIP), `whisper-server` 1.8.1 (`Release/`), `piper` 2023.11.14-2. Pocket TTS — instalacja ręczna.

## Testy
`crates/app-models/tests/`: `download.rs` (lokalny serwer HTTP: pobranie z postępem, zerwanie → `Range`, anulowanie
→ wznowienie, zły hash → usunięcie, TOFU: zły/pusty/zmieniony hash odrzucony, limit równoległości, limit rozmiaru,
tylko HTTPS, GGUF widoczny dla `providers-local`, weryfikacja i usuwanie, drzewo ZIP sidecara), `unpack.rs` (13
złośliwych archiwów bez skutków ubocznych + poprawne drzewo i wpisy PyPI), `embed.rs` (zabawkowy model `lib-embed`:
pobranie → aktywacja → przebudowa sesji i zakresu globalnego → wyszukiwanie wektorowe; powrót do leksykalnego;
wybór przy starcie). `app-core/tests/commands.rs::model_manager_lists_catalog_and_switches_embedder`. UI: vitest
`fake-engines.test.ts`, `logic/models.test.ts`, E2E `e2e/models.spec.ts` (axe w obu motywach).

## Otwarte pytania
- Przypięcie SHA-256 i potwierdzenie adresów/licencji pozycji „do potwierdzenia” (bramka ludzka; zmiana `confirmed`).
- Buildy Vulkan/CUDA `whisper-server` (wiele archiwów na pozycję działa od fali 5 — `Install::Tree` z kilkoma plikami).
- Podpisy Authenticode binariów sidecarów (P-08) — dziś tylko SHA-256/TOFU.
- Przebudowa (`search-impl::reindex_all`) otwiera naraz wszystkie bazy sesji przez `SessionDbProvider` (pamięć
  podręczna `sessions` ich nie zamyka) — przy setkach sesji RAM rośnie; do rozważenia zamykanie baz po kroku.
- `models_local_download` (`providers-local`) zostaje dla zgodności, ale zapisuje hash bez zgody w UI — do decyzji,
  czy wyłączyć (onboarding korzysta już z menedżera).

## Przegląd bezpieczeństwa #3 (2026-10, `docs/reviews/2026-10-security-review-3.md`) — decyzja człowieka
- **P3-04:** sidecary wykonywalne (`llama-server`, `whisper-server`, `piper`) pobierane bez przypiętego SHA-256 (tylko TOFU) — przypiąć skróty potwierdzonych wydań albo zablokować pobieranie nieprzypiętych plików wykonywalnych (THREAT_MODEL S23). Test `tests/review.rs` (`#[ignore]`).

## Fala 5: `llama-server` CUDA i zastępstwo backendu
- **Pozycja `sidecar-llama-cuda`** (`data.rs`, „do potwierdzenia przez człowieka”, bez SHA-256 — wzór pozycji Vulkan/CPU):
  dwa archiwa wydania `b6710` — `llama-b6710-bin-win-cuda-12.4-x64.zip` i `cudart-llama-bin-win-cuda-12.4-x64.zip`
  (biblioteki `cudart`/cuBLAS) — rozpakowywane do jednego drzewa `sidecars/llama-cuda/`; wymagane `llama-server.exe`
  i `cudart64_12.dll` (bez niego serwer po cichu liczyłby na CPU). Wariant CUDA (12.4), rozmiary (200 + 400 MiB —
  limit pobierania 2×), układ archiwów i licencja redystrybucji bibliotek NVIDIA — do potwierdzenia.
- **Wiele archiwów na pozycję:** `unpack::extract_trees` — wszystkie pliki pozycji do jednego katalogu roboczego,
  potem atomowa podmiana; plik powtórzony w dwóch archiwach (także inną wielkością liter) odrzuca całość. Limity
  rozpakowania sidecarów: wpis ≤ 1 GiB, całość ≤ 2 GiB (cuBLAS przekracza domyślne 512 MiB); stopień kompresji
  (zip-bomb) i limit pobierania bez zmian.
- **Zastępstwo** (`app-modules::route::local::server_for`): kompilacja własna backendu → wspólna `sidecars/llama/` →
  zastępcza: CUDA → Vulkan → CPU, Vulkan → CPU → CUDA, CPU → Vulkan → CUDA (każda kompilacja llama.cpp dla Windows
  ma backend CPU). Gdy backend z profilu urządzenia nie ma własnej kompilacji — ostrzeżenie w dzienniku
  (`brak llama-server dla backendu z profilu — używam kompilacji zastępczej`, pola `profil`, `uzyty`); notatka
  pozycji CUDA w UI mówi, co się stanie bez niej.
- **Testy:** `crates/app-models/tests/cuda.rs` (pozycja w katalogu, instalacja serwera i `cudart` do jednego drzewa,
  odrzucenie powtórzonego pliku), `crates/app-modules/src/route/local.rs` (testy zastępstwa).
