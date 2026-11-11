# models — SPEC (v1, menedżer modeli i silników, `app-models`)

## Nowy profil główny — polecenie właściciela10.11.2026

Qwen3-8B Q4_K_M jest pierwszym LLM katalogu. Pakiety dla osobnej karty12GB zastępują
mały/historyczny LLM modelem głównym8B i pokazują zgodne opisy pamięci/jakości.
Bielik1.5B jest pomocniczy; karty8B/11B i licencji pozostają jawnie niepotwierdzone przez
człowieka. Poprzednie porażki jakości/CPU pozostają zachowane, GPU wymaga odbioru.


## Cel
`catalog::builtin()` wybiera katalog hosta, a `catalog::builtin_for(Option<Target>)` pozwala jawnie
sprawdzić dane innej platformy. Testy pakietów i fixture'ów Windows wybierają Windows x64 także
na Linuxie; nie uruchamiają tych binarek. Metadane archiwów Windows zachowują nazwy `.exe`
niezależnie od hosta. Brak znanego celu udostępnia wyłącznie pozycje przenośne.


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
- HTTP5xx/429: przed ponowieniem przerwy250/500/1000ms, anulowalne dotychczasowym tokenem.
  Nadal najwyżej cztery żądania i te same timeouty15/30s. Zerwany strumień/Range wznawiany jak wcześniej.
  Trwała awaria kończy się jawnym błędem; odpowiedź błędu nie nadpisuje pliku częściowego.
- Plik bez przypiętego hasha → `needs_trust`: UI pokazuje policzony SHA-256 i licencję; instalacja wyłącznie po
  `models_trust_hash` z hashami równymi policzonym, a przed instalacją pliki są hashowane ponownie (podmiana po
  pobraniu → usunięcie). Rekord instalacji zapamiętuje `trusted: true`.
