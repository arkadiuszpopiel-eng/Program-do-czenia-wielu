# Macierz aplikacja × trasa (ACCEPTANCE F6-03)

**Status: szkic — wymaga akceptacji człowieka, potem zamrożenie hashem.** Kolumny „oczekiwane” to prognoza
model-recenzentki z dokumentacji technologii UI i kodu Alfy; **pomiar** (arkusz w §3) wykonuje spike (c) na VM
`alfa-f6-base` i desktopie właściciela. Kryterium F6-03: dla każdej aplikacji z listy wybrana **działająca** trasa,
a puste drzewo UIA **wykryte** (nie zgadywane przez model).

Kolejność tras (PLAN §7.1): **API/CLI/COM > UI Automation > wizja > syntetyczne wejście**; degradacja przy błędzie,
weryfikacja po każdej akcji GUI (F6-04). Ograniczenia nie do obejścia (PLAN §7.3): UAC i bezpieczny pulpit, ekran
blokady, Windows Hello, CAPTCHA, UIPI wobec okien podniesionych (tylko helper `uiAccess` — F6-07, jeszcze nie
istnieje), okna chronione przed przechwyceniem (czarne klatki).

## 1. Legenda

| Jakość UIA      | Znaczenie                                                                                                                    |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| **Pełne**       | elementy interaktywne mają rolę, nazwę i wzorce (Invoke, Value, Toggle, SelectionItem, ExpandCollapse, Text) — UIA wystarcza |
| **Częściowe**   | rama, menu i okna dialogowe tak; obszar treści ubogi, wirtualizowany albo dostępny dopiero po włączeniu dostępności          |
| **Puste**       | poza ramą okna brak węzłów (płótno, DirectX, GTK bez mostu, Java bez Access Bridge, sesja zdalna) → wizja + wejście          |
| **Podniesione** | okno o wyższej integralności (UIPI): odczyt i wejście z procesu Alfy odrzucane — tylko trasa API albo helper `uiAccess`      |
| **Zablokowane** | aplikacja chroniona albo na deny-liście: żadna trasa GUI nie jest dozwolona (strażnik celów, Broker, maskowanie zrzutów)     |

Narzędzia tras: **API/CLI/COM** — `fs_*`, `shell_run`, `office_*` (COM Word/Excel), `browser_*` (CDP przez potok,
profil Alfy), `system_*`, `net_*`, `media_*`, `clipboard_*`; **UIA** — `window_*`, `uia_tree`/`uia_find`/
`uia_read_text`/`uia_act`; **wizja** — `screen_capture`, `vision_ocr`, `vision_describe`, `browser_screenshot`;
**wejście** — `input_click`/`input_keys`/`input_type_text`/`input_scroll` (skróty Win+…, Alt+Tab i kill-switch
odrzucane).

## 2. Macierz (45 pozycji: 39 aplikacji i 6 klas okien specjalnych)

