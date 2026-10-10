# 06 — Linux

Linux jest czwartą platformą w kolejności (etap G). Rdzeń logiki już dziś buduje się i przechodzi testy na
Ubuntu w CI, ale **żadna** funkcja systemowa nie ma implementacji dla Linuksa — crate'y `platform-windows-*`
kompilują się tam z zaślepkami zwracającymi „nieobsługiwane”. Ten dokument opisuje, co trzeba zbudować.

Cel: Ubuntu LTS i Fedora (bieżące wydania), środowiska GNOME i KDE na Waylandzie; X11 w miarę możliwości.

## Porty do zaimplementowania

| Port                                            | Windows dziś                       | Linux — propozycja                                                 | Trudność                             |
| ----------------------------------------------- | ---------------------------------- | ------------------------------------------------------------------ | ------------------------------------ |
| `FsPort` (z koszem)                             | API powłoki                        | specyfikacja kosza freedesktop                                     | mała                                 |
| `ProcessPort`, `ExecPort`                       | Job Objects                        | grupy procesów / cgroups v2 użytkownika; dekodowanie wyjścia UTF-8 | średnia                              |
| `ClipboardPort`                                 | Win32                              | portale / `wl-clipboard`; na Waylandzie dostęp w tle ograniczony   | średnia                              |
| `HotkeyPort`                                    | hak klawiatury                     | portal `GlobalShortcuts` (zależny od środowiska)                   | średnia — nie wszędzie dostępny      |
| `TrayPort`                                      | zasobnik                           | StatusNotifierItem                                                 | mała                                 |
| `SecretStore`, `KeyVault`                       | Credential Manager                 | Secret Service (GNOME Keyring, KWallet) przez D-Bus                | mała                                 |
| `AudioIo`                                       | WASAPI                             | PipeWire                                                           | średnia; AEC już jest w czystym Rust |
| `PseudoConsolePort`                             | ConPTY                             | PTY                                                                | mała                                 |
| `SecurePipePort`, tożsamość procesu             | potok z DACL, SID                  | gniazdo Unix 0600 + `SO_PEERCRED`                                  | średnia — obszar Jądra               |
| `ServiceHostPort`                               | usługa Windows                     | usługa użytkownika systemd                                         | średnia — obszar Jądra               |
| `ApprovalSurfacePort`                           | okno na podwyższonej integralności | **brak prostego odpowiednika** — patrz niżej                       | duża                                 |
| `ScreenCapturePort`                             | API Windows                        | portal `ScreenCast` (każdorazowa zgoda systemu)                    | średnia                              |
| `UiaPort`, `InputPort`                          | UI Automation, `SendInput`         | AT-SPI; wstrzykiwanie wejścia przez portal `RemoteDesktop`         | duża — badawcze                      |
| `IdlePort`, `PowerPort`, `SessionPort`, sygnały | Win32                              | logind / portale                                                   | mała                                 |
| `OfficePort`, `RegistryPort`                    | COM, rejestr                       | **bez odpowiednika**                                               | nie planujemy                        |
| `OcrPort`                                       | OCR systemu                        | silnik dołączony (do wyboru)                                       | średnia                              |

## Największe ryzyka

### Okno zgód i dowód fizycznego wejścia

Na Windows okno Brokera działa na wyższym poziomie integralności, a system gwarantuje, że inne programy nie
wstrzykną do niego kliknięć (UIPI). Linux nie ma odpowiednika: na X11 każdy program może wysyłać zdarzenia do
dowolnego okna; Wayland izoluje okna lepiej, ale nie daje aplikacji dowodu, że wejście pochodzi z urządzenia.

Opcje do zbadania (zadanie G.4, wynik w ADR):

1. Zgody wyłącznie przez urządzenie sparowane (ADR 0022) — na Linuksie telefon staje się oknem zgód.
2. Okno zgód jako osobny proces innego użytkownika systemowego + Wayland; X11 nieobsługiwany dla zgód.
3. Uwierzytelnienie systemowe (polkit) przy zgodach wysokiego ryzyka.