- Rozpakowanie ZIP: `updater_contract::validate_package_path` (bez `..`, `\`, `:`, nazw urządzeń, kropki/spacji
  na końcu), bez dowiązań, bez duplikatów (wielkość liter), `PackageLimits` (wpisy, rozmiar wpisu i całości,
  stopień kompresji), rzeczywiste bajty ≤ zadeklarowane; drzewo do `.staging-<katalog>` i `rename`; błąd nie rusza
  poprzedniej instalacji. Wpis wybrany z paczki (PyPI) sprawdzany przypiętym SHA-256.
- Limit równoległych pobrań (domyślnie 2), anulowanie czeka na koniec zadania (bez dwóch piszących do `.part`).
- Aktywnego embeddera nie można usunąć; zmiana embeddera = `SqliteSearch::set_embedder` (migawka na operację)
  - przebudowa wektorów w tle (bazy sesji + bazy zakresów pamięci); w trakcie wyszukiwanie wektorowe działa jak FTS.

## Zależności

`app-api` (DTO, `EventHub`, `AppPaths`), `search-impl` (`set_embedder`, `spawn_reindex`), `memory-impl`
(`ScopeDbs`, `index_label`), `lib-embed` (katalog, `OnnxEmbedder`, manifest), `providers-local-impl` (manifest
GGUF, rekord `.sha256`), `updater-contract` (reguły ścieżek, limity), `model-residency-contract`, `core-config-contract`.
Zewnętrzne (już w `Cargo.lock`): `reqwest` (rustls), `zip`, `flate2`, `sha2`, `tokio-util`.

## Konfiguracja

`[search.embedder] model = "multilingual-e5-small" | "lexical"` (warstwa wspólna; brak = model domyślny, używany,
gdy jest zainstalowany). Katalogi: `%LOCALAPPDATA%\Alfa\{models,sidecars,downloads,state\models}`. Opcje kompozycji
(`ModelsOptions`): `parallel = 2` (ustawienie `models.parallel_downloads` 1–4, nakładka maszyny, od następnego
uruchomienia), wybór embeddera 20 s po uruchomieniu; pełny przebieg przebudowy na starcie tylko,
gdy ostatni bezbłędny był dla innego embeddera albo ponad 7 dni temu (`state\models\reindex.json`: identyfikator
embeddera i czas — przebieg otwiera wszystkie bazy sesji, które zostają w pamięci podręcznej `sessions`).

## Katalog — stan danych (2026-10-04)

Potwierdzone (PyPI, hash archiwum z API i hash wpisu policzony lokalnie): Silero VAD 6.2.3 (`silero_vad_op18_ifless.onnx`
= `voice_vad_impl::KNOWN_MODELS`), openWakeWord 0.5.1 (`melspectrogram.onnx`, `embedding_model.onnx`).
**Do potwierdzenia przez człowieka** (HF/GitHub zablokowane w sesji, SHA-256 nieprzypięte): Bielik GGUF, e5-small
i MiniLM (ONNX + tokenizer), whisper `large-v3-turbo-q5_0` / `small-q5_1`, Piper `pl_PL-gosia-medium` (licencja
głosu), WeSpeaker ResNet34 (warunki VoxCeleb; okno 200 ramek), `llama-server` (wydanie `b6710` — numer do
przypięcia, układ ZIP), `whisper-server` 1.8.1 (`Release/`), `piper` 2023.11.14-2. Pocket TTS — instalacja ręczna.

**System / architektura (B.6, 2026-10-29):** `ItemSpec::targets` — lista par system/procesor
(`catalog::target::Target`), pod które zbudowano pliki pozycji. Binarki silników (`llama-server` Vulkan/CUDA/CPU,
`whisper-server` CPU/CUDA, `piper`, ONNX Runtime CPU i DirectML, DirectML) mają `Windows x64`; modele, wagi
i pozycje ręczne mają listę pustą (każdy system). `builtin()` pomija pozycje, które nie działają na tym komputerze
(nieznany system — zostają tylko pozycje bez celu), a pakiety pomijają brakujące identyfikatory. Test pilnuje,
że każda pobierana binarka ma cel. Nowy system albo ARM64 = nowe wiersze z własnym archiwum i sumą.

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
- Build Vulkan `whisper-server` — brak w wydaniach whisper.cpp (desktop AMD: Whisper na CPU, od 0.0.63 głównym
  silnikiem jest tam Qwen3-ASR przez `llama-server` Vulkan — ADR 0028); CUDA — od fali 6.
- Podpisy Authenticode binariów sidecarów (P-08) — dziś tylko SHA-256/TOFU.
- Przebudowa (`search-impl::reindex_all`) otwiera naraz wszystkie bazy sesji przez `SessionDbProvider` (pamięć
  podręczna `sessions` ich nie zamyka) — przy setkach sesji RAM rośnie; do rozważenia zamykanie baz po kroku.
- `models_local_download` (`providers-local`) zostaje dla zgodności, ale zapisuje hash bez zgody w UI — do decyzji,
  czy wyłączyć (onboarding korzysta już z menedżera).

## Przegląd bezpieczeństwa #3 (2026-10, `docs/reviews/2026-10-security-review-3.md`) — decyzja człowieka

- **P3-04:** zrealizowane 2026-10-21 (0.0.68) — archiwa sidecarów (`llama-server`, `whisper-server`, `piper`) i wszystkie modele mają przypięte SHA-256; test `tests/review.rs` bez `#[ignore]`; `confirmed` (potwierdzenie przez człowieka) bez zmian.

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

## Fala 6: próba generalna przed testem na laptopie (2026-10-06)

- **Układ archiwów z GitHuba sprawdzony** (pobranie / lista wpisów przez HTTP Range, `sha256sum`): `llama-server` b6710
  cpu/vulkan/cuda — pliki w korzeniu (`llama-server.exe`, `ggml-*.dll`, `LICENSE-*`), `cudart-llama-bin-…` — 3 biblioteki
  w korzeniu (bez powtórzeń z archiwum serwera), `whisper-bin-x64.zip` — `Release/` (21 plików, w tym
  `whisper-server.exe`), `piper_windows_amd64.zip` — `piper/` (363 wpisy, katalogi z `/`). Flagi CLI używane przez Alfę
  są w binariach (`--api-key`, `--alias`, `--jinja`, `-np`, `-ngl`; `-nlp`, `-sns`, `-ng`, `-fa`; `--output_raw`,
  `--quiet`). Sumy i rozmiary: `docs/user-guide/11-pierwszy-test-na-pc.md` („Jutro na laptopie”) — do przypięcia przez
  człowieka (bramka P3-04 — zamknięta w 0.0.68); `confirmed` bez zmian.
- **Pozycja `sidecar-whisper-cuda`**: `whisper-cublas-12.4.0-bin-x64.zip` (430 MiB; `Release/` z `cudart`/cuBLAS,
  ~0,7 GB po rozpakowaniu) → `sidecars/whisper-cuda/`, wymagane `whisper-server.exe` i `cudart64_12.dll`. Zainstalowana
  ma pierwszeństwo przed CPU (`app_modules::stt`), nieudany start → CPU (`voice-stt-impl`).
- **Rozpakowanie odporne na inne wydanie:** separator `\` (Compress-Archive w PowerShell 5.1) normalizowany do `/`
  **przed** walidacją ścieżki; `strip` z katalogu nieobecny w archiwum → rozpakowanie od korzenia (ostrzeżenie w
  dzienniku); brak wymaganego pliku → błąd z listą wpisów najwyższego poziomu (widać go w UI). Poza Windows bit
  wykonywania z archiwum (0o755). Testy: `tests/layout.rs` (pozycje z `builtin()` z archiwami o układzie wydań).
- **Serwer pobrany po starcie działa bez restartu:** `LocalConfig::server_candidates` (providers-local) sprawdzane
  przy każdym starcie sidecara; wcześniej ścieżka `llama-server` była ustalana raz przy starcie aplikacji i pierwsza
  rozmowa po pobraniu serwera w Ustawieniach kończyła się błędem do ponownego uruchomienia.
- **Próba na żywo** (`tests/live_catalog.rs`, `#[ignore]`, `ALFA_LIVE_CATALOG=1`; job „Silniki na żywo (CPU)” w
  `.github/workflows/rehearsal.yml`): prawdziwy `ModelsApp` pobiera i rozpakowuje pozycje CPU + najmniejsze modele
  (oraz CUDA/Vulkan tylko do sprawdzenia układu), uruchamia je kompozycją aplikacji (`app_modules::route::local`,
  `tts::engines`, `stt::whisper`), generacja ≥ 16 tokenów po polsku, Piper → WAV → whisper (WER ≤ 0,5). Raport JSON
  (artefakt `alfa-live-report`): adres, rozmiar, SHA-256, surowy układ archiwów, wersje i flagi CLI, czasy (tok/s,
  RTF). `ALFA_LIVE_MIRROR=http://127.0.0.1:<port>` — próba na sucho z lokalnego lustra (atrapy silników).