| #   | Aplikacja (obraz)                                                            | Technologia UI            | API / CLI / COM                                                          | UIA (oczekiwane) | Trasa wybrana                       | Zapasowa                            | Uwagi i ryzyka                                                                                 | Zadania F6          |
| --- | ---------------------------------------------------------------------------- | ------------------------- | ------------------------------------------------------------------------ | ---------------- | ----------------------------------- | ----------------------------------- | ---------------------------------------------------------------------------------------------- | ------------------- |
| 1   | Eksplorator plików (`explorer.exe`)                                          | Win32 + XAML (Win11)      | `fs_*` (operacje na plikach), Shell COM w `shell_run`                    | Pełne            | API (pliki); UIA (widok, Kosz)      | wejście (Ctrl+Shift+1…8)            | lista wirtualizowana — widoczne tylko bieżące elementy; menu kontekstowe Win11 dwupoziomowe    | pl-08…pl-10, ap-06  |
| 2   | Notatnik (`Notepad.exe`, Store)                                              | WinUI/XAML + RichEdit     | `fs_*` (treść pliku)                                                     | Pełne            | API (pliki); UIA (dane tylko w GUI) | wejście                             | karty (TabItem); autozapis sesji — zamknięcie nie pyta o zapis                                 | ap-02…ap-06         |
| 3   | Kalkulator (`CalculatorApp.exe`)                                             | UWP/XAML                  | —                                                                        | Pełne            | UIA (AutomationId przycisków)       | wejście (klawiatura)                | ramka `ApplicationFrameWindow` — treść w procesie aplikacji (P2-01)                            | ap-01, ap-05, ap-08 |
| 4   | Paint (`mspaint.exe`, Store)                                                 | WinUI + płótno            | `media_convert` (skalowanie, formaty)                                    | Częściowe        | UIA (polecenia, okna dialogowe)     | API (`media_convert`); wizja        | płótno puste dla UIA — rysowanie poza zakresem v1                                              | ap-07               |
| 5   | Ustawienia (`SystemSettings.exe`)                                            | UWP/WinUI                 | część ustawień w HKCU przez `shell_run` (brak narzędzia zapisu rejestru) | Pełne            | UIA                                 | CLI (Set-ItemProperty HKCU); wizja  | strony ładowane leniwie; sekcje rozwijane (ExpandCollapse); nazwy po polsku                    | us-02, us-06, us-07 |
| 6   | Panel sterowania i aplety (`control.exe`, `*.cpl`)                           | Win32 (DirectUI, dialogi) | —                                                                        | Pełne            | UIA                                 | wejście                             | część apletów wymaga UAC (bezpieczny pulpit — nieautomatyzowalne)                              | us-02 (zapas)       |
| 7   | Menedżer zadań (`Taskmgr.exe`)                                               | WinUI 3                   | `system_processes`, `system_process_kill`                                | Podniesione      | API                                 | — (helper `uiAccess`)               | dla konta administratora startuje podniesiony (UIPI); procesy Alfy/Jądra chronione             | us-03               |
| 8   | Edytor rejestru (`regedit.exe`)                                              | Win32                     | `RegistryPort` (odczyt, MCP v1), `shell_run` (HKCU)                      | Podniesione      | API                                 | —                                   | `requireAdministrator`; klucze z sekretami na deny-liście portu                                | —                   |
| 9   | Podgląd zdarzeń (`mmc.exe eventvwr.msc`)                                     | MMC (Win32)               | `system_events` (Application, System; nigdy Security)                    | Częściowe        | API                                 | UIA                                 | lista wirtualna; dziennik Security tylko dla administratora                                    | us-05               |
| 10  | Usługi (`mmc.exe services.msc`)                                              | MMC (Win32)               | `system_services`, `system_service_control`                              | Częściowe        | API                                 | UIA (odczyt)                        | sterowanie wymaga administratora (dziś `PermissionDenied`, UAC na żądanie — SPEC tools-system) | us-12               |
| 11  | Microsoft Word (`WINWORD.EXE`)                                               | Win32 + własny silnik     | `office_read`, `office_edit` (COM, nowa wersja pliku)                    | Pełne            | COM                                 | UIA + wejście (formatowanie)        | COM bez formatowania i stylów → GUI; makra nigdy; MOTW = Protected View (edycja odrzucana)     | of-01…of-12         |
| 12  | Microsoft Excel (`EXCEL.EXE`)                                                | Win32 + siatka własna     | `office_read`, `office_edit` (COM; formuły z listy dozwolonej)           | Częściowe        | COM                                 | UIA (wstążka) + wejście             | siatka wirtualizowana — tylko widoczne komórki; nowa wersja z ręcznym przeliczaniem            | of-02, of-05…of-09  |
| 13  | Microsoft PowerPoint (`POWERPNT.EXE`)                                        | Win32                     | COM poza v1 (P2)                                                         | Pełne            | UIA                                 | wizja                               | treść slajdu częściowo (kształty jako grupy)                                                   | —                   |
| 14  | Outlook klasyczny (`OUTLOOK.EXE`)                                            | Win32                     | COM poza v1 (P2, Object Model Guard); poczta przez MCP                   | Pełne            | UIA (odczyt)                        | wizja                               | treść maili niezaufana (S03); wysyłka = egress (trifecta)                                      | —                   |
| 15  | Nowy Outlook (`olk.exe`), Teams (`ms-teams.exe`)                             | WebView2                  | Microsoft Graph przez MCP (klucz)                                        | Częściowe        | UIA                                 | wizja                               | dostępność Chromium włączana na żądanie — pierwsze `uia_tree` bywa ubogie                      | —                   |
| 16  | Microsoft Edge — przeglądarka Alfy                                           | Chromium                  | `browser_*` (CDP przez potok, profil Alfy, egress przez Brokera)         | —                | API (CDP)                           | `browser_screenshot` + wizja        | proces potomny Alfy = chroniony dla `gui.control`; deny-lista domen dostawców (S16)            | pr-01…pr-12         |
| 17  | Microsoft Edge — okno użytkownika (`msedge.exe`)                             | Chromium                  | — (profilu użytkownika Alfa nie używa)                                   | Częściowe        | UIA                                 | wizja + wejście                     | ciasteczka i hasła użytkownika poza zasięgiem (S15); kliknięcia na stronach = skutki sieciowe  | ap-09               |
| 18  | Google Chrome (`chrome.exe`)                                                 | Chromium                  | `browser_*` z `kind = "chrome"` (profil Alfy)                            | Częściowe        | API (CDP)                           | UIA, wizja                          | jak Edge; dostępność na żądanie                                                                | —                   |
| 19  | Mozilla Firefox (`firefox.exe`)                                              | Gecko                     | brak (WebDriver BiDi poza v1)                                            | Częściowe        | UIA                                 | wizja + wejście                     | IAccessible2/MSAA → UIA przez proxy, role bywają ogólne                                        | —                   |
| 20  | Przeglądarka PDF w Edge                                                      | Chromium (PDFium)         | —                                                                        | Częściowe        | UIA (warstwa tekstu)                | wizja (`vision_ocr`)                | PDF bez warstwy tekstu (skan) — tylko OCR                                                      | ap-11               |
| 21  | Adobe Acrobat Reader (`Acrobat.exe`)                                         | Win32 + własny silnik     | brak narzędzia PDF w v1                                                  | Częściowe        | UIA                                 | wizja (`vision_ocr`)                | tryb chroniony; dokumenty z tagami czytelniejsze                                               | —                   |
| 22  | Zdjęcia (`Photos.exe`)                                                       | WinUI 3                   | `media_info`, `media_convert`, `vision_*` (plik)                         | Pełne            | API (plik)                          | UIA + wizja                         | obraz to piksele — treść przez `vision_ocr`/`vision_describe`                                  | ap-10               |
| 23  | Narzędzie Wycinanie (`SnippingTool.exe`)                                     | WinUI                     | `screen_capture` (maskowanie)                                            | Pełne            | API (zrzut Alfy)                    | UIA                                 | nakładka wycinania nie jest potrzebna agentce                                                  | —                   |
| 24  | Microsoft Store (`WinStore.App.exe`)                                         | WinUI                     | winget (v2)                                                              | Pełne            | UIA                                 | wizja                               | instalacja = zgoda (L2+, `install`); konto Microsoft właściciela                               | —                   |
| 25  | Windows Terminal (`WindowsTerminal.exe`)                                     | WinUI + DirectX (tekst)   | `shell_run` (ConPTY, Job Object, migawka)                                | Pełne            | API                                 | UIA (TextPattern bufora)            | terminal Alfy (logowanie do CLI) chroniony — tylko użytkownik                                  | —                   |
| 26  | Konsola (`conhost.exe`: cmd, PowerShell)                                     | Win32                     | `shell_run`                                                              | Pełne            | API                                 | UIA                                 | sterowanie cudzą konsolą wejściem = ryzyko pomyłki okna — tylko z weryfikacją                  | —                   |
| 27  | Visual Studio Code (`Code.exe`)                                              | Electron                  | CLI `code` (otwieranie, rozszerzenia), pliki przez `fs_*`                | Częściowe        | API/CLI                             | UIA, wizja                          | edytor czytelny dla UIA dopiero z `editor.accessibilitySupport`                                | —                   |
| 28  | Notepad++ (`notepad++.exe`)                                                  | Win32 + Scintilla         | pliki przez `fs_*`                                                       | Częściowe        | API                                 | UIA (menu) + wejście                | Scintilla bez TextPattern — treść tylko przez plik                                             | —                   |
| 29  | 7-Zip (`7zFM.exe`)                                                           | Win32                     | CLI `7z.exe` w `shell_run`                                               | Pełne            | CLI                                 | UIA                                 | archiwa z internetu: MOTW, ścieżki w archiwum (zip-slip) sprawdzać po rozpakowaniu             | pl-04, pl-05 (CLI)  |
| 30  | LibreOffice (`soffice.exe`)                                                  | VCL                       | CLI `soffice --headless --convert-to`                                    | Częściowe        | CLI                                 | UIA                                 | mostek IAccessible2; makra Basic — nigdy                                                       | —                   |
| 31  | VLC (`vlc.exe`)                                                              | Qt                        | `media_*` (odtwarzanie przez Alfę), CLI `vlc.exe`                        | Częściowe        | API                                 | UIA (menu), wizja + wejście         | interfejs HTTP VLC to `localhost` — odrzucany przez `lib-netguard`                             | us-09…us-11 (API)   |
| 32  | Spotify (`Spotify.exe`)                                                      | CEF                       | Web API (klucz, poza v1)                                                 | Puste            | wizja + wejście                     | —                                   | konto właściciela; odtwarzanie koliduje z głośnikiem (zasób wyłączny)                          | —                   |
| 33  | Discord, Slack (Electron)                                                    | Electron                  | API botów (klucz, poza v1)                                               | Częściowe        | UIA                                 | wizja                               | treść niezaufana; wysłanie wiadomości = egress (trifecta)                                      | —                   |
| 34  | WhatsApp (`WhatsApp.exe`)                                                    | WinUI 3                   | —                                                                        | Pełne            | UIA                                 | wizja                               | dane prywatne + egress — każda wysyłka przez zgodę                                             | —                   |
| 35  | Zoom (`Zoom.exe`)                                                            | Qt + własne               | —                                                                        | Częściowe        | UIA                                 | wizja + wejście                     | kamera/mikrofon to zasoby wyłączne Alfy (scheduler)                                            | —                   |
| 36  | OBS Studio (`obs64.exe`)                                                     | Qt                        | obs-websocket (`localhost` — odrzucany)                                  | Częściowe        | UIA                                 | wizja + wejście                     | nagrywanie ekranu — prywatność (S26)                                                           | —                   |
| 37  | GIMP (`gimp-2.10.exe`)                                                       | GTK                       | CLI wsadowe (Script-Fu), `media_convert`                                 | Puste            | CLI / API                           | wizja + wejście                     | GTK na Windows bez mostu UIA                                                                   | —                   |
| 38  | IntelliJ IDEA, Android Studio                                                | Java Swing                | CLI narzędzi                                                             | Puste            | wizja + wejście                     | UIA po włączeniu Java Access Bridge | Access Bridge domyślnie wyłączony — włącza właściciel                                          | —                   |
| 39  | Total Commander (`TOTALCMD64.EXE`)                                           | Delphi VCL                | parametry CLI                                                            | Częściowe        | wizja + wejście                     | UIA (menu)                          | listy plików to kontrolki własne                                                               | —                   |
| 40  | Gry i aplikacje pełnoekranowe (DirectX, Steam)                               | DirectX, CEF              | —                                                                        | Puste            | — (poza zakresem)                   | —                                   | wyłączny pełny ekran → czarne klatki; tryb gry wstrzymuje tło                                  | —                   |
| 41  | Pulpit zdalny, Citrix (`mstsc.exe`, `wfica32.exe`)                           | bitmapa sesji zdalnej     | —                                                                        | Puste            | wizja + wejście tylko za zgodą      | —                                   | wejście trafia do **innego** systemu — proponowana odmowa domyślna (do decyzji)                | —                   |
| 42  | Menedżery haseł (KeePass, KeePassXC, 1Password, Bitwarden, Dashlane, Enpass) | różne                     | —                                                                        | Zablokowane¹     | —                                   | —                                   | maskowane w zrzutach i OCR; UIA, wejście i okna chronione (`SENSITIVE_APPS`, fala 5), §4.3     | —                   |
| 43  | Aplikacje dostawców planów (`claude.exe`, `chatgpt.exe`, `codex.exe`…)       | Electron                  | —                                                                        | Zablokowane      | —                                   | —                                   | `PROVIDER_APPS` → `KernelRule::ProviderWebUi` (SR-09), maskowanie zrzutów (S16)                | pr-11 (domeny)      |
| 44  | Okna Alfy, Broker-UI, watchdog, helper                                       | WebView2, Win32           | —                                                                        | Zablokowane      | —                                   | —                                   | strażnik celów (drzewo procesów, `GA_ROOT`/`GA_ROOTOWNER`, UWP — P2-01), F6-06                 | ap-12, us-12        |
| 45  | UAC (`consent.exe`), ekran blokady, Windows Hello                            | bezpieczny pulpit         | —                                                                        | Zablokowane      | —                                   | —                                   | nieautomatyzowalne z założenia (PLAN §7.3); `CredentialUIBroker` maskowany                     | —                   |

