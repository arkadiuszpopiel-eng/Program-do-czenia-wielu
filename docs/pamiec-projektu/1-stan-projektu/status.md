# Status Sesji — Alfa AI Program

<!-- AUTO-ŁADOWANY przez SessionStart hook i wstrzykiwany jako kontekst. -->
<!-- Claude widzi go automatycznie na początku każdej sesji. -->
<!-- Aktualizuj NA KOŃCU każdej sesji kodowania (jednym promptem: "zaktualizuj memory"). -->

## Aktualny cel

Dwa tory: (1) domknięcie MVP na Windows — etap A planu v6 (= `alfa/docs/KONTYNUACJA.md`, etapy 0–3);
(2) plan v6 prowadzony na bieżąco w `alfa/docs/plan/` (MD) i `alfa/docs/html/` (HTML, generowany).

## Stan projektu

- **Kopia robocza:** `E:\PROGRAMY W CZASIE PRAC\Alfa AI Program\alfa` (dysk przenośny)
- **Narzędzia:** `alfa-narzedzia` obok repo; przed pracą `source scripts/portable-env.sh` albo `. .\scripts\portable-env.ps1`
- **Branch:** `ccr-af4b63c6-3fyzaj`; ponad 130 commitów lokalnych od `75fee0d`, niewypchniętych
- **Pamięć projektu ma kopię w repozytorium:** `alfa/docs/pamiec-projektu/` (stan, zasady pracy, dziennik sesji; opis w `README.md`). Na końcu każdej sesji: `bash scripts/memory-sync.sh` i commit.
- **Sesja zakończona 2026-10-12 15:50 na polecenie właściciela.** Właściciel ocenił nowe okna startowe: „na chwilę obecną OK”. Plik aplikacji 0.0.49 zbudowany z `65d1e361` i skopiowany do `Alfa-uruchomiona/` (15:47, uruchomiony na profilu właściciela).
- **Wersja:** 0.0.49 (2026-10-12, commit `65d1e361`) — pełny `verify wersja` 10/10 ZIELONY o 15:45 (Rust 2680/0/34, vitest 347, przeglądarka 353, prawdziwa aplikacja 27/27, bez powtórek); `sprzet` dla 0.0.49 nie puszczony. W tej wersji naprawiony też błąd PTT (`voice-pipeline-impl/src/mic.rs::on_wake_events`). Zrzuty prawdziwej aplikacji w każdym stylu i motywie robi `e2e-app` (`inEverySkin` w `alfa.ts`); reszta zielona: vitest 347, przeglądarka 353, prawdziwa aplikacja 27/27 (nowy `e2e-app/z-terminal.spec.ts`: plakietka, prawdziwy PowerShell, okno logowania Claude Code), kopia właściciela odświeżona; `verify sprzet` dla 0.0.49 NIE puszczony. 0.0.49: nowy kreator pierwszego uruchomienia (`views/onboarding/`: `OnboardingRail`, `OptionCards`, `HardwareStep`, `StepIcon`; zrzuty `ui/scripts/shots-onboarding.mjs`), krok „Konto Claude / ChatGPT” i okno logowania z instrukcją (`TerminalDialog.svelte`), opisy dostawców (`logic/provider-meta.ts`), izolacja instancji testowej (`ScopedSecretStore` → wpisy `Alfa/test/…`, `user_root` ustawiany w powłoce, plakietka w oknie, środowisko bez `*_API_KEY`) — zmiana w powłoce i sekretach czeka na przegląd właściciela. W TOKU (cel właściciela): pokrycie poleceń rdzenia 51,3 % → 70 %; sonda gotowa w scratchpadzie sesji (`probe.mjs` — do odtworzenia), wyniki: ok. 100 poleceń da się bezpiecznie wywołać (lista w rozmowie), testy `e2e-app/z-*.spec.ts` do napisania; nie wywoływać poleceń z oknami dialogowymi i otwierających programy. Poprzednio 0.0.48 (commit `5983f26c`) — pełne sprawdzenie ZIELONE: `verify wersja` 10/10 (2679 testów Rust, vitest 340, przeglądarka 348, prawdziwa aplikacja 24/24, pokrycie poleceń 117 z 228 = 51,3 %), `verify sprzet` 5/5 (11 etapów sprzętowych, 4 obowiązki na modelu lokalnym); kopia właściciela odświeżona. 0.0.48 naprawiło wady z listy poniżej: (1) silniki poboczne zamykane przy wyjściu i sprzątane po awarii — `crates/lib-sidecar-guard` (rejestr `state/sidecars.pids`, `RunEvent::Exit` w powłoce); (3) bramka słuchania na czas próbki głosu i czytania (`app-voice/src/port.rs::read_aloud`, plik ma 399 linii — nie dokładać); (4) test idle_unload stabilny; (2) wyjaśnione: akustyka laptopa (test diagnostyczny `voice-audio-impl::capture_microphone_to_wav`); (5) wyjaśnione: Whisper to obowiązkowy zapas strażnika rozpoznawania. Pobranie Qwen3-ASR i llama-server samym programem sprawdzone (`scripts/trial/download.mjs`). OTWARTE: testy `--pulpit` (gdy właściciel nie pracuje), pokrycie poleceń ≥ 60 %, żywa mowa (właściciel), model słowa wywoławczego (brak plików), kamera. Poniżej opis stanu z 0.0.47: SPOSÓB SPRAWDZANIA (od 0.0.46, czytać `alfa/docs/plan/24-samodzielne-testowanie.md`): `node scripts/verify.mjs szybki|wersja|sprzet` → raporty w `alfa/target/verify/<wersja>/raport-*.md|json`; testy sprzętowe `scripts/verify-hw.mjs` (`--pulpit`, `--dzwiek`, `--only`); test prawdziwej aplikacji jako instancja testowa OBOK Alfy właściciela (`scripts/verify-app.sh`, port 9333, `ALFA_E2E_INSTANCE=1`); próby na modelu `scripts/trial/profile.sh start [--model|--voice]`, `duty.mjs` (ocena `grounding.mjs`: dane + forma), `voice-loop.mjs` (głośnik→mikrofon, gra dźwięk — tylko w dzień); pokrycie poleceń rdzenia `scripts/ipc-coverage.mjs` (52 %). Alfa właściciela działa z kopii `Alfa AI Program/Alfa-uruchomiona/` (`scripts/deploy-owner.sh`) — nie z `target/shell`; nigdy `taskkill /IM`. 0.0.46: forma raportów obowiązków (`REPORT_FORM` w `app-tasks/src/facts.rs`), kwarantanna pobrań a nazwy 8.3 (`platform-windows-sys-impl/src/paths_win.rs`). 0.0.47: ostatni wynik wyzwalacza (`TriggerLastRun.svelte`, `logic/trigger-runs.ts`), pasek rozmowy na żywo (`VoiceLiveStrip.svelte`), koszty z dni w arkuszu (`CostHistory`). ZNANE WADY DO NAPRAWY (pierwsze w kolejce): (1) osierocony `llama-server` po `app.exit(0)` — trzyma ok. 4 GB VRAM (naprawa: jedna grupa procesów/Job Object dla sidecarów przez `ProcessPort`; uważać na Brokera i watchdoga); (2) rozpoznanie mowy przez głośnik+mikrofon laptopa złe (1 z 8 słów) — zbadać nagraniem z mikrofonu; (3) próbka głosu z Ustawień łapana przez mikrofon; (4) niestabilny test `providers-local-impl::idle_unload_module_events_and_health`; (5) głos „niedostępny” bez Whispera mimo zainstalowanego Qwen3-ASR — wyjaśnić. Poprzednie: 0.0.45 — historia zmian 10+Starsze, arkusz faktów, kula stanu, katalog modeli. Poprzednia noc (0.0.34–0.0.44): raport `alfa/docs/plan/23-raport-nocny-2026-10-12.md`. PRÓBY NA PRAWDZIWYCH SILNIKACH (bez właściciela): `ALFA_LIVE_TALK=1 ALFA_LIVE_TALK_WAV=<wav 16 kHz> cargo test -p app-voice --test live_talk -- --nocapture` (nagranie zamiast mikrofonu; także test `self_test_runs_every_engine`), `ALFA_LIVE_MIC=1 cargo test -p app-core --test live_mic -- --nocapture` (prawdziwy mikrofon, po cichu). Na laptopie w `%LOCALAPPDATA%/Alfa`: `sidecars/{smart-turn,supertonic,onnxruntime}` i `models/turn` (dołożone tej nocy); sidecary do zbudowanej powłoki kładzie `scripts/stage-sidecars.sh`. NIESPRAWDZONE: żywa mowa do mikrofonu i echo, brzmienie głosów, pobranie Qwen3-ASR i llama-server samym programem (Smart Turn, ONNX Runtime i Supertonic sprawdzone na świeżym profilu). NASTĘPNE: P14 — krótszy krok Krytyczki na modelu lokalnym, koszty z poszczególnych dni, ekran „Obowiązki”; P1 — głosy Pipera z parametrami, głos Edge jako opcja (sieć — przegląd właściciela), kula jako nakładka; P2 — Sklep modeli, rozkład na karty, panel System, karty AMD/Intel na żywo; P3 — menu kołowe, doki; P12/P13 wymagają przeglądu właściciela (Broker, lib-netguard). Reguła budowy: co 5 dużych ulepszeń albo na polecenie.
- **Ogólny stan:** Częściowy — aplikacja działa na laptopie (okna, ustawienia, skalowanie); odpowiedź modelu
  lokalnego, głos i Broker poza procesem nadal niesprawdzone na sprzęcie; 0 ze 108 kryteriów potwierdzonych na maszynie

