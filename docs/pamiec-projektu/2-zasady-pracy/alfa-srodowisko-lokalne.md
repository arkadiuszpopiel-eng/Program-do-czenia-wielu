---
name: alfa-srodowisko-lokalne
description: "Alfa — kopia robocza na przenośnym dysku E:, środowisko przenośne, pułapki budowy i powłoki na Windows"
metadata:
  node_type: memory
  type: reference
  originSessionId: 526ab23b-611a-489a-a8a3-7c1153e69825
  modified: 2026-10-22T00:47:12.024Z
---

Stan z 2026-10-08 (laptop i7-13620H, RTX 4050 6 GB) — sprawdź, czy nadal aktualne.

- **Kopia robocza: `E:\PROGRAMY W CZASIE PRAC\Alfa AI Program\alfa`** — decyzja właściciela (2026-10-08): pracuje na
  E:, bo dysk jest przenośny (USB NVMe). Repo git to podkatalog `alfa/`; katalog sesji nie jest repozytorium.
- **Rozłączenia E: (14–15.10.2026):** pod dużym obciążeniem (kompilacja, testy) sterownik USB resetuje dysk
  (UASPStor 129, disk 153, potem Ntfs 50/140); kabel jest podłączony, Alfa uruchomiona z E: wtedy się zamyka.
  Właściciel zdecydował 2026-10-15: **wszystko zostaje na E:** — nie proponować przenoszenia `target` ani niczego
  na C:. Po rozłączeniu: `git fsck`, `cargo clean -p <pakiet>` przy błędzie linkera LNK1207, ponowne uruchomienie Alfy.
- To jedyna kopia: stare `D:\alfa`, `D:\alfa-narzedzia` i migawka ZIP z GitHuba usunięto 2026-10-09 za zgodą
  właściciela (Kosz). Z globalnego `~/.npmrc` usunięto `cache=D:\alfa-narzedzia\npm-cache` (odtwarzało katalog na D:
  przy każdym `npx` poza `portable-env`, także z haka formatującego); kopia starego pliku:
  `%TEMP%\npmrc-przed-porzadkami.txt`. Skrypt `install-on-drive.ps1` wpisuje tę linię ponownie przy instalacji.
- Kopiowanie repo między dyskami potrafi zamienić dowiązanie w `alfa-narzedzia/pnpm-store/v10/projects/` w pełną
  kopię repozytorium (2026-10-09 leżało tam 39 GB) — po takiej operacji sprawdź ten katalog.
- Narzędzia leżą obok repo w `alfa-narzedzia` (rustup, cargo, nodejs z pnpm, StrawberryPerl, pnpm-store). Przed
  `cargo`/`pnpm` zawsze: `source scripts/portable-env.sh` (Git Bash) lub `. .\scripts\portable-env.ps1`.
  Bez tego Git Bash bierze własnego Perla i budowa OpenSSL (SQLCipher) pada.
