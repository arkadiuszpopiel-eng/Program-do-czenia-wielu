# Wydanie Alfy — instalator, aktualizacje, rollback

Skrót procedury; szczegóły modułu: `docs/modules/updater/SPEC.md`, ADR 0007.

## Raz (bramka ludzka #10)
1. **Klucz minisign** (u właściciela, nigdy w repo): `minisign -G -p alfa.pub -s alfa.key` (≥ 0.11 — podpisy
   „prehashed”). Sekrety repozytorium: `MINISIGN_KEY` = treść `alfa.key`, opcjonalnie `MINISIGN_PASSWORD`.
2. **Zmienne repozytorium**: `ALFA_UPDATE_PUBKEY` = druga linia `alfa.pub` (`RW…`) — wbudowywana w launcher i aplikację;
   `ALFA_UPDATE_FEED` = publiczny adres `https://` katalogu z manifestami `stable.json` / `beta.json` i paczkami
   (np. `https://github.com/<właściciel>/<repo-wydań>/releases/latest/download` — prywatne repo nie zadziała bez
   tokenu). Bez tych zmiennych wydanie działa, ale aktualizacje są wyłączone („O programie” to pokazuje).
3. (Opcjonalnie) certyfikat Authenticode — workflow go nie używa; podpis kodu dochodzi osobno.

## Wydanie
1. Podbij wersję w `apps/desktop/src-tauri/tauri.conf.json` i `apps/desktop/src-tauri/Cargo.toml` (ta sama semver;
   `-beta.N` = kanał testowy). Opcjonalnie „Co nowego”: `apps/desktop/release-notes/<wersja>.md` (inaczej lista commitów
   od poprzedniego tagu).
2. Tag `v<wersja>` (albo ręcznie: Actions → Release → kanał). Workflow `.github/workflows/release.yml`:
   licencje („O programie”) → launcher `alfa.exe` (`updater-impl`, klucz wbudowany) → `tauri build` z nakładką
   `tauri.bundle.conf.json` (instalator NSIS; procesy Jądra `alfa-broker`, `alfa-broker-ui`, `alfa-watchdog` z
   `app-safety` jako `externalBin`, przenoszone hakami do `versions\<ver>\`) → paczka `alfa-<wersja>-x64.zip`
   (`alfa-desktop.exe`, `alfa.exe`, `alfa-broker.exe`, `alfa-broker-ui.exe`, `alfa-watchdog.exe`, `version.json`, `notes.md`) → podpis minisign z komentarzem zaufanym `file:… version:<wersja> channel:<kanał>` →
   manifest `<kanał>.json` (SHA-256, podpis, notatki, adres względny) → **szkic** wydania z `SHA256SUMS.txt`.
3. Sprawdź szkic (instalacja na czystym koncie Windows, aktualizacja z poprzedniej wersji) i opublikuj ręcznie.

## Instalacja (NSIS, per-user, bez UAC)
`%LOCALAPPDATA%\Alfa\`: `alfa.exe` (stały launcher — skrót Menu Start z AUMID, protokół `alfa://`, „Otwórz w Alfie”),
`versions\<ver>\` (wersje obok siebie), `current.json`, `webview-data\` (stały folder WebView2), dane. WebView2: cichy
bootstrapper tylko, gdy brak runtime’u (Windows 11 ma go wbudowanego). Instalator starszej wersji nie zainstaluje się
bez odinstalowania bieżącej — do poprzedniej wersji wraca się w Ustawieniach. Aktualizując instalatorem, wybierz „Nie odinstalowuj”,
aby zachować poprzednią wersję do przywrócenia. Deinstalacja usuwa pliki programu; dane — tylko z zaznaczonym
„Usuń także dane Alfy” (`%USERPROFILE%\Alfa` i klucze w Menedżerze poświadczeń zostają zawsze).
Sidecary (`llama-server`, `whisper-server`, `piper`) **nie są w instalatorze ani w paczce aktualizacji** (rozmiar,
warianty GPU: Vulkan/CUDA/CPU). Miejsca (`AppPaths::sidecar`): najpierw `%LOCALAPPDATA%\Alfa\sidecars\<silnik>\<plik>.exe`
(pobrane przy pierwszym użyciu / w Ustawieniach — przeżywają aktualizacje i rollback), potem opcjonalnie
`versions\<ver>\sidecars\<silnik>\` (gdyby paczka je kiedyś zawierała). Brak sidecara = czytelny komunikat modułu
ze ścieżką docelową; modele GGUF pobiera `providers-local` (wznawianie + SHA-256).

## Aktualizacja i rollback (w aplikacji)
Ustawienia → Aktualizacje: tryb automatycznie / pytaj (domyślnie) / ręcznie, kanał stabilny / testowy. Pobieranie
wznawiane po przerwaniu; paczka bez ważnego podpisu, z inną sumą, z wersją ≤ bieżącej albo z niebezpiecznymi ścieżkami
nigdy nie jest instalowana. Nowa wersja działa od „Uruchom ponownie”: launcher obserwuje ją do zdrowego startu
(`mark_good` po 30 s) — awaria albo zawieszenie (5 min bez `mark_good`) = automatyczny powrót do poprzedniej.
„Przywróć poprzednią wersję” działa od ponownego uruchomienia; rozmowy, ustawienia i pliki zostają.
