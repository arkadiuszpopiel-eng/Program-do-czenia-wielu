# Spike (j) — powłoka Windows: Snap Layouts, Mica, pisownia PL w WebView2, toasty z AUMID

**Cel (PLAN §14.2, §16.2 F0 (j); UI.md; ACCEPTANCE F0-15):** dla każdego z czterech punktów odpowiedzieć
„działa / działa z obejściem / nie działa" w Tauri 2 na Windows 11 i zapisać obejście do ADR. Testy ręczne na
**desktopie**; jeśli laptop ma inną wersję Windows 11, powtórz tam Snap Layouts i Mica (zależą od kompozytora).

Wymaga powłoki „hello" z `f-ram-tauri/README.md` krok 1 oraz — dla punktów 1 i 2 — małych zmian w `apps/desktop`,
które robi sesja AI w worktree tego spike'u (gałąź `spike/j-powloka`, **nie merge'owana** do main; wynik trafia do ADR):

| Zmiana w kodzie (sesja AI) | Po co |
|---|---|
| `tauri.conf.json`: `decorations: false`; własny pasek tytułu w Svelte z `data-tauri-drag-region` i przyciskami min/max/zamknij | test 1 |
| obsługa `WM_NCHITTEST` zwracająca `HTMAXBUTTON` nad przyciskiem maksymalizacji (Tauri 2: `window.on_window_event`/`with_hwnd` + `windows-rs` subclass) — **wariant B**, gdy wariant A nie działa | test 1 |
| crate `window-vibrancy`: `apply_mica(&window, None)` + `transparent: true` w konfiguracji i przezroczyste tło w CSS | test 2 |
| `additionalBrowserArgs` / `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = --lang=pl-PL` i `<html lang="pl">`, `<textarea spellcheck="true">` | test 3 |
| brak — test 4 robimy z PowerShella, kod launchera powstaje w F1 | test 4 |

## Jak sprawdzać — punkt po punkcie

### 1. Snap Layouts przy własnym pasku tytułu

Co ma działać: po **najechaniu** myszą na przycisk maksymalizacji w naszym własnym pasku pokazuje się dymek Snap Layouts
(siatka układów); `Win+Z` pokazuje go też; przeciąganie okna za pasek działa; podwójne kliknięcie paska maksymalizuje.

Kroki:
1. Uruchom build z `decorations: false` i własnym paskiem (wariant A: zwykły przycisk HTML).
2. Najedź na przycisk maksymalizacji i **przytrzymaj kursor 1 s**. Zapisz: dymek pojawia się / nie.
3. Naciśnij `Win+Z` z aktywnym oknem — dymek nad przyciskiem / nie.
4. Przeciągnij okno do górnej krawędzi (snap) i do bocznej — działa / nie.
5. Jeśli 2 nie działa: wariant B (`WM_NCHITTEST → HTMAXBUTTON`), powtórz 2–4.
6. Sprawdź, że przycisk maksymalizacji **nadal reaguje na kliknięcie** (w wariancie B kliknięcie obsługuje system).

Znane od Windows 11: dymek Snap Layouts wymaga, by system „wiedział", gdzie jest przycisk maksymalizacji — to daje
`HTMAXBUTTON` z `WM_NCHITTEST` (tak robią przeglądarki i Terminal). Bez tego dymek się nie pokaże — wtedy wpis „obejście = wariant B".

### 2. Mica przez efekty okna

Co ma działać: pasek tytułu i tło paneli mają tło Mica (delikatnie prześwitująca tapeta), treść na nieprzezroczystym tle;
przy wyłączonej przezroczystości w Windows albo trybie oszczędzania baterii — jednolity kolor bez artefaktów.

Kroki:
1. Ustawienia → Personalizacja → Kolory → „Efekty przezroczystości" **włączone**. Uruchom build z `apply_mica`.
2. Przesuń okno nad kolorową tapetę — tło paska zmienia odcień / nie. Zrób zrzut (`Win+Shift+S`).
3. Przełącz motyw jasny ↔ ciemny (Ustawienia → Kolory) przy otwartym oknie — okno nadąża / nie.
4. Wyłącz „Efekty przezroczystości" — okno ma jednolite tło (nie czarne, nie białe z dziurami) / nie.
5. Laptop: włącz tryb oszczędzania baterii — jw.
6. Zmierz koszt: Menedżer zadań → GPU okna przy przesuwaniu (porównaj z build bez Mica) — wpisz procent.
7. Sprawdź, czy przy `transparent: true` **nie ma** przezroczystego prześwitu przez treść (błąd konfiguracji CSS) i czy
   okno nadal ma cień i ramkę do zmiany rozmiaru.

### 3. Pisownia PL w WebView2

Co ma działać: w polu tekstowym błędnie napisane polskie słowo jest podkreślone na czerwono, a prawy przycisk daje
polskie podpowiedzi; poprawne słowa z polskimi znakami **nie są** podkreślane.

Kroki:
1. Wpisz w composer/`<textarea>`: `Poszłem do sklepu i kupiłem chleb, mleko i jabłka.` — „Poszłem" ma być podkreślone,
   reszta nie.
