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
- Format repo wydań (manifest JSON + zip; delta-update poza v1) — do ustalenia w SPEC v1.
- Aktualizacja samego launchera (self-replace) — do ustalenia w SPEC v1.

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
