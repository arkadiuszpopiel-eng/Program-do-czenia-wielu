# Status Sesji — Alfa AI Program

<!-- AUTO-ŁADOWANY przez SessionStart hook i wstrzykiwany jako kontekst. -->
<!-- Claude widzi go automatycznie na początku każdej sesji. -->
<!-- Aktualizuj NA KOŃCU każdej sesji kodowania (jednym promptem: "zaktualizuj memory"). -->

## Aktualny cel

**Bieżący (2026-10-21, 15:20):** w Alfa-uruchomiona działa 0.0.66 (L4 zatwierdzone). 0.0.67 zatwierdzona lokalnie (`0e54f5b9`): skrót „Alfa”, odświeżanie L4 po zgodzie, łatki bibliotek, poprawka bezpieczeństwa strażnika autonomii po przeglądzie — wgranie czeka na decyzję właściciela. Właściciel poza domem kilka godzin (/goal: poprawki, ulepszenia, nowe wersje). Lista zadań: `handoff.json` → `next_session`.

Tory stałe: (1) domknięcie MVP na Windows — etap A planu v6 (= `alfa/docs/KONTYNUACJA.md`, etapy 0–3);
(2) plan v6 prowadzony na bieżąco w `alfa/docs/plan/` (MD) i `alfa/docs/html/` (HTML, generowany).

## Stan projektu

- **2026-10-21 (PC stacjonarny):** właściciel uruchomił ręcznie build testowy `alfa\target\shell\debug\alfa-desktop.exe` (bez programów Jądra obok → Broker w procesie, „Zmień na L4” odrzucane) — na tym PC nie było skrótu do Alfy. Naprawa `4a5662b7`: skrót **„Alfa”** na pulpicie i w menu Start → `Alfa-uruchomiona\alfa-desktop.exe` (`scripts/owner-shortcut.ps1`, odświeżany przez `deploy-owner.sh`), tekst w buildzie testowym mówi, że to build testowy i co uruchomić. Sprawdzone: Alfa ze skrótu = tryb przenośny (alfa-broker, okno zgód alfa-broker-ui, watchdog z Ctrl+Shift+F12). verify-app na `dc0590d2`: 47 + 1 pominięty, pokrycie 259/273 (94,9 %). Usunięte stare kompilacje (target/debug/deps, shell, trial-profile, release, src-tauri/target): 229 GB, E: 268 GB wolne. **Właścicielowi podawać tylko skrót „Alfa”, nigdy ścieżki do target/shell.**
- **2026-10-21 popołudnie:** zgłoszenie „zatwierdziłem L4 w oknie, a zmiany nie ma” — Broker miał L4 (odczyt przez CDP), rdzeń nie przyjmował podniesienia bez wcześniejszego obniżenia i nie ogłaszał zmiany. Naprawa `7889e282` + `015a1a98`: strażnik autonomii przyjmuje zatwierdzone podniesienie (globalne i sesji) i wysyła SessionUpdated; strona Uprawnień odpytuje poziom co 2 s (do 10 min) po otwarciu okna Brokera; poziomy/podniesienia w `app-broker::levels` (test regresji red→green). Wersja **0.0.67** `376b21d0`; łatki `39965c0c`: rustls-webpki 0.103.16, wasmtime 36.0.19, vite 7.3.7 (zip zostaje — wersja główna).
- **2026-10-21 po południu (przeglądy):** przegląd 0.0.67 (16 recenzentów) — luka strażnika: zgodność poziomu brana za zgodę, nowy Broker startuje z L3 → prośba o L3 bez zgody + restart Alfy w 15 min nadpisywały obniżenie. Naprawa `0e54f5b9`: `BrokerPort::approval_outcome` (stan prośby z Brokera, przez IPC), prośby tylko w pamięci (`app_broker::levels::PendingRaises`, Notify), drugi przegląd (9 agentów) — 5 potwierdzonych, poprawione. Też: test obowiązków (5), sidecary/wgranie, `memory-sync` wstrzymuje starą pamięć PC (przywrócone zasady dwóch maszyn), `verify` bez Edge → Chromium. Rust 3079/0, zmienione crate'y 240/0, vitest 478.
- **Kopia robocza:** `E:\PROGRAMY W CZASIE PRAC\Alfa AI Program\alfa` (dysk przenośny)
- **Narzędzia:** `alfa-narzedzia` obok repo; przed pracą `source scripts/portable-env.sh` albo `. .\scripts\portable-env.ps1`
- **Branch:** `ccr-af4b63c6-3fyzaj`; 317 commitów lokalnych ponad `origin` przy `c8791fdd` (18.10; liczba rośnie —
  aktualna: `git rev-list --count origin/HEAD..HEAD`), niewypchnięte
