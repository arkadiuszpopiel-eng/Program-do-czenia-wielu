# ADR 0007 — Instalacja: stały launcher, wersje side-by-side, stały folder WebView2

| Pole | Wartość |
|---|---|
| Status | Zaakceptowany (Snap Layouts, toasty z AUMID — spike (j) w F0) |
| Data | 2026-09-30 |
| Powiązane sekcje planu | §1.1 (pkt 1), §1.2 (Instalacja), §3.3 (`updater`), §7.3, §8.7, §12.2, §16.2 (F1, F3), §17 |

## Kontekst

Użytek osobisty: bez dystrybucji, bez usługi podpisywania kodu (własny certyfikat lokalny), aktualizacje z własnego repo. Potrzebne: rollback do ostatniej dobrej wersji (Watchdog, §12.2), stabilne integracje z Windows (skrót Menu Start, AUMID toastów, autostart, handler protokołu, „Wyślij do", skróty globalne) oraz helper `uiAccess`, który musi leżeć w `Program Files` i być podpisany. MSIX wirtualizuje rejestr i utrudnia usługę Brokera oraz helper.

## Decyzja

| Element | Ścieżka / mechanizm |
|---|---|
| Wersje aplikacji | `%LOCALAPPDATA%\Alfa\versions\<ver>\` — side-by-side, każda wersja kompletna |
| Launcher | **stały `%LOCALAPPDATA%\Alfa\alfa.exe`** — jedyna ścieżka, którą znają skrót w Menu Start, AUMID toastów, autostart, handler protokołu, „Wyślij do", skróty globalne |
| Dane WebView2 | **stały folder** poza katalogiem wersji (ciasteczka, cache, słownik pisowni PL przeżywają aktualizację) |
| Konfiguracja i dane | `%APPDATA%\Alfa\config\*.toml`; dane sesji i modele w `%LOCALAPPDATA%\Alfa\` (dokładny podział katalogów — F1) |
| Helper `uiAccess` | instalowany osobno w `Program Files`, podpisany własnym certyfikatem (import do Trusted Root), jednorazowy UAC |
| Usługa Brokera | osobne konto Windows, instalacja z UAC (ADR 3) |
| Aktualizacje | z własnego repo, podpis **minisign** własnym kluczem; pobranie nowej wersji do `versions\<ver>`, przełączenie launchera |
| Rollback | przełączenie launchera na poprzednią wersję (Watchdog po N awariach lub ręcznie) |
| Pakowanie | bez MSIX |

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| MSIX | wirtualizacja rejestru i systemu plików utrudnia usługę Brokera, helper `uiAccess` i named pipes; wymaga certyfikatu zaufanego przez system |
| Instalacja w miejscu (nadpisywanie plików) | brak atomowego rollbacku; Watchdog nie może wrócić do „ostatniej dobrej" |
| Skróty i AUMID wskazujące na `versions\<ver>\alfa.exe` | każda aktualizacja psuje skrót, toasty i handler protokołu (ryzyko z §17) |
| Folder WebView2 w katalogu wersji | utrata cache i słownika pisowni po każdej aktualizacji; wielokrotne foldery Chromium |
| Tauri updater z serwerem dystrybucyjnym | zbędny dla jednego użytkownika; własne repo + minisign wystarcza |

## Konsekwencje

- Moduł `updater` (jądro) obsługuje: listę wersji, pobranie, weryfikację podpisu, przełączenie, rollback, sprzątanie starych wersji.
- Migracje schematów konfiguracji i danych muszą działać w obu kierunkach na tyle, by rollback nie zniszczył danych (upcastery + testy migracji, `transfer`).
- Launcher jest maleńkim natywnym programem; jego zmiana jest rzadka (sam nie jest wersjonowany side-by-side).
- Spike (j) w F0 sprawdza: Snap Layouts z własnym paskiem tytułu, Mica, pisownię PL w WebView2, toasty z AUMID przez launcher.
- Bramka ludzka #10: klucz minisign i lokalny certyfikat podpisu.
- F1 wprowadza launcher i układ katalogów; aktualizacje i rollback dochodzą w F3.

## Jak cofnąć

- Przejście na instalację w miejscu jest możliwe (launcher wskazuje wtedy jedną wersję), ale traci rollback Watchdoga.
- MSIX można rozważyć dopiero, gdy Broker i helper `uiAccess` przestałyby wymagać rejestru i `Program Files` — nie w v1.