- Bramki lokalne: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\gates.ps1` (`cargo fmt --all` pada na
  Windows z os error 206 — skrypt formatuje partiami).
- Po skopiowaniu repo między dyskami trzeba `pnpm install --frozen-lockfile` (kopiowanie psuje dowiązania pnpm).
- Narzędzie Bash w tej konfiguracji zamienia `\\` na `\` także w heredocach z `'EOF'` — skrypty Pythona z
  backslashami zapisywać narzędziem Write, nie heredokiem.
- E: ok. 107 GB wolnego po porządkach z 2026-10-09; pełne `cargo test --workspace` zajmuje ok. 100 GB w `target/debug` (patrz niżej). `gh`
  niezalogowany, brak `cargo-deny`.
- Plan: `alfa/docs/KONTYNUACJA.md`; kryteria: `alfa/docs/STATUS.md`; katalog modułów: `alfa/docs/CATALOG.md`;
  mapa dokumentów: `alfa/docs/README.md`. Logi uruchomionej Alfy: `%LOCALAPPDATA%\Alfa\logs`.

Uruchamianie prawdziwej aplikacji bez ruszania danych właściciela (sprawdzone 2026-10-08):

- budowa: w `apps/desktop/src-tauri`, `CARGO_TARGET_DIR=<repo>\target\shell`, `CARGO_PROFILE_DEV_DEBUG=0`,
  `cargo build --features "e2e tauri/custom-protocol"` (ok. 10 min od zera, ok. 5 GB); pierwsze polecenie w tle
  potrafi zawisnąć po limicie czasu narzędzia — budować na pierwszym planie z `timeout 570` i powtarzać;
- test: ustawić `APPDATA` i `LOCALAPPDATA` na katalog tymczasowy (aplikacja bierze ścieżki z tych zmiennych),
  potem `powershell -File apps/desktop/ui/e2e-app/run.ps1 -Exe <...>\alfa-desktop.exe -OutDir <katalog>`;
  bez tego skrypt odmawia startu, gdy istnieje `%LOCALAPPDATA%\Alfa`.
- Zachowanie procesów konsolowych (kodowanie wyjścia, `chcp`, zmienne środowiskowe) sprawdzać testem przez prawdziwy
  `ExecPort` (`crates/platform-windows-impl/tests/shell_utf8.rs` jako wzór), nie sondą z Pythona — proces z dołączoną
  konsolą zachowuje się inaczej (2026-10-09: sonda pokazała, że `chcp 65001` działa w `cmd`, a pod `ExecPort` nie działa).
- E2E na tym laptopie: `PW_WORKERS=4 PW_CHANNEL=msedge pnpm test:e2e` (przy domyślnych 8 procesach testy padają z obciążenia).
- `cargo clean` w repo jest dozwolone (własne wyniki testów); usuwanie katalogów `rm -rf` blokuje zabezpieczenie sesji.
  Do usuwania katalogów za zgodą właściciela: przeniesienie do Kosza przez PowerShell
  (`Microsoft.VisualBasic.FileIO.FileSystem::DeleteDirectory` z `SendToRecycleBin`); Kosz odrzuca pamięć podręczną npm
  w całości — najpierw `npm cache clean --force --cache <katalog>`, potem podkatalogi osobno.
- `cargo test --workspace` na tym laptopie (16 GB RAM) uruchamiać z `-j 3` i bez równolegle działających przeglądarek —
  przy domyślnej równoległości linker kończy pamięć („memory allocation failed”, 2026-10-09). Zajmuje ok. 100 GB w `target/debug` — 2026-10-09 zapełniło dysk (LNK1180);
  testować pakietami (`cargo test --no-fail-fast -j 3 -p <crate> …`), a po pełnym przebiegu `cargo clean --profile dev`
  (zostawia `target/shell` z aplikacją). Bez `--no-fail-fast` cargo kończy na pierwszym czerwonym pakiecie.
- Numer wersji zmieniać wyłącznie przez `node scripts/set-version.mjs <numer>` (sprawdzenie: `--check`): ręczne podbicie
  czterech plików zostawiało `crates/*/module.toml` przy starej wersji i psuło ok. 20 testów manifestów (0.0.2–0.0.5).
- Narzędzie Bash psuje ukośniki odwrotne także w heredocach: pliki z `\` (ścieżki Windows, wyrażenia regularne) pisać
  narzędziem Write/Edit, nie przez `cat <<EOF` ani `sed`.
- Diagnoza zgłoszeń z działającej Alfy: `%LOCALAPPDATA%\Alfa\logs\diagnostics\*.ndjson` (zdarzenia magistrali, pole
  `event.kind`: `router.decision`, `voice.dialog.state_changed`, `voice.wake.mic_state`…) pokazuje, gdzie tor się urywa —
  2026-10-09 tak znalazłem model `tts-1` w trasie i dialog głosowy zawieszony w „user_speaking”.
- Atrapy audio mają wspólny zegar z potokiem, prawdziwe WASAPI znaczy ramki czasem QPC (od startu systemu): błędy
  podstawy czasu wychodzą dopiero na sprzęcie — sprawdzać testem na prawdziwym urządzeniu (`app-core/tests/voice_clock.rs`).
- Dysk w trakcie sesji (2026-10-10): każde podbicie wersji zostawia w `target/debug/deps` i `target/shell/debug`
  komplet starych artefaktów (62 GB + 44 GB po 10 wersjach jednego dnia). Zamiast `cargo clean` usuwać stare pliki:
  `find target/debug/deps -maxdepth 1 -type f -mmin +180 -delete`, to samo dla `target/shell/debug/deps` i katalogów
  w `target/shell/debug/incremental` — odzyskało ok. 70 GB bez pełnej przebudowy bieżących pakietów.
- E2E interfejsu nie puszczać równolegle z `cargo` (2026-10-10: 10–17 min zamiast 3 i fałszywe przekroczenia czasu);
  nie przebudowywać `ui/dist` w trakcie budowy powłoki (powłoka osadza `dist`).
- Kamera laptopa bywa odłączona sprzętowo (zasłonka / klawisz Fn): `MFEnumDeviceSources` zwraca 0 urządzeń, a
  `Get-PnpDevice` pokazuje „Chicony USB2.0 Camera” z `Present False` — testu na prawdziwej kamerze nie da się wtedy
  wykonać bez właściciela.
- W `index.html` nie wstawiać bloku `<style>`: Tauri dodaje wtedy do CSP znacznik jednorazowy, który wyłącza style
  nadawane elementom (`style:` w Svelte); wyłapuje to tylko test CSP w `e2e-app`. Style startowe: `ui/public/boot.css`.
- Nowe polecenie rdzenia trzeba dopisać także do `ui/src/lib/api/COMMANDS.md` — `build.rs` powłoki generuje z niego
  uprawnienia `allow-*`, a `dto_roundtrip` porównuje z nim przykłady danych.

Podgląd interfejsu (`vite preview`) odpowiada pod `http://localhost:4173`, nie pod `127.0.0.1` — skryptom zrzutów podawać `--url http://localhost:4173`. Nie zatrzymywać go przez `taskkill /IM node.exe` (zabija wszystkie procesy Node na maszynie, także cudze) — tylko po PID albo po porcie. Po `find target/... -mmin +180 -delete` pierwsza kompilacja trwa ok. 10 min (pełna przebudowa).

**Miejsce na dysku (2026-10-11):** `alfa/target/debug/deps` urósł do 93 GB i właściciel zapytał z irytacją, czemu
folder ma 112 GB. Po każdej wersji sprawdzać `du -sh alfa/target/debug` i przy ponad ok. 20 GB usuwać
`deps`, `build`, `incremental`, `.fingerprint` (pliki odtwarzalne; `target/shell` z plikiem aplikacji zostawić).
Przed usunięciem upewnić się, że nie działa `cargo`. Duże pobrania (środowiska Pythona, modele) kłaść poza repo
i podawać właścicielowi rzeczywisty rozmiar.

**Noc 2026-10-11/12 (8 wersji z rzędu):** dysk zapełnił się znowu (LNK1180, „no space on device”), mimo
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0` — każde podbicie wersji przebudowuje cały workspace i dokłada
ok. 15 GB do `target/debug/deps` (121 GB po 8 wersjach). Zasady: (1) przed każdym większym `cargo test` sprawdzić
`df -h /e`; (2) sprzątać **tylko** `target/debug/deps` i `incremental` — katalog `build` zostawić, bo trzyma
zbudowany OpenSSL (jego ponowna budowa to ok. 20 min); (3) przy pracy seriami podbijać wersję rzadziej (kilka zmian
na wersję); (4) nie edytować źródeł Rust, gdy w tle kompiluje się `cargo` — bierze stan mieszany.
**PC stacjonarny, 2026-10-22 2:30:** po dniu z trzema `verify wersja` `target/debug` miał 230 GB, na E: zostało
14 GB i `verify` oznaczył linter i testy jako NIEURUCHOMIONE (próg ok. 30 GB) — bez czerwonego wyniku, łatwo przeoczyć.
Przed każdym `verify` sprawdzić `df -h /e`; sprzątanie bez pełnej przebudowy: usunąć z `target/debug/deps` tylko
`.exe` i `.pdb` starsze niż ostatni pełny bieg testów (152 GB) oraz `incremental`. Ukośniki: tej nocy heredoc zepsuł
też literały `'\\'` i kontynuacje wiersza w napisach Rust (ciągi spacji w komunikatach) — tylko Edit/Write.

Skrypty poprawek w katalogu roboczym sesji zapisywać pod nowymi nazwami (podkatalog na daną noc): plik o starej
nazwie nie daje się nadpisać bez odczytu, a uruchomiony przez pomyłkę stary skrypt wprowadza obce zmiany.

**Sprawdzanie funkcji opartych na modelu bez danych właściciela (2026-10-12):** profil tymczasowy
(`APPDATA`/`LOCALAPPDATA` na katalog w `alfa/target/`), do niego KOPIA modelu i silnika z `%LOCALAPPDATA%/Alfa`
(nie dowiązanie — sprzątanie `rm -rf` poszłoby za dowiązaniem do danych właściciela), uruchomić
`target/shell/debug/alfa-desktop.exe` (build `e2e` wystawia CDP na 9222) i wołać polecenia rdzenia przez
`chromium.connectOverCDP` + `window.__TAURI_INTERNALS__.invoke`. Tak wyszło, że gotowe obowiązki agentek (0.0.40)
dawały raport z wymyślonymi danymi — funkcji, której wynik pisze model, nie wypuszczać bez takiej próby.
Po próbie zabić `alfa-desktop.exe` i `llama-server.exe`, usunąć profil, uruchomić aplikację z powrotem na profilu
właściciela. Okno „Co nowego” zasłania interfejs po zmianie wersji — nie zamykać go właścicielowi.

**Limity rozmiaru po każdej zmianie w Rust (2026-10-22):** `node scripts/check-size.mjs` woła `git ls-files` i bez
wyjątku `safe.directory` pada (dysk E: ma innego właściciela). Uruchamiać z
`GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=safe.directory GIT_CONFIG_VALUE_0='*'` (bez zmiany konfiguracji globalnej) i nie przez
`| tail`, które ukrywa kod wyjścia. Crate'y przy samym limicie 8000 linii: app-core, app-api — każda dopisana linia
w `src` wymaga sprawdzenia; 22.10 zmiana komunikatu w app-api przeszła testy, a przekroczyła limit (8007).