Rekomendacja wstępna: opcja 1 jako pewna podstawa, opcja 2 jako wygoda. Do decyzji właściciela po badaniu.

### Analizator poleceń powłoki

Dzisiejsze reguły znają tylko `cmd` i PowerShell. Bash ma inną składnię (potoki, podstawienia, `eval`, aliasy,
heredoc). Analizator POSIX to nowa polityka bezpieczeństwa — pisana od zera, z własnym zestawem ataków,
pod przeglądem właściciela (G.2). Do tego czasu narzędzie powłoki na Linuksie jest wyłączone w mapie zdolności.

### Listy zakazów i ścieżki

Listy chronionych miejsc (profil przeglądarek, klucze SSH, `~/.claude`, katalogi systemowe) trzeba napisać
dla Linuksa osobno: `~/.ssh`, `~/.gnupg`, `~/.config`, `/etc`, `/proc`, `/sys`. Semantyka ścieżek: wielkość
liter ma znaczenie, dowiązania symboliczne są powszechne — `PathScope` musi je rozwijać przed porównaniem (B.7).

### WebKitGTK

Tauri na Linuksie używa WebKitGTK zamiast WebView2. Skutki: inne błędy renderowania, udokumentowane problemy
z kartami NVIDIA (DMABUF), inna wydajność. Interfejs trzeba przejść testami E2E na WebKitGTK; test dymny
na karcie NVIDIA trafia do CI (G.8).

## Silniki i modele

`llama.cpp` i `whisper.cpp` mają wydania dla Linuksa (CUDA, Vulkan, CPU). Potrzebny jest wymiar
system / architektura w katalogu silników (B.6) i przypięte sumy kontrolne dla każdego archiwum.
Modele GGUF są te same.

## Pakowanie

| Format   | Kolejność             | Uwagi                                                                                               |
| -------- | --------------------- | --------------------------------------------------------------------------------------------------- |
| deb, rpm | 1                     | korzysta z WebKitGTK i sterowników hosta — najmniej tarcia dla CUDA                                 |
| Flatpak  | 2                     | niesie własne środowisko; wymaga uprawnień do karty, dźwięku i plików; skróty i ekran przez portale |
| AppImage | 3, w miarę możliwości | bywa kruchy przy niezgodności WebKitGTK z hostem                                                    |

Aktualizacje: mechanizm side-by-side z launcherem (ADR 0007) jest pomyślany pod Windows; na Linuksie
aktualizuje menedżer pakietów albo Flatpak — `updater` dostaje tryb „tylko powiadom”.

## Mapa zdolności na start

| Działa od pierwszego wydania                                                                                                                    | Później                                                                                     | Nie planujemy             |
| ----------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ------------------------- |
| rozmowa (lokalnie i chmura), sesje, pamięć, wyszukiwanie, pliki z „Cofnij”, sieć, MCP, wtyczki, import/eksport `.alfa`, dostęp zdalny jako host | głos (G.5), powłoka (G.2), skróty globalne, zrzuty ekranu, sterowanie komputerem (badawcze) | Office przez COM, rejestr |

## Kryteria akceptacji (szkic; progi ustala właściciel)

- `ACC-V6-linux-01`: scenariusz MVP 6/6 na Ubuntu LTS z kartą NVIDIA.
- `ACC-V6-linux-02`: `cargo test --workspace` na Ubuntu bez pomijania testów platformy (dziś zaślepki).
- `ACC-V6-linux-03`: zestaw ataków na analizator POSIX — zero udanych.
- `ACC-V6-linux-04`: funkcje spoza mapy zdolności są ukryte w interfejsie (test E2E).
- `ACC-V6-linux-05`: sekrety wyłącznie w Secret Service; brak usługi = czytelna odmowa, nie zapis do pliku.