## Plan v6 (2026-10-09)

- Dokumenty: `alfa/docs/plan/README.md` (spis), `03-roadmapa.md` (żywy, etapy A–H), `00-stan-wykonania.md`
- HTML: `alfa/docs/html/index.html` — buduje `node scripts/docs-html.mjs` (`--check` sprawdza odnośniki; jest w `gates.ps1`)
- Podgląd w przeglądarce sesji: `preview_start name=alfa-docs` (port 5180)
- Projekty ADR 0017–0024 — status „Proponowany”, czekają na podpis właściciela; SPEC-e 9 nowych modułów
- **Zasada (w AGENTS.md):** po zadaniu — stan w `03-roadmapa.md` (kryterium w STATUS); po etapie —
  `00-stan-wykonania.md`, wyższa wersja + CHANGELOG, `node scripts/docs-html.mjs`
- Decyzje właściciela 2026-10-09: przekaźnik + E2E; zgody zdalne dla wszystkich poziomów ryzyka; telefon i panel WWW
  jako pilot 100 %, bez komputera rozmowa przez model API (model lokalny tylko na komputerze); plan v6 obok v5

## Ukończone w sesji 2026-10-07 → 09

- [x] Porządek, katalog, bramki, ADR 0016; skalowanie interfejsu; naprawy z przeglądów (wersja 0.0.2)
- [x] Porządki na dyskach (jedna kopia robocza na E:)
- [x] Style interfejsu Szkło i Next, jasny i ciemny (`ui.style`, `packages/ui-kit/src/skins.css`), testy
- [x] Plan v6 w MD i HTML, aktualizacja STATUS / KONTYNUACJA / mapy dokumentów / AGENTS.md