- **Modele rozmowy z oficjalnych repozytoriów** (manifest `providers-local`, 2026-10-06): Bielik 4.5B v3.0 Instruct
  **Q8_0** (4826 MiB, pierwsza pozycja LLM — tę pobiera wprowadzenie) i Bielik 1.5B v3.0 Instruct Q8_0 (1620 MiB, bez
  narzędzi). Wpis Q4_K_M wskazywał plik nieobecny w `speakleash/Bielik-4.5B-v3.0-Instruct-GGUF` (HTTP 404). Próba na
  żywo pobiera teraz najmniejszy — 1.5B (żądanie z narzędziem pomijane: `tools = false`).

## Pakiety 1–6 i „Napraw” (2026-10-07)

- **Skala ocen 6 → 1** (`bundle_data.rs`, stan 0.0.63): 6 Wzorcowy (Bielik 4.5B, Qwen3-ASR, Whisper turbo i small,
  Supertonic 3, Piper, Smart Turn, ONNX Runtime, Silero VAD, openWakeWord, WeSpeaker, e5; karta ≥ 8 GB), 5 Bardzo
  dobry (jak 6 bez Qwen3-ASR, słowa wywoławczego i mówcy; karta ≥ 6 GB), 4 Dobry (Bielik 1.5B + turbo i small +
  Supertonic 3 + Piper + Smart Turn + ONNX Runtime + VAD + e5; RAM ≥ 12 GB i karta ≥ 4 GB albo procesor ≥ 8 rdzeni),
  3 Zrównoważony (1.5B + small + Piper + Smart Turn + ONNX Runtime + VAD), 2 Lekki (1.5B + small + VAD), 1 Minimalny
  (1.5B). Pozycje zależne od karty dokłada `members_for` (niżej). Progi w MB z tolerancją raportowania systemu (16 GB RAM →
  ≥ 15 000 MB, karta 8 GB → ≥ 7 500 MB); próg „na styk” z opisem kompromisu (np. 4.5B częściowo na karcie 6 GB).