- **Maszyny (praca na zmianę, dysk E: przenoszony):** laptop i7-13620H, 16 GB, RTX 4050 6 GB + Intel UHD Graphics
  (bieżąca od 18.10 po południu; Edge, `jq`, `gh`, Claude Code, dane Alfy; cargo `-j 3`) i komputer stacjonarny
  Ryzen 7 5700X3D, RX 9070 XT 16 GB, 32 GB (17–18.10, seria 0.0.63; bez Edge, `jq`, Claude Code i danych Alfy;
  `CARGO_BUILD_JOBS=6`). Opis: `alfa/docs/plan/00-stan-wykonania.md`, pamięć `maszyna-stacjonarna-2026-10-17`.
- **2026-10-20 po południu — WERSJA 0.0.66:** po zrzutach właściciela („Zmień na L4” odrzucane, Alfa „nie mam dostępu”): deploy-owner buduje app-safety do target/safety i kopiuje Broker/Broker-UI/watchdog (tryb przenośny), domyślny katalog roboczy rozmowy (app-store workdir_choice), Dyrygentka z informacją o Delcie w prompcie (zwykły czat bez zmian — wariant „wszystko przez przebieg agentki” wycofany po 14 testach), RunOutcome::Failed z ProviderError (429/offline w przebiegach). Wgrana 17:45, start bez błędów. Raport `docs/plan/32-raport-2026-10-20-0066.md`.
- **2026-10-20 — WERSJA 0.0.65 (seria 6, laptop):** kanwa artefaktów, Arena, karty rozmów, „Naucz Alfę”, edycja kont firm, karty w mowie; naprawa [poważne] regresji obowiązków (delegate_task w zadaniach z arkuszem faktów + walidacja schematem narzędzia) — próba na Bieliku 4/4 zielone. verify: Rust 3063/0, UI 473, przeglądarka 547/547, aplikacja 48/48, pokrycie 95,6 %; czerwony tylko cargo deny (zip). Budżet startowy 149,6/150 KB (leniwe karty, okienka stanu/kosztu, baner aktualizacji, błąd tury). Przełącznik baneru trybu deweloperskiego (ui.broker_dev_banner). Decyzja właściciela: poziom „Pełny dostęp” w serii 7. Usunięto target/agents (62 GB), wt-s6/*, gałęzie s5-*/s6-*. Raport `docs/plan/31-raport-2026-10-20.md`. Kopia F:\Alfa — patrz raport §6.
- **2026-10-19 rano — KONIEC SESJI NA LAPTOPIE, dalej PC:** opis przekazania `PRZEKAZANIE-NA-PC-2026-10-19.md` (obok repo) i `alfa/docs/plan/30-przekazanie-na-pc-2026-10-19.md`. `verify sprzet` 0.0.64 przerwany na polecenie właściciela (bez wyniku).
- **2026-10-18/19 — WERSJA 0.0.64 (seria 5, laptop):** 8 punktów przez agentów w kopiach roboczych `wt-s5/<punkt>` (gałęzie s5-*, scalone), + sprawdzenie laptopa iGPU + NVIDIA (bez regresji) i test Claude Code. Końcowy `verify wersja` na `728558d2`: 18/19 (Rust 2983/0, UI 406, przeglądarka 475, aplikacja 42/42, pokrycie 88,6 %); czerwony tylko `cargo deny` — `zip` 8.6.0 w `updater-impl` → gałąź `do-przegladu-updater-zip9` (nie scalać bez przeglądu). Azure OpenAI w `providers-catalog/czeka-na-wlasciciela/` (rejestr zgodności). Zbudowana 0.0.64 w `target/shell/debug`, wgrana do Alfa-uruchomiona 19.10 5:13. Raport `docs/plan/29-raport-2026-10-18.md`. Kopia na F:\Alfa (nowy pusty dysk „Dane”).
- **2026-10-18 po południu — POWRÓT NA LAPTOP (i7-13620H, 16 GB, RTX 4050 6 GB + Intel UHD Graphics):** przegląd stanu po PC stacjonarnym (25 commitów 0.0.63, ostatni `c096d02e`, drzewo czyste, 316 commitów ponad origin). Wgrana u właściciela nadal **0.0.62**; kandydat 0.0.63 = `target/shell/debug/alfa-desktop.exe` z `21897bf0` (każda budowa na laptopie go nadpisze). Pamięć zasad laptopa uzupełniona z repo (2 notatki z PC + nowsze MEMORY.md i alfa-pamiec-w-repozytorium.md) i opisana z perspektywy dwóch maszyn (laptop `-j 3`). F: na laptopie = inny dysk „Dane” 1,9 TB pełny plików CAPACITYTESTER — NIE uruchamiać `backup.sh` z domyślnym `/f/Alfa`. Spowolnienie sesji: wtyczka „remember” (post-tool-hook liczy linie zapisu sesji ~350 MB przy każdym poleceniu — dziesiątki procesów po 500 s CPU); właściciel ją wyłączył, zawieszone procesy zatrzymane. **CEL właściciela (/goal):** wszystko porządnie do wersji **0.0.65** (0.0.64 i 0.0.65 = serie po 10 punktów), więcej agentów; kopia bezpieczeństwa dopiero po 0.0.65 na nowym dysku, który podłączy właściciel. Praca wstrzymana na polecenie właściciela (pauza).
- **2026-10-17 — KOMPUTER STACJONARNY (Ryzen 7 5700X3D, RX 9070 XT, 32 GB) zamiast laptopa; seria 0.0.63 / C6.6 „każda karta graficzna”:** commity `6ff0c311` (wykrywanie DXCore: zintegrowana/LUID/sterownik, klasy niezależne od producenta, klasa Integrated, pomiar PDH bez nvidia-smi), `72c4e1cb` (DirectML dla Supertonic — ORT 1.23.0 DML + DirectML 1.15.4 z NuGet, `--dml`, karta ≥ 12 GB; Qwen3-ASR przez Vulkan na AMD/Intel w pakietach 5/4; APU z rezerwą ≥ 6 GB = karta; 15 poprawek po przeglądzie agentów; ADR 0028 PROPONOWANY — podpis właściciela + licencja DirectML z danymi diagnostycznymi), `55bc0f06` (Qwen3-ASR `--device` przy ≥ 2 kartach), `d02237bd` (verify.mjs: Git Bash), `0011a06b` (14 poprawek po 2. przeglądzie: rezerwa APU liczona raz — `Profile::system_ram_mb`, pomiar pamięci głównej karty, test terminala bez Claude Code), `c3ec80fb` (4 poprawki po 3. przeglądzie: widok silników bez listy kart co 20 ms, kilka kart NVIDIA — wiersz głównej karty, `mainCardMb` jak `primary_gpu`, angielski komunikat braku programu). Po północy: `87a4a156` (raport, pamięć — zasady pracy z laptopa przywrócone do `~/.claude/.../memory`, bo lustro memory-sync kasowało je z repo), `7698da68` (ammonia 4.2.4 za stałą zgodą na pobrania, `Select` w ui-kit nie pomija kliku w pierwszych 300 ms, testy w przeglądarce na Chromium Playwrighta 1194: 467/467; końcowa budowa `target/shell/debug/alfa-desktop.exe` z `7698da68`, e2e-app 37 + 1 pominięty, pokrycie 75 %). verify wersja na `c3ec80fb`: 16 zielonych, 2 czerwone (Edge, zip), 1 nieuruchomiony (jq); Rust 2897/0. Potem: `e91b1ef3`/`059b9283` (e2e-app `z-commands-2.spec.ts` — pokrycie łącza 235/264, 89 %), `c24da9ad` … `96588e5b` (całościowy przegląd od 0.0.62 i dwa przeglądy poprawek: na Vulkanie i głównej karcie < 12 GB Qwen3-ASR na procesorze — `stt::qwen_free`; rezerwa 3 GB sprawdzona i odrzucona; kompromis klasy „nieznana” zależnie od producenta; `HardwarePort::gpu_memory` bez 250 ms; klucz TTS; primary_gpu przy bliźniaczych kartach; UI primaryGpu i chatMemoryMb). Raport dla właściciela: `alfa/docs/plan/28-raport-nocny-2026-10-18.md`. `target\debug` urósł do 170 GB w kilka godzin (duplikaty plików testowych) — sprzątnięte 130 GB. Pomiary RX 9070 XT: Supertonic 0,58 s/zdanie (CPU 0,95 s), 538 MB; Qwen3-ASR Vulkan 0,16–0,23 s, 2878 MB. Niesprawdzone: laptop z iGPU, APU, Intel, DirectML na NVIDIA/Intel. NIE wgrano 0.0.63 do Alfa-uruchomiona (tam 0.0.62). Awaria PC 20:31 (brak pamięci + resety USB E:) — pamięć `awaria-2026-10-17-obciazenie`; ciężkie biegi z `CARGO_BUILD_JOBS=6`. `verify.mjs` bierze Git Bash (na tym PC `bash` = WSL); brak `jq` — graf sprawdzany ręcznie. Kopie: F:\Alfa\kopia-2026-10-17_2115, kopia-2026-10-18_0054 i **kopia-2026-10-18_0330** (stan `678e7b2a`: 5854 sumy SHA-256 zgodne, paczka git pełna) — noc zakończona 3:32, praca zatrzymana. Wersja do wgrania: `target/shell/debug/alfa-desktop.exe` z `21897bf0` (decyzja właściciela).
- **Pamięć projektu ma kopię w repozytorium:** `alfa/docs/pamiec-projektu/` (stan, zasady pracy, dziennik sesji; opis w `README.md`). Na końcu każdej sesji: `bash scripts/memory-sync.sh` i commit.
- **NOC 15/16.10 — ZAKOŃCZONA: 0.0.62 wgrana 5:44, verify wersja 19/19 i verify sprzet zielone, kopie na F:/Alfa. (cel właściciela: praca do 4:30, wersja zbudowana i przetestowana, plan HTML, raport, kopia na F:\Alfa do 5:00):** 0.0.60 = seria 1 (wgrana 20:13). 0.0.61 = seria 2 (verify do przeglądarki: Rust 2858/1 czerwony test czasowy terminala → poprawiony, UI 381, przeglądarka 437; bieg zatrzymany przed etapem aplikacji, nie wgrywana osobno). 0.0.62 = seria 3 (jednostki, daty, F1, trafienia instrukcji w Ustawieniach, JSON szablonów/poleceń, statystyki rozmowy, szukanie/eksport Tablicy, zapamiętany tryb skupienia LayoutPrefs.focus, „Zapisz jako szablon”, E.1 wąskie okno 360/400) + seria 4 (liczby słownie/rzymskie, procenty, lata przestępne, wybór z listy, liczenie znaków, podpowiedź „Bez modelu”, przykłady→tester, kopia rozmowy z notą AI, statystyki z menu sesji) + poprawki po przeglądzie serii 3. Kopia robocza alfa-s3 (gałąź seria3) z dowiązaniami node_modules — usuwać dowiązania przez rmdir, NIE rm -rf. Raport: docs/plan/27-raport-nocny-2026-10-16.md. Kopia: scripts/backup.sh → F:\Alfa\kopia-<data>.
- **SERIA 1 (cel właściciela 15.10: „dalsze etapy, wersja co 10 punktów”):** 0.0.59 (C6.4 przeglądarka + C6.5 tools-dev) zatwierdzone, osobno niebudowane — weszły do 0.0.60. Punkty: 1 C4.3 Moduły (`230cbcf4`), 2 C4.4 Zaawansowane — surowy TOML (`a99b6d53`), 3 C2.9 informacja AI + znaczniki w eksporcie (`4f7600ae`), 4 C2.9 karty modeli (`88ecf161`), 5 P5 polecenia bez modelu — `lib-quick` (`e2393c13`)., 6 P5 quiz (`9c033118`)., 7 P5 własne polecenia z testerem (`2d3f5ce2`)., 8 P4 eksport JSON (`5641ebd1`)., 9 P4 szablony promptów (`e2169bab`)., 10 C4.1 gdzie trafiają dane (`c4902ef8`). SERIA 1 KOMPLETNA — zbudowana jako 0.0.60 i wgrana 15.10 o 20:13 (wpis NOC 15/16.10). UWAGA przy budowie: nowy crate lib-quick → `cargo update --offline -w` w src-tauri. Po 10 punktach: CHANGELOG (0.0.59 + seria), `set-version` 0.0.60, `cargo update --offline -w` w src-tauri, clippy, `verify-app.sh`, `deploy-owner.sh`.
- **0.0.58 (15.10 po południu, na polecenie właściciela):** C6 — `system_software` (programy na maszynie w zasadach agentek), serwery MCP użytkownika (`app-mcp`, Ustawienia → Serwery MCP, przepisy Playwright/Chrome DevTools/pliki/Git/GitHub, zgody z odciskiem, zmienne tajne w Credential Manager, Broker `gui.control(mcp-<id>)`), diagnostyka komputera `pc-diag` (contract/impl/fake + `platform-windows-diag-impl` PowerShell/CIM + `app-diag`; Ustawienia → Diagnostyka komputera, `diag_*`, narzędzie `system_diagnostics`). Diagnostyka na laptopie: BSOD 0x116 dziś 11:42 (sterownik RTX), 4 nieoczekiwane wyłączenia, 200 błędów dysku E: (mostek USB Realtek RTL9210B), 87 °C. Potem zrobione w 0.0.59 (zbudowane w 0.0.60): C6.4 przeglądarka (scroll/klawisze/select/historia/czekanie/konsola), C6.5 Git i testy. app-core 7995/8000 linii (18.10; przy 0.0.58 było 7989) — przed dodaniem kodu przenosić czyste funkcje do app-modules.
- **0.0.55–0.0.57 (15.10 rano):** logowanie dla zwykłej osoby (instrukcje, pusty folder `%LOCALAPPDATA%\Alfa\logowanie`, wykrycie zalogowania, `bridges_mark_login`), parser `agy` zgodny z prawdziwym 1.2.11, klawisze terminala po kolei. 0.0.57 zielony 18/18, wgrany. Antigravity zalogowany i włączony u właściciela.
- **0.0.54 (15.10, 4:03):** werdykt Krytyczki rozpoznawany mimo literówek lekkiego modelu; wgrane. Próba obowiązków na przeciążonej maszynie: rano 329 s, wieczór >10 min (1,4 słowa/s, 2 GB RAM wolne) — środowisko, nie kod.
- **0.0.53 (noc 14/15.10, praca do 5:00 na polecenie właściciela):** dokument 25 rozstrzygnięty: izolacja poleceń powłoki integralnością Low (nie AppContainer — zepsułby git/cargo/npm), macierz dowodów = `docs/DOWODY.md`, Antigravity zatwierdzony, Markdown w wątkach (7,8 ms). wasmtime 36.0.18. Pokrycie łącza 71,8 %, bramka 70 %. O 10:00 test konta Google z właścicielem.
- **0.0.52 (2026-10-14 wieczór):** Ustawienia → Stan połączeń (`ConnectionsPage.svelte`, `logic/connections.ts`), `bridges_check` (`app-bridges/src/login.rs`, `CliProbe::run_output`), sprawdzenie po „Gotowe” (`TerminalDialog.svelte`), Antigravity: `BridgeKind::Antigravity`, `agent-backends-impl/src/agy/`, profil `agy_login`, ostrzeżenie na karcie, blokady `.agy`/`.gemini`; wpis rejestru czekał na podpis (dokument 25 §3) — zatwierdzony w 0.0.53. Zawieszenie próby obowiązków = obciążenie równoległym testem, nie kod.
- **Sesja 2026-10-14 (do 18:20).** Wersja **0.0.51** (commity od `99367ee7` do `8006e86a` i dalej) wgrana do `Alfa-uruchomiona/` o 18:17 i uruchomiona na profilu właściciela.
- **Wersja:** 0.0.51 — pełny `verify wersja` (NOWY zestaw = bramki CI, 18 etapów) 17/18 zielonych przy biegu o 18:05; czerwony `zaleznosci` = nowe RUSTSEC-2026-0346/0347 w rustls 0.23.45 → podniesione do 0.23.46 (oba Cargo.lock), `cargo deny` czysty, testy TLS 136/0, ponowna budowa i prawdziwa aplikacja 28/28 zielone. Liczby: Rust 2732/0/34, vitest 356, przeglądarka 357, prawdziwa aplikacja 28, pokrycie łącza 122/233 (52,4 %). `verify sprzet` dla 0.0.49–0.0.51 NIE puszczony.
- **0.0.50:** test klucza OpenRoutera (`GET /key`), wspólna kolejka modeli wszystkich kont (zakładka „Wszystkie konta”, `model-queues.json`).
- **0.0.51 — dobór modelu według trudności (zgłoszenie właściciela):** ocena trudności `providers-contract/src/difficulty.rs`, wskazówka trasy `RouteHint` w `RequestMeta::route`, szczeble `router-contract/src/rung.rs`, kolejność w `router-impl/src/difficulty.rs`; Krytyczka na szczeblu sprawdzania, poprawka po odrzuceniu od modelu mocniejszego (`agent-runtime-impl` critic.rs/turn.rs), klasa roli przy delegacji; ustawienia `model-routing.json` (app-modules `route/routing_settings.rs`), polecenia `routing_get/set/preview`, strona Ustawienia → Dobór modelu (`RoutingPage.svelte`, `RoutingPreview.svelte`). DECYZJA: ręczna kolejka (konta/wspólna) ma pierwszeństwo; przełącznik `reorder_queues`.
- **Audyt zewnętrzny 2026-10-14** (`../audyt-2026-10-14/RAPORT.md`, 17 pozycji): naprawione w 0.0.51 A-01…A-14, A-16, A-17; A-15 częściowo. Zestawienie i propozycje: `alfa/docs/plan/25-do-przegladu-po-audycie.md`. CZEKAŁO NA WŁAŚCICIELA: izolacja procesu poleceń powłoki (AppContainer — A-07 część 2), jedna macierz dowodów (A-15) — rozstrzygnięte w nocy 14/15.10 (wpis 0.0.53). Nowe bramki: `scripts/check-size.mjs` (w `pnpm lint`, wyjątki `scripts/size-exceptions.json`), `verify.mjs` = bramki CI + wynik NIEKOMPLETNY, `set-version.mjs` pilnuje obu Cargo.lock, generator tokenów formatuje prettierem.
- **Dysk E: rozłączał się 2026-10-14 rano** (Ntfs 140/50, opóźniony zapis $Mft) przy dużym obciążeniu — commitować często; kopia zmian na C: w scratchpadzie sesji.
- **W TOKU / NASTĘPNE (stan z 14.10 — nieaktualne, zostawione jako historia):** pokrycie poleceń 52,4 % → 70 % (cel właściciela) — osiągnięte: 71,8 % w 0.0.53, 89 % (235 z 264) w 0.0.63; decyzje z dokumentu 25 — rozstrzygnięte 14/15.10; `verify sprzet` — zielony dla 0.0.62 (dla 0.0.63 nieuruchomiony); testy `--pulpit` — w raportach 27 i 28 nieuruchamiane. Bieżąca lista: `handoff.json` → `next_session`.
- **Ogólny stan (18.10):** Częściowy — aplikacja działa na laptopie (okna, ustawienia, skalowanie); model lokalny
  odpowiada (próby obowiązków, `verify sprzet` 0.0.62 — raporty 26 i 27); rozmowa żywą mową czeka na ocenę słuchem
  właściciela; Broker poza procesem niesprawdzony na żadnej maszynie; 0 ze 108 kryteriów potwierdzonych na maszynie