## Zasada po błędzie z 0.0.3
Każdą zmianę wyglądu sprawdzać w stanie OTWARTYM (listy, menu, okna) w każdym stylu × motywie i w prawdziwej aplikacji;
żadnych kontrolek systemowych. Narzędzie: `apps/desktop/ui/scripts/shots-open.mjs`; testy: `e2e/skins.spec.ts`, `e2e-app`.

## Ostatnie testy (2026-10-10, wersja 0.0.17)

```
vitest 295/295 · E2E (PW_WORKERS=4 PW_CHANNEL=msedge) 239 z 241 (znany próg strumienia + losowy test panelu głosu)
Rust: cargo test --workspace 2565 zielonych / 0 czerwonych / 33 pominięte; clippy --workspace czysty · e2e-app 15/15 na profilu tymczasowym
NIE sprawdzono: prawdziwa kamera (kamera laptopa odłączona sprzętowo — MFEnumDeviceSources = 0 urządzeń),
brzmienie głosu na słuch, import kluczy z prawdziwych zmiennych, rozmowa głosowa na żywo
E2E nie puszczać równolegle z cargo (fałszywe przekroczenia czasu, 10 min zamiast 3)
```

## Aktywne błędy

| Problem                                                 | Moduł   | Priorytet           | Status                              |
| ------------------------------------------------------- | ------- | ------------------- | ----------------------------------- |
| Odpowiedź modelu lokalnego i głos niesprawdzone na żywo | całość  | wysoki              | czeka na zgodę na pobranie ok. 6 GB |
| Build wydaniowy ma `panic = "abort"`                    | powłoka | średni              | decyzja właściciela                 |
| Analizator powłoki S-01…S-04, S-06; schowek C-02, C-03  | tools-* | wysoki (S-01, S-03) | decyzja właściciela                 |
| Testy czasowe (strumień, paleta) na granicy progu       | ui/e2e  | niski               | decyzja o progu                     |