- **Dobór silników do maszyny** (`bundle_machine.rs`, `Machine::from_profile` — główna karta wg
  `Profile::primary_gpu`: karta z własną pulą pamięci, osobna przed zintegrowaną, ≥ 3 500 MB; RAM bez rezerwy APU
  liczonej jako pamięć karty — `Profile::system_ram_mb`): NVIDIA → `llama-server` i `whisper-server` CUDA + CPU
  w zapasie; AMD/Intel → `llama-server` Vulkan + CPU w zapasie, `whisper-server` CPU; sama karta zintegrowana z Vulkanem
  przy ≥ 16 GB RAM → jak AMD/Intel (lekki model na karcie); laptop NVIDIA z kartą zintegrowaną → dodatkowo
  `llama-vulkan` w zapasie (lekki model); bez karty → CPU. Pozycje zależne od karty (`members_for`): na Vulkanie
  Qwen3-ASR w pakietach z Whisperem turbo (na karcie liczy od 12 GB — `stt::qwen_free`; na mniejszej na procesorze),
  przy karcie z własną pamięcią ≥ 12 GB i głosie Supertonic — `onnxruntime-directml` i `directml` (ADR 0028).
  Dopasowanie `fits` / `tight` / `too_weak` z uzasadnieniem (PL/EN), zalecany = najwyższy `fits`.
- **Stan pakietu** ze stanów pozycji (pierwszeństwo: pobieranie → uszkodzony → zgoda TOFU → zainstalowany → częściowo);
  `missing_bytes` — górne oszacowanie (bez pobranej części bieżącego pliku).
- **Działania** (`bundle_ops.rs`, z istniejących prymitywów): `models_bundle_download` — brakujące, wstrzymane
  i nieudane do kolejki, uszkodzone naprawia; ręcznych, trwających i czekających na zgodę nie rusza; pozycja, która
  nie ruszyła, dostaje błąd w swoim stanie, reszta idzie dalej. `models_bundle_verify` — ponowne SHA-256.
  `models_repair` — usunięcie plików, częściowych pobrań i rekordu, pobranie od nowa (odmowa: pozycja ręczna,
  aktywny embedder, instalacja w toku).
- **Uwagi o jakości** (`bundle_quality.rs`): normy wyłącznie jako zalecenie albo metoda pomiaru, bez deklaracji
  certyfikacji i bez wymyślonych wyników — ISO/IEC 25010:2023 (wydajność, niezawodność), ISO/IEC 25059:2023 (jakość
  systemów AI), WER (metodyka NIST SCLITE, próg ACCEPTANCE F2-03), ITU-T P.800/P.808 (MOS), ITU-T G.114 (opóźnienie —
  telefoniczne, Alfa ma własne cele PLAN §6.4), ISO/IEC 19795-1:2021 (FAR/FRR/EER). Liczby tylko z pomiarów repozytorium
  (próba generalna 2026-10-06) i planu rozmieszczenia `providers-local`.
