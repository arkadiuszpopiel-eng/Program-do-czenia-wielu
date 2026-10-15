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
- **0.0.53 (noc 14/15.10, praca do 5:00 na polecenie właściciela):** dokument 25 rozstrzygnięty: izolacja poleceń powłoki integralnością Low (nie AppContainer — zepsułby git/cargo/npm), macierz dowodów = `docs/DOWODY.md`, Antigravity zatwierdzony, Markdown w wątkach (7,8 ms). wasmtime 36.0.18. Pokrycie łącza 71,8 %, bramka 70 %. O 10:00 test konta Google z właścicielem.
- **0.0.52 (2026-10-14 wieczór):** Ustawienia → Stan połączeń (`ConnectionsPage.svelte`, `logic/connections.ts`), `bridges_check` (`app-bridges/src/login.rs`, `CliProbe::run_output`), sprawdzenie po „Gotowe” (`TerminalDialog.svelte`), Antigravity: `BridgeKind::Antigravity`, `agent-backends-impl/src/agy/`, profil `agy_login`, ostrzeżenie na karcie, blokady `.agy`/`.gemini`; wpis rejestru czeka na podpis (dokument 25 §3). Zawieszenie próby obowiązków = obciążenie równoległym testem, nie kod.
- **Sesja 2026-10-14 (do 18:20).** Wersja **0.0.51** (commity od `99367ee7` do `8006e86a` i dalej) wgrana do `Alfa-uruchomiona/` o 18:17 i uruchomiona na profilu właściciela.
- **Wersja:** 0.0.51 — pełny `verify wersja` (NOWY zestaw = bramki CI, 18 etapów) 17/18 zielonych przy biegu o 18:05; czerwony `zaleznosci` = nowe RUSTSEC-2026-0346/0347 w rustls 0.23.45 → podniesione do 0.23.46 (oba Cargo.lock), `cargo deny` czysty, testy TLS 136/0, ponowna budowa i prawdziwa aplikacja 28/28 zielone. Liczby: Rust 2732/0/34, vitest 356, przeglądarka 357, prawdziwa aplikacja 28, pokrycie łącza 122/233 (52,4 %). `verify sprzet` dla 0.0.49–0.0.51 NIE puszczony.
- **0.0.50:** test klucza OpenRoutera (`GET /key`), wspólna kolejka modeli wszystkich kont (zakładka „Wszystkie konta”, `model-queues.json`).
- **0.0.51 — dobór modelu według trudności (zgłoszenie właściciela):** ocena trudności `providers-contract/src/difficulty.rs`, wskazówka trasy `RouteHint` w `RequestMeta::route`, szczeble `router-contract/src/rung.rs`, kolejność w `router-impl/src/difficulty.rs`; Krytyczka na szczeblu sprawdzania, poprawka po odrzuceniu od modelu mocniejszego (`agent-runtime-impl` critic.rs/turn.rs), klasa roli przy delegacji; ustawienia `model-routing.json` (app-modules `route/routing_settings.rs`), polecenia `routing_get/set/preview`, strona Ustawienia → Dobór modelu (`RoutingPage.svelte`, `RoutingPreview.svelte`). DECYZJA: ręczna kolejka (konta/wspólna) ma pierwszeństwo; przełącznik `reorder_queues`.
- **Audyt zewnętrzny 2026-10-14** (`../audyt-2026-10-14/RAPORT.md`, 17 pozycji): naprawione w 0.0.51 A-01…A-14, A-16, A-17; A-15 częściowo. Zestawienie i propozycje: `alfa/docs/plan/25-do-przegladu-po-audycie.md`. CZEKA NA WŁAŚCICIELA: izolacja procesu poleceń powłoki (AppContainer — A-07 część 2), jedna macierz dowodów (A-15). Nowe bramki: `scripts/check-size.mjs` (w `pnpm lint`, wyjątki `scripts/size-exceptions.json`), `verify.mjs` = bramki CI + wynik NIEKOMPLETNY, `set-version.mjs` pilnuje obu Cargo.lock, generator tokenów formatuje prettierem.
- **Dysk E: rozłączał się 2026-10-14 rano** (Ntfs 140/50, opóźniony zapis $Mft) przy dużym obciążeniu — commitować często; kopia zmian na C: w scratchpadzie sesji.
- **W TOKU / NASTĘPNE:** pokrycie poleceń 52,4 % → 70 % (cel właściciela), `verify sprzet`, testy `--pulpit`, decyzje z dokumentu 25.
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