¹ (Fala 5: przypis nieaktualny — blokada obejmuje też UIA, wejście i operacje na oknach; patrz §4.3.)

## 3. Arkusz pomiaru (spike c) — wypełnia wykonawczyni pomiaru

Protokół dla każdej pozycji 1–41 (40 — tylko sprawdzenie czarnej klatki): okno referencyjne (stan po świeżym starcie, 1920×1080, 100 %), `uia_tree`
(`max_depth` 12, `max_nodes` 400) — liczba węzłów, liczba węzłów interaktywnych **w obszarze klienta** (poza
`TitleBar` i jego potomkami), flaga `sparse`; `screen_capture` okna — czarna klatka tak/nie; jedna akcja
referencyjna na trasie wybranej i na zapasowej (np. Kalkulator: „1+1=”, Word: wpisanie słowa) z krokiem
`tool.gui.verify`. Dla 42–45: próba odczytu i jednej akcji — oczekiwana odmowa przed Brokerem albo
`broker.kernel_block`, zero skutków.

| #   | Węzły (wszystkie) | Węzły interaktywne (klient) | `sparse` | Czarna klatka | Akcja referencyjna | Trasa wybrana działa | Zapasowa działa | Uwagi |
| --- | ----------------- | --------------------------- | -------- | ------------- | ------------------ | -------------------- | --------------- | ----- |
| 1   |                   |                             |          |               |                    |                      |                 |       |
| …   |                   |                             |          |               |                    |                      |                 |       |
| 45  |                   |                             |          |               |                    |                      |                 |       |