- **Atrapa UI:** `apps/desktop/ui/src/lib/api/fake/bundles.json` generuje `tests/bundles_ui.rs`
  (`ALFA_UPDATE_FIXTURES=1`) dla maszyny atrapy (Radeon 780M 4 GB, 32 GB, 8 rdzeni: 6 za słaby, 5 na styk, 4 zalecany);
  stan liczy atrapa ze swoich pozycji. Testy: `tests/bundles.rs` (maszyny referencyjne, warianty silników, stan,
  normy), `tests/bundle_ops.rs` (lokalny serwer HTTP: pobranie, weryfikacja, naprawa, TOFU, pozycje ręczne).

## Karty licencji (C5.7, 0.0.64)

- **Dane:** `crates/app-models/data/license-cards.json` (identyfikator pozycji → karta), ładowane przez
  `licenses.rs` i dołączane do `ModelItem::license_card` (pole z `serde(default)` — starsze ładunki bez niego
  wczytują się jako `null`). Karta: identyfikator licencji (SPDX albo nazwa umowna), pełna nazwa, źródło licencji (adres tekstu albo strony autora),
  autor, użycie komercyjne (`yes` / `no` / `conditional` / `unknown`), wymóg wskazania autora, uwagi PL/EN,
  `confirmed` (licencja ma zapis sprawdzenia w dokumentacji projektu — odnośnik „Zapis licencji:” w uwagach) i głosy pozycji (`VoiceLicense`: barwy Supertonic 3 F1–F5 i M1–M5, Gosia z Pipera, mówczynie
  Pocket TTS). To zapis faktów do pokazania, nie opinia prawna.
- **Niezmienniki (testy `licenses_tests.rs`):** każda pozycja katalogu ma kartę i żadna karta nie wisi bez pozycji;
  karta potwierdzona tylko przy potwierdzonej pozycji i z odnośnikiem „Zapis licencji:” / „Licence record:”; karta niepotwierdzona ma w uwagach „Do potwierdzenia przez
  człowieka” (EN: „To be confirmed by a human”); użycie komercyjne `unknown` nigdy potwierdzone; każdy głos
  z `voice-tts-contract` (`SUPERTONIC_ALL`, `PIPER_BASE`, `POCKET_BASE`) i każda pobierana barwa Supertonic mają
  wpis; karty pozycji mówiących opisują znak wodny mowy syntetycznej jako decyzję właściciela w toku.
- **Sprawy otwarte zapisane w kartach:** DirectML (dane diagnostyczne, §2a), biblioteki NVIDIA CUDA, espeak-ng
  (GPL-3.0) w archiwum Pipera, licencja nagrań głosu Gosia, modele cech openWakeWord (CC BY-NC-SA według README),
  VoxCeleb (WeSpeaker), pochodzenie „Pocket TTS PL” (C5.1), model bazowy Bielika 4.5B (Qwen2.5-3B), build ffmpeg,
  zakaz porad medycznych w OpenRAIL-M (Supertonic 3) — decyzje człowieka.
- **UI:** rozwijana „Karta licencji” w wierszu pozycji (`LicenseCardInfo.svelte`, przycisk z `aria-expanded`),
  atrapa `fake/license-seeds.ts`, e2e `e2e/license-cards.spec.ts`.

## PróbaCPU katalogu modeli o różnych wymaganiach

Próba zachowuje budżetCPU8192MiB z rekomendacji runnera. Każdy model jest sprawdzany;
przekraczający budżet musi zwrócić dokładną odmowę pamięci przed startem (osobny test ochrony).
Jego generacja pozostajeNOT_RUN_CPU_BUDGET, błąd zostaje w raporcie. Nieoczekiwane błędy
nie są ukrywane. Wspierane modele nadal muszą wygenerować≥16 tokenów z polskimi literami,
a cały przebieg wymaga co najmniej jednej rzeczywistej generacji. To nie jest odbiórGPU.
