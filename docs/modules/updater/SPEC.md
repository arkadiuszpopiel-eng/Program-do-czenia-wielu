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
