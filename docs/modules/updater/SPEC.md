# updater — SPEC (szkic v0)

## Cel
Launcher i aktualizacje: wersje side-by-side w `%LOCALAPPDATA%\Alfa\versions\<ver>`, **stały launcher** `%LOCALAPPDATA%\Alfa\alfa.exe` (stała ścieżka dla skrótów, AUMID, autostartu, protokołu, „Wyślij do"), stały folder danych WebView2 poza katalogiem wersji, aktualizacje z własnego repo z podpisem minisign, rollback = przełączenie launchera na poprzednią wersję, helper `uiAccess` instalowany osobno (jednorazowy UAC) (PLAN §1.2, §8.7).

## Fala i priorytet
F1: launcher + układ katalogów. F3: aktualizacje, rollback (z `watchdog`). F6: instalacja helpera `uiAccess`. P0.

## Kontrakt (szkic Rust)
```rust
// updater-contract — SZKIC
pub struct Layout { pub root: PathBuf /* %LOCALAPPDATA%\Alfa */, pub launcher: PathBuf, pub versions: PathBuf, pub webview_data: PathBuf, pub current: Version, pub previous: Option<Version> }
pub struct Release { pub version: Version, pub url: Url, pub sha256: Hash, pub minisign: Signature, pub notes: String /* „Co nowego” */, pub min_previous: Option<Version> }
pub enum UpdateState { Idle, Checking, Available(Release), Downloading { pct: u8 }, Verified, Staged, RestartRequired, Failed(String) }
pub trait Updater: Send + Sync {
    fn layout(&self) -> Layout;
    fn check(&self) -> Result<Option<Release>>;
    fn download_and_stage(&self, r: &Release, cancel: CancelToken) -> BoxStream<UpdateState>;   // wznawialne, weryfikacja podpisu
    fn switch_to(&self, v: Version) -> Result<()>;                                                // launcher → wersja (atomowo)
    fn rollback(&self) -> Result<Version>;                                                        // → previous
    fn mark_good(&self, v: Version) -> Result<()>;                                                // po zdrowym starcie
    fn prune(&self, keep: u8) -> Result<()>;
}
```
Zdarzenia (Audyt): `updater.available`, `updater.staged`, `updater.switched`, `updater.rolled_back`, `updater.signature_invalid`, `updater.pruned`.

## Zależności
`core-bus/config/log-contract`, `platform-windows-contract` (pliki, procesy, rejestr HKCU), `watchdog-contract` (rollback po crash-loopie), `shell-integration-contract` (rejestracje wskazują launcher). Zewnętrzne: minisign (weryfikacja własnym kluczem publicznym wbudowanym).

## Niezmienniki
- Bez MSIX; wszystko w profilu użytkownika (HKCU, `%LOCALAPPDATA%`), bez UAC — poza jednorazową instalacją helpera `uiAccess` w `Program Files` (F6).
- Paczka bez ważnego podpisu minisign i zgodnego hasha nigdy nie jest uruchamiana; klucz publiczny w binarium, prywatny u właściciela (bramka #10).
- Przełączenie wersji atomowe (zapis wskaźnika + rename); poprzednia wersja pozostaje do `mark_good` nowej.
- Rejestracje systemowe nigdy nie wskazują katalogu wersji; folder danych WebView2 wspólny między wersjami.
- Aktualizacja nie startuje w trakcie zadania agentki ani rozmowy głosowej; „Co nowego" po aktualizacji.
- Ulepszacz/agentki nie mają dostępu do updatera (Jądro).

## Zdolności / uprawnienia
`net.egress(<repo aktualizacji>)`; `fs.write(%LOCALAPPDATA%\Alfa\versions\**)` — jądro/launcher, nie agentki.

## Izolacja
Launcher: osobny mały proces (`process`, uruchamia watchdog i jądro); moduł sprawdzania aktualizacji `inproc`, `on-demand`.

## Budżet zasobów
Launcher ≤ 3 MB RAM, start ≤ 100 ms zanim przekaże do wersji; instalacja bazowa (jądro + UI) ≤ 40 MB (§3.4); `keep = 2` wersje.

## Konfiguracja (klucze TOML)
`[updates] channel = "stable"`, `check = "daily" | "manual"`, `auto_download = true`, `auto_install = "on_idle" | "ask"`, `keep_versions = 2`, `repo_url = "<własne repo>"`.

## Wkład do UI
Ustawienia → Aktualizacje (stan, „Co nowego", rollback ręczny), baner „wymagany restart", stan „przerwane pobieranie" (§14.4).

## Testy akceptacyjne
- `ACC-F1-updater-01`: układ katalogów i launcher — skróty/AUMID/protokół działają po zmianie wersji (z `shell-integration`).
- `ACC-F3-updater-02`: update + rollback: nowa wersja z wymuszonym crash-loopem → watchdog cofa do poprzedniej ≤ 30 s; sesje i konfiguracja nietknięte.
- `ACC-F3-updater-03`: paczka z błędnym podpisem/hashem odrzucona 100/100; pobieranie wznawialne po przerwaniu sieci.

## Fake
`updater-fake`: lokalne repo z fixture'ami wydań (dobre/złe podpisy), wersje jako katalogi tmp, `switch_to` bez restartu.

## Otwarte pytania
- ~~Format repo wydań~~ — rozstrzygnięte w F3: manifest JSON per kanał + ZIP (niżej); delta-update poza v1.
- ~~Aktualizacja samego launchera~~ — rozstrzygnięte w F3: zapis obok + samotest + zamiana przy starcie (niżej).
- Serwer wydań: prywatne repo GitHub wymaga uwierzytelnienia do pobrania zasobów — adres `ALFA_UPDATE_FEED`
  musi być publicznym HTTPS (np. publiczne repo wydań albo własny serwer statyczny). Decyzja człowieka (#10).

## Zmiany po implementacji (F1, `updater-contract/-impl/-fake`, 2026-10-01)
- **Kontrakt synchroniczny** `Updater { layout, state, installed, select_launch, switch_to, rollback, mark_good,
  record_exit, prune, check, verify_release }`; `download_and_stage` (pobieranie, wznawianie, rozpakowanie) — F3.
- `current.json`: `{ schema: 1, active, previous, pending, crashes, bad[], updated_at }`, zapis atomowy (tymczasowy +
  `fsync` + `rename`). Brak/uszkodzony/nieznany schemat → launcher bierze najnowszą poprawną wersję (bez zapisu).
- Wersja poprawna = katalog o kanonicznej nazwie semver z `alfa-desktop.exe` i (opcjonalnie) zgodnym `version.json`.
- Wybór: aktywna → (brak/uszkodzona/wycofana) poprzednia → błąd. Rollback = zamiana aktywnej z poprzednią.
- **Crash-loop** (`CrashPolicy`: okno 15 s, 2 awarie): wyjście ≠ 0 w oknie albo błąd startu = szybka awaria; wersja
  `pending` (przed `mark_good`) wraca do poprzedniej po pierwszej, dobra — po drugiej (wcześniej ponowienie); wersja
  wycofana trafia do `bad` (zdejmuje ją dopiero jawne `switch_to`). Kod 0 i wyjście po oknie to nie awaria startu.
  `mark_good` woła aplikacja po zdrowym starcie (launcher tylko zeruje licznik).
- `prune(keep)`: zostaje `keep` najnowszych, zawsze aktywna, poprzednia i wersje nowsze od aktywnej (przygotowana
  aktualizacja); obcych katalogów nie dotyka.
- **minisign**: `minisign-verify` 0.3 (bez zależności), weryfikacja strumieniowa, tylko podpisy „prehashed”; klucz
  publiczny z konfiguracji (`[updates] public_key`, base64 albo plik `.pub`); **komentarz zaufany musi zawierać
  `version:<wersja>`** (`require_version_tag`, domyślnie tak) — wiąże podpis z wersją z manifestu (ochrona przed
  podsunięciem starszej paczki). Manifest wydań: `{ schema, channel, releases: [{ version, url (https), sha256,
  minisign, notes, min_previous }] }`; `select_update` = najnowsze osiągalne wprost, niewycofane.
- **Launcher**: binarium `alfa` w pakiecie `updater-impl` (`[[bin]]`; osobny pakiet nie może zależeć od `-impl` —
  `scripts/check-deps.sh`), `#![windows_subsystem = "windows"]`, katalog instalacji = katalog `alfa.exe` z `versions\`
  albo `%LOCALAPPDATA%\Alfa`; argumenty przekazywane bez zmian (`OsString`, bez powłoki); błędy do `launcher.log`
  (≤ 64 KiB) — okno błędu przez `platform-windows` później. Launcher zostaje na czas okna obserwacji (15 s), potem kończy.
- **Do zrobienia (F3)**: pobieranie i rozpakowanie wydań, aktualizacja samego launchera, rollback z `watchdog`,
  okno błędu launchera, pomiar RAM/startu launchera na Windows.

## Zmiany po implementacji (F3, `updater-*`, `app-updates`, instalator, 2026-10-03)
- **Kontrakt** (dodane): `Channel` (`stable`/`beta`, ustawienie `preview` = `beta`; stabilny odrzuca wersje
  przedpremierowe), `UpdateMode` (`auto`/`ask`/`manual`), `InstallIntent` + `check_install_allowed` (wersja ≤ bieżącej
  tylko jako jawny `UserRollback`), port async **`ReleaseFeed`** (`manifest(kanał)`, `download(wydanie, plik, postęp)` —
  wznawianie od rozmiaru pliku; anulowanie = porzucenie przyszłości), `validate_manifest` (schemat 1, kanał, ≤ 64
  wydania), adresy: tylko `https://` (`http://` wyłącznie na pętli zwrotnej w testach), adres paczki bezwzględny
  `https://` albo **względny wobec katalogu manifestów** (`alfa-<ver>-x64.zip`), notatki ≤ 64 KiB; reguły paczki
  `validate_package_path` + `PackageLimits` (4096 wpisów, 512 MiB/wpis, 1,5 GiB razem, kompresja ≤ 200:1 dla > 1 MiB);
  `UpdateStatus`/`UpdatePhase` (stan dla UI), `UpdatesFile` (`updates.json`: ostatnie sprawdzenie, „Co nowego”
  pokazane), `AppExit::Unconfirmed` + `CrashPolicy::confirm_ms` (5 min), błędy `Network`, `Cancelled`, `Downgrade`,
  `UnsafePackage`, `NotConfigured`; zdarzenie `updater.launcher_replaced`.
- **Manifest** `<feed>/<kanał>.json`: `{ schema: 1, channel, releases: [{ version, url, sha256, minisign, notes,
  min_previous }] }`; wydawany przez `.github/workflows/release.yml` (szkic wydania; publikuje człowiek). Brak telemetrii:
  `GET` manifestu i paczki, stały `User-Agent: Alfa-Updater`, bez identyfikatorów i ciasteczek; przekierowania tylko
  na `https://` (≤ 5).
- **Paczka** (ZIP = katalog `versions\<ver>\`): `alfa-desktop.exe` i `version.json` (wymagane, wersja = manifest),
  `notes.md` („Co nowego” z podpisanej treści), opcjonalny `alfa.exe` (nowy launcher). Rozpakowanie do
  `versions\.staging-<ver>` (niewidoczny dla launchera) → `rename`; odrzucane: `..`, ścieżki bezwzględne, `\`, `:`
  (dyski, strumienie NTFS), nazwy urządzeń, kropka/spacja na końcu, dowiązania, duplikaty (bez wielkości liter),
  zip-bomb, więcej bajtów niż w nagłówku; po odrzuceniu nie zostaje nic w `versions\` ani w `staging\`.
- **Cykl (`UpdateService`, `updater-impl`)**: sprawdź → (tryb `auto` albo zgoda) pobierz do
  `staging\alfa-<ver>-<sha16>.zip.part` z **wznawianiem** (HTTP Range; do 3 automatycznych wznowień; po restarcie
  aplikacji ten sam plik) → SHA-256 + minisign z `version:<ver>` (zły podpis/suma → plik usunięty) → rozpakuj →
  `activate_staged`: aktywna = nowa (`pending`), poprzednia = **uruchomiona** wersja (nawet gdy wcześniej przygotowano
  inną, nieuruchomioną) → sprzątanie do 2 wersji, nigdy uruchomionej → `updater.staged`. Restart: aplikacja uruchamia
  `alfa.exe --alfa-restart` i kończy się; launcher czeka (≤ 30 s) na zwolnienie `running.lock` (`File::try_lock`,
  zwalniane przez system), potem startuje aktywną. Restart odmawia w trakcie zadania agentki albo rozmowy głosowej.
- **Automatyczny rollback**: launcher obserwuje nową wersję (`pending`) do `mark_good` — wyjście z błędem przed
  potwierdzeniem albo brak `mark_good` w 5 min (zawieszenie → launcher zamyka proces) = awaria → powrót do poprzedniej,
  nowa trafia do `bad` (nie wraca automatycznie). Szybkie awarie dobrej wersji — jak w F1 (2 w 15 s). Watchdog:
  `WatchdogSignal` (`UpdaterSignal`) przełącza na poprzednią o jeden krok i wycofuje porzuconą.
- **„Przywróć poprzednią wersję”** (użytkownik): `rollback_by` — aktywna ↔ poprzednia, porzucona wycofana; przed
  restartem anuluje przygotowaną aktualizację. Starsze wydanie z manifestu tylko przez `offer(ver)` +
  `download(UserRollback)`.
- **Launcher**: tryby tylko jako pierwszy argument — `--alfa-restart [arg…]`, `--alfa-installed <ver>` (instalator NSIS:
  `version.json`, przełączenie, sprzątanie), `--alfa-launcher-check` (samotest, kod 73). Własna aktualizacja: po
  `mark_good` wersji z `alfa.exe` w paczce → `alfa.exe.new` + `.sha256` (atomowo); przy następnym starcie: skrót →
  samotest → `rename` bieżącego na `alfa.exe.old` → `rename` nowego; `.old` usuwany przy kolejnym starcie; launcher
  z instalatora unieważnia przygotowany `.new`.
- **Aplikacja (`app-updates`)**: komendy `updates_*` (COMMANDS.md), zdarzenie `UpdateStatus`, ustawienia
  `updates.channel`/`updates.mode` (domyślnie „pytaj”)/`updates.whats_new`, sprawdzanie raz na dobę (pierwsze po 5 min,
  tryb `manual` — nigdy samo), `mark_good` po zdrowym starcie (przeniesione z `app-core`), „Co nowego” raz po
  aktualizacji (notatki z paczki), „O programie” (wersja, kanał, data kompilacji `ALFA_BUILD_DATE`, commit, licencje z
  `crates/app-updates/data/licenses.json` — `apps/desktop/scripts/gen-licenses.mjs`). Klucz publiczny i adres wydań są
  **wbudowane w wydanie** (`ALFA_UPDATE_PUBKEY`, `ALFA_UPDATE_FEED` przy kompilacji); bez nich aktualizacje wyłączone.
- **Instalator** (`tauri.conf.json` + nakładka `tauri.bundle.conf.json`, haki `windows/hooks.nsh`): NSIS per-user bez
  UAC do `%LOCALAPPDATA%\Alfa`, PL/EN, WebView2 przez bootstrapper (cichy, tylko gdy brak), bez instalacji starszej
  wersji z instalatora; po kopiowaniu: aplikacja → `versions\<ver>\alfa-desktop.exe`, launcher → `alfa.exe`
  (skrót Menu Start z AUMID, protokół `alfa://`, „Otwórz w Alfie” dla plików i folderów w HKCU wskazują launcher),
  `alfa.exe --alfa-installed <ver>`. Deinstalacja usuwa pliki programu; dane (rozmowy, pamięć, modele, WebView2,
  konfiguracja) tylko po zaznaczeniu „Usuń także dane Alfy”; `%USERPROFILE%\Alfa` i Menedżer poświadczeń — nigdy.
  Sidecary (`llama-server`, `whisper-server`, `piper`) nie są w instalatorze — pobierane przy pierwszym użyciu.
- **Testy**: lokalny serwer HTTP w teście + pary kluczy minisign z testu: dobra aktualizacja (pełny cykl), zły podpis,
  zła suma, obcy klucz, podpis innej wersji (downgrade przez podmianę etykiety), wersja ≤ bieżącej (tylko jawny
  rollback), przerwane pobieranie → wznowienie `Range`, anulowanie → wznowienie, serwer bez zakresów, 13 niebezpiecznych
  archiwów, kanał beta, sprzątanie, crash-loop → rollback, brak `mark_good` → rollback, watchdog, tryby launchera,
  zamiana launchera (atrapa i prawdziwy proces), `app-updates` (tryby, restart z blokadą, zdarzenia, „Co nowego”).
- **Do zrobienia**: okno błędu launchera (`platform-windows`), pomiar RAM/startu launchera na Windows, obsługa ścieżki
  z „Otwórz w Alfie” w aplikacji (dziś pokazuje okno; załączanie pliku — `shell-integration`), podpis Authenticode.