2. Prawy przycisk na „Poszłem" — podpowiedź „Poszedłem" / brak menu / menu angielskie.
3. Wpisz `This is an English sentence.` — czy angielskie słowa są podkreślane (świadczy o tylko-PL) — zapisz.
4. Sprawdź, jaki język ma słownik: `edge://settings/languages` nie jest dostępne w WebView2, więc oceń po zachowaniu.
   Jeśli podkreśla po angielsku: dodaj `--lang=pl-PL` (patrz tabela zmian) i powtórz. Jeśli nadal nie: WebView2 pobiera
   słowniki dynamicznie — sprawdź, czy w `%LOCALAPPDATA%\<identyfikator-aplikacji>\EBWebView\Dictionaries` pojawił się
   plik `pl-PL-*.bdic` (może wymagać połączenia z internetem i kilku minut).
5. Wpisz, czy da się mieć **PL i EN naraz** (WebView2 obsługuje wiele języków sprawdzania, jeśli oba są na liście języków
   przeglądarki: `--lang=pl-PL --accept-lang=pl-PL,en-US` — do sprawdzenia).

### 4. Toasty z AUMID przez launcher

Co ma działać: powiadomienie Windows (toast) pokazuje się z nazwą i ikoną „Alfa", trafia do Centrum powiadomień,
kliknięcie w toast można obsłużyć; wymaga zarejestrowanego **AppUserModelID (AUMID)**. W produkcie robi to launcher (F1);
w spike'u rejestrujemy AUMID ręcznie w rejestrze (wariant bez skrótu w menu Start) i wysyłamy toast z PowerShella.

Kroki (pwsh, jako zwykły użytkownik):

```powershell
$aumid = 'pl.alfa.desktop.spike'
$key = "HKCU:\Software\Classes\AppUserModelId\$aumid"
New-Item -Path $key -Force | Out-Null
Set-ItemProperty -Path $key -Name DisplayName -Value 'Alfa (spike)'
Set-ItemProperty -Path $key -Name IconUri -Value "$HOME\Alfa\apps\desktop\src-tauri\icons\128x128.png"

[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
$xml = @"
<toast><visual><binding template="ToastGeneric"><text>Alfa</text><text>Zadanie zakończone. Kliknij, aby otworzyć.</text></binding></visual></toast>
"@
$doc = New-Object Windows.Data.Xml.Dom.XmlDocument
$doc.LoadXml($xml)
$toast = New-Object Windows.UI.Notifications.ToastNotification $doc
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($aumid).Show($toast)
```

Uwaga: w **PowerShell 7** typy WinRT mogą być niedostępne (`Unable to find type`) — wtedy uruchom ten sam blok
w **Windows PowerShell 5.1** (`powershell.exe`). To nie zmienia wyniku testu (liczy się rejestracja AUMID).

1. Toast pojawił się z nazwą „Alfa (spike)" i ikoną / bez ikony / nie pojawił się.
2. Otwórz Centrum powiadomień (`Win+N`) — jest na liście pod „Alfa (spike)" / nie.
3. Ustawienia → System → Powiadomienia — „Alfa (spike)" jest na liście aplikacji / nie.
4. Powtórz z **drugim** AUMID zarejestrowanym przez skrót w menu Start (wariant produktowy z launchera; sesja AI
   przygotuje mały skrypt tworzący `.lnk` z `System.AppUserModel.ID`) — czy zachowanie jest takie samo.
5. Sprawdź „Nie przeszkadzać" (Focus Assist) włączone: toast nie pokazuje się, ale jest w Centrum / nie.
6. Sprzątanie: `Remove-Item -Path $key -Recurse`.

## Tabela wyników — `results/<maszyna>-<data>.md`

```markdown
# Spike (j) — <maszyna> — <RRRR-MM-DD>

Windows: <winver, build> | WebView2: <wersja> | Tauri: <wersja z Cargo.lock> | gałąź spike'u: spike/j-powloka@<commit>

| # | Sprawdzenie | Wynik (działa / obejście / nie działa) | Wariant / obejście | Uwagi, zrzuty (nazwy plików) |
|---|---|---|---|---|
| 1a | Snap Layouts: dymek po najechaniu (wariant A, zwykły przycisk) | | | |
| 1b | Snap Layouts: dymek po najechaniu (wariant B, HTMAXBUTTON) | | | |
| 1c | Win+Z, przeciąganie za pasek, dwuklik = maksymalizuj | | | |
| 2a | Mica: prześwit tapety, zmiana motywu | | | |
| 2b | Mica: fallback przy wyłączonej przezroczystości / baterii | | | |
| 2c | Mica: koszt GPU przy przesuwaniu [%] vs bez | | | |
| 3a | Pisownia PL: „Poszłem" podkreślone, podpowiedź „Poszedłem" | | | |
| 3b | Pisownia: EN też? PL+EN naraz? potrzebne `--lang`? | | | |
| 4a | Toast z AUMID z rejestru: widoczny, ikona, Centrum, lista w Ustawieniach | | | |
| 4b | Toast z AUMID przez skrót w menu Start (launcher) | | | |
| 4c | Toast przy „Nie przeszkadzać" | | | |

## Wnioski do ADR (1) / (7) i UI.md
```

Zrzuty ekranu (PNG) zapisz do `results/` z nazwą `<maszyna>-<data>-<nr>.png` (są małe, mogą iść do repo).