## Klucze API (0.0.7)

- Raport przeglądu: `alfa/docs/reviews/2026-10-09-przeglad-kluczy-api.md` (19 ustaleń, naprawione; otwarte: uwierzytelniony test OpenRouter, lista modeli DeepSeek pod `/v1`, atrapa bez stanu „brak klucza”).
- Reguła wymiany: kandydat zapisywany tylko po `Ok / Restricted / NoCredit / Unsupported` (`accepts_new_key`).
- Właściciel ma sprawdzić dodanie prawdziwego klucza OpenAI w działającej aplikacji 0.0.7.
- Plan: zadania „klucze dla firm” C2.11a–C2.17 w `docs/plan/03-roadmapa.md` (tylko plan).

## Wersja 0.0.8 — co warto wiedzieć

- Właściciel pobrał silniki: whisper (CUDA), piper, silero-vad, Bielik 1.5B; model lokalny odpowiada, mikrofon słucha.
- Błąd głosu: ramki WASAPI mają znaczniki QPC (od startu systemu), potok liczył czas od własnego startu → koniec
  wypowiedzi nigdy nie był wykrywany. Zegar potoku = `platform_windows_impl::qpc_nanos`. Diagnoza z
  `%LOCALAPPDATA%\Alfa\logs\diagnostics\*.ndjson` (zdarzenia `voice.dialog.state_changed`, `router.decision`).
- Kolejki modeli: `Account.queues`, polecenia `accounts_set_queue` / `accounts_suggest_queue`, strona `api`.
- Zlecenie właściciela na dalej: porządkowanie i dopracowanie interfejsu (organizacja!).
- Dysk: `cargo test` wielu pakietów zapełnia E: — po serii testów `cargo clean --profile dev` (trwa ok. 10 min).

## Wersja 0.0.11 — co warto wiedzieć

- Okno główne ma `decorations(false)` + `shadow(true)`; przyciski okna: `ui/src/lib/api/window.ts`, `WindowControls.svelte`
  (uprawnienia `core:window:allow-minimize/toggle-maximize/close/is-maximized`). Snap Layouts po najechaniu nie działa.
- Pasek LED: `.led` w `TitleBar.svelte` (klasy `busy`, `offline`; `data-motion='off'` → stoi).
- Nawigacja Ustawień: `SettingsNav.svelte` + `SettingsNavTree.svelte`, logika `logic/settings-nav.ts`, ikony
  `settings-icons.ts`. W testach strony wybierać `button.node` albo nazwą z `exact: true` (nagłówki grup to też przyciski).
- Karty: `Profile::light_gpu()`, `providers-local-impl/src/devices.rs` (`--device`), `route/local.rs` (`local-light`),
  Router `set_light_local` / `LIGHT_CLASSES`. Atrapa UI ma RTX 4050 + Intel Iris Xe.
- Zrzuty belki/nawigacji/kart: `apps/desktop/ui/scripts/shots-chrome.mjs`.
- Właściciel zaczął pisać kolejne zlecenie: system kamer (IP, Wi-Fi, USB, kilka naraz, rozpoznawanie użytkownika) —
  do dopisania do planu prac, gdy zlecenie przyjdzie w całości.