## Plan v6 (2026-10-09)

- Dokumenty: `alfa/docs/plan/README.md` (spis), `03-roadmapa.md` (żywy, etapy A–H), `00-stan-wykonania.md`
- HTML: `alfa/docs/html/index.html` — buduje `node scripts/docs-html.mjs` (`--check` sprawdza odnośniki; jest w `gates.ps1`)
- Podgląd w przeglądarce sesji: `preview_start name=alfa-docs` (port 5180)
- Projekty ADR 0017–0024 — status „Proponowany”, czekają na podpis właściciela; SPEC-e 9 nowych modułów
- ADR 0028 (DirectML dla Supertonic na karcie ≥ 12 GB, Qwen3-ASR przez Vulkan na AMD/Intel; 2026-10-17) — „Proponowany”,
  czeka na podpis i potwierdzenie licencji DirectML (dane diagnostyczne, §2a); blokuje zakończenie części DirectML C6.6
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

## Testy z 2026-10-10 (wersja 0.0.17 — historia; aktualne wyniki: `handoff.json` → `test_results`)

```
vitest 295/295 · E2E (PW_WORKERS=4 PW_CHANNEL=msedge) 239 z 241 (znany próg strumienia + losowy test panelu głosu)
Rust: cargo test --workspace 2565 zielonych / 0 czerwonych / 33 pominięte; clippy --workspace czysty · e2e-app 15/15 na profilu tymczasowym
NIE sprawdzono: prawdziwa kamera (kamera laptopa odłączona sprzętowo — MFEnumDeviceSources = 0 urządzeń),
brzmienie głosu na słuch, import kluczy z prawdziwych zmiennych, rozmowa głosowa na żywo
E2E nie puszczać równolegle z cargo (fałszywe przekroczenia czasu, 10 min zamiast 3)
```