Wynik F6-03: ✅ dla pozycji, gdy trasa wybrana albo zapasowa działa i jest zgodna z kolejnością PLAN §7.1;
pozycje „Puste” zaliczają się tylko, gdy narzędzie **samo** zgłosiło `sparse` (albo inny sygnał trasy wizji), a
nie gdy model przełączył się z własnej inicjatywy.

## 4. Ustalenia z przeglądu kodu (do potwierdzenia pomiarem)

### 4.1 Próg „ubogiego” drzewa łapie tylko okna bez ramy

**Fala 5 — zrobione:** `UiaTree::client_nodes()` pomija węzeł okna, poddrzewo `TitleBar` i `SystemMenuBar`;
`sparse` = co najwyżej `SPARSE_TREE_NODES` (5) węzłów klienta (próg bez zmian — rozstrzygnie pomiar §3). Testy:
`platform-fake/tests/wave5.rs`, `tools-uia-impl/tests/wave5.rs`. Opis pierwotny:

`platform-contract::UiaTree::is_sparse` = `nodes.len() <= SPARSE_TREE_NODES` (5), a drzewo z
`platform-windows-gui-impl` (`ControlViewWalker`) zawiera węzeł okna i pasek tytułu z przyciskami (zwykle ≥ 6
węzłów). Okno, którego obszar klienta jest jedną nieprzezroczystą płaszczyzną (płótno, GTK, Java bez Access
Bridge, sesja zdalna), może więc dostać `sparse = false`, a opis narzędzia mówi modelowi, żeby przechodził na zrzut
tylko przy `sparse = true` — agentka traci kroki na bezużytecznym drzewie. Propozycja dla `tools-uia`/
`platform-contract` (zmiana kontraktu — osobna sesja): liczyć węzły interaktywne w obszarze klienta (poza
poddrzewem `TitleBar`), `sparse`, gdy ≤ 2; test na atrapie z ramą i pustym klientem. Pomiar w arkuszu §3 (kolumny
„wszystkie” i „interaktywne”) rozstrzyga próg.