## Wersja 0.0.14 — co warto wiedzieć

- Nie wstawiać bloku `<style>` do `index.html`: Tauri dodaje wtedy do CSP znacznik jednorazowy i style nadawane
  elementom (`style:` w Svelte) przestają działać. Wyłapuje to tylko `e2e-app` (test CSP).
- Głos: wszystkie agentki to nadal jeden głos Pipera („Gosia”) przestrajany na cztery. Właściciel uważa głos za
  sztywny — po ocenie 0.0.14 na słuch kolejny krok to inny silnik mowy (pobranie albo chmura; decyzja właściciela).
- Podgląd stanów awatarów w atrapie: `?cast=speaking,working,waiting_approval,idle`, `?avatars=letters`.
- Kamery (C3) są w planie, kod nie istnieje. C3.1–C3.7 (kamery, „opisz, co widzisz”) mogą ruszyć od razu;
  C3.8–C3.13 (osoby, biometria) czekają na podpis ADR 0025. Technologie w rozdziale 10 dokumentu 11 są
  niesprawdzone w źródłach — przegląd jest pierwszym krokiem zadań C3.5, C3.6, C3.8.

## Wersje 0.0.16–0.0.17 — co warto wiedzieć

- Rano właściciel ma: `alfa/docs/plan/13-podsumowanie-i-dalsze-kroki.md` (stan zgłoszeń, co sprawdzić w 15 minut,
  decyzje, plan sesji) i `alfa/docs/reviews/2026-10-10-pakiet-decyzji-wlasciciela.md` (44 decyzje).
- Kamery: `camera-hub-*`, `platform-windows-impl/src/camera/` (Media Foundation, NIGDY nie działało na sprzęcie),
  polecenia `cameras_*`, `attachments_add_camera`, zdarzenie `CamerasChanged`; UI: `CamerasPage`, `CameraIndicator`,
  `CameraSnapButton`, `logic/camera-look.ts`. Otwarte z przeglądu: K-04, K-08, K-09 (wymagają kamery).
- Nie robiłem bez właściciela: głos z chmury (ADR 0026), narzędzie `camera.look` dla agentek (zdolność Brokera),
  biometria (ADR 0025), kamery sieciowe, pobieranie czegokolwiek.
- Dysk: stare pliki kompilacji usuwać przez `find target/debug/deps -maxdepth 1 -type f -mmin +180 -delete`
  (i to samo dla `target/shell/debug/deps` oraz katalogów w `target/shell/debug/incremental`) — szybciej niż
  `cargo clean` i nie kasuje bieżących artefaktów.
- Nowa strona ustawień / polecenie: dopisać też wiersz w `ui/src/lib/api/COMMANDS.md` — z niego `build.rs` powłoki
  generuje uprawnienia `allow-*`.

## Przerwane 2026-10-10 07:00 (prośba właściciela: koniec na dziś)

- B.2 (numery sekwencji zdarzeń) i B.3 (szczegóły błędów) — ledwo rozpoczęte; początek B.3 leży w `git stash`
  w repo `alfa` (`git stash list`). Nic z tego nie jest w działającym programie.

## Następne kroki

1. Właściciel: zgoda na pakiet ok. 6 GB; `gh auth login`; ustalenia bezpieczeństwa; podpis ADR 0017–0024;
   ADR 0024 (licencja silnika mowy); progi kryteriów `ACC-V6-…`.
2. Sesja: etap A.4–A.6 (pierwsza rozmowa i głos na laptopie, pomiary) zaraz po zgodzie na pobranie.
3. Sesja (bez czekania): etap B — B.1 generator typów TS, B.2 numery sekwencji zdarzeń, B.3 błędy strukturalne,
   B.5, B.6, B.9; C2.1 zestaw ewaluacyjny; E.1 układ na wąski ekran.
4. E2E na tym laptopie: `PW_WORKERS=4 PW_CHANNEL=msedge`.

## Info o sesji

- Czas: 2026-10-07 23:31 → 2026-10-09
- Model: Opus 5.5
- Maszyna: laptop i7-13620H, RTX 4050 6 GB, 16 GB RAM