## Aktywne błędy

| Problem                                                                                            | Moduł   | Priorytet           | Status                                                           |
| -------------------------------------------------------------------------------------------------- | ------- | ------------------- | ---------------------------------------------------------------- |
| Rozmowa żywą mową nieoceniona słuchem (model lokalny odpowiada od 0.0.8; próby obowiązków zielone) | całość  | wysoki              | ocena właściciela (silniki i model pobrał właściciel przy 0.0.8) |
| Build wydaniowy ma `panic = "abort"`                                                               | powłoka | średni              | decyzja właściciela                                              |
| Analizator powłoki S-01…S-04, S-06; schowek C-02, C-03                                             | tools-* | wysoki (S-01, S-03) | decyzja właściciela                                              |
| Testy czasowe (strumień, paleta) na granicy progu                                                  | ui/e2e  | niski               | decyzja o progu                                                  |

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

## Następne kroki (stan z 2026-10-10 — historia; bieżące: `handoff.json` → `next_session`)

1. Właściciel: zgoda na pakiet ok. 6 GB; `gh auth login`; ustalenia bezpieczeństwa; podpis ADR 0017–0024;
   ADR 0024 (licencja silnika mowy); progi kryteriów `ACC-V6-…`.
2. Sesja: etap A.4–A.6 (pierwsza rozmowa i głos na laptopie, pomiary) zaraz po zgodzie na pobranie.
3. Sesja (bez czekania): etap B — B.1 generator typów TS, B.2 numery sekwencji zdarzeń, B.3 błędy strukturalne,
   B.5, B.6, B.9; C2.1 zestaw ewaluacyjny; E.1 układ na wąski ekran.
4. E2E na tym laptopie: `PW_WORKERS=4 PW_CHANNEL=msedge`.

## Info o sesji

- Pierwsza sesja: 2026-10-07 23:31 → 2026-10-09 (laptop); ostatnia aktualizacja tego pliku: 2026-10-18 (laptop,
  po powrocie z komputera stacjonarnego; porządek dokumentów w serii 0.0.64)
- Model: Opus 5.5
- Maszyny: laptop i7-13620H, RTX 4050 6 GB + Intel UHD Graphics, 16 GB RAM (bieżąca) i komputer stacjonarny
  Ryzen 7 5700X3D, RX 9070 XT 16 GB, 32 GB RAM — na zmianę, zob. „Stan projektu” → Maszyny