### 4.2 Dostępność Chromium/Electron „na żądanie”

Chromium włącza pełne drzewo dostępności po wykryciu klienta UIA; pierwsze `uia_tree` po starcie bywa ubogie, a
kolejne pełne. Runner F6 i agentka powinni ponowić odczyt raz po ~500 ms przed przejściem na wizję (do wpisania w
podpowiedź narzędzia po pomiarze).

### 4.3 Menedżery haseł: maskowane w zrzutach, ale nie chronione przed UIA i wejściem

**Fala 5 — zrobione:** `platform_contract::SENSITIVE_APPS` (te same aplikacje co `DEFAULT_MASKED_APPS`) w
`TargetGuard::is_protected` — UIA, wejście i operacje na oknach odmawiają (`platform-fake/tests/wave5.rs`,
`tools-uia-impl/tests/wave5.rs`, `tools-input-impl/tests/wave5.rs`); reguła Brokera — propozycja `#[ignore]`.
Opis pierwotny:

`DEFAULT_MASKED_APPS` działa w `ScreenCapturePort` (zrzuty, OCR, opis obrazu), a strażnik celów aplikacji
(`app-gui::alfa_guard` = `TargetGuard::baseline()` + drzewo Alfy) ich nie obejmuje. `uia_tree`/`uia_read_text` na
oknie menedżera haseł mogą więc zwrócić nazwy wpisów, loginy i adresy, a pole hasła odsłoniętego przyciskiem
„pokaż” może przestać mieć `IsPassword`. PLAN §8.7 i THREAT_MODEL S26 wymagają dziś wykluczenia tylko ze zrzutów/OCR — rozszerzenie na
`gui.control` to decyzja człowieka (pozycja PT-F6-05b w `evals/F9/pentest.md`).

### 4.4 Okna podniesione

Bez helpera `uiAccess` (F6-07) pozycje „Podniesione” mają wyłącznie trasę API. Gdy okno administratora jest na
pierwszym planie, `input_*` i dyktowanie nie działają (PLAN §7.3) — agentka ma to zgłosić, a nie ponawiać.
