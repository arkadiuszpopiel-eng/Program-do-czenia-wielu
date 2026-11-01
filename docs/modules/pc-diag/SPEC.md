# pc-diag — SPEC (v0)

## Cel
Dokładna diagnostyka komputera na żądanie właściciela i agentki: zapis parametrów z historią,
testy podzespołów z ocenami i porównaniem z poprzednim pomiarem, raport do pliku. Tylko pomiar —
moduł niczego nie naprawia ani nie zmienia w systemie.

## Kontrakt (`crates/pc-diag-contract`)
- `Snapshot` — system (wersja, model, BIOS, czas pracy), procesor (rdzenie/wątki, taktowanie,
  obciążenie, L2/L3, wirtualizacja), pamięć (zajętość, plik stronicowania, moduły: gniazdo,
  pojemność, szybkość, producent, numer części), dyski fizyczne (numer, model, końcówka numeru
  seryjnego, SSD/HDD, magistrala, kondycja, stan, liczniki SMART, woluminy, błędy z dziennika
  przypisane do dysku), woluminy (wolne miejsce), karty graficzne (sterownik, data, VRAM z
  rejestru), karty sieciowe (bez adresów IP/MAC), bateria (zużycie, cykle), temperatury,
  zdarzenia awarii z 7 dni (`EventKind`: nieoczekiwane wyłączenia, BSOD, WHEA, dysk, USB, awarie
  aplikacji; powtórzenia zwinięte), `gaps` — części nieodczytane z powodem.
- `CheckKind` (`cpu_single`, `cpu_multi`, `memory`, `disk:<litera>`, `network`, `integrity`),
  `CheckResult` (ocena `pass|warn|fail|skipped`, miary, uwagi), `CheckRun` (ocena zbiorcza).
- `rating` — progi dysków według rodzaju (NVMe/SSD/HDD/zewnętrzny), spójność z zapisu
  parametrów, porównanie z poprzednim wynikiem (spadek ≥ 30% / wzrost opóźnień ≥ 50% →
  ostrzeżenie); `bugcheck` — kod BSOD → nazwa i przyczyna; `report::markdown`.
- `DiagProbe` — odczyt parametrów (Windows: `platform-windows-diag-impl`, PowerShell/CIM,
  `-EncodedCommand`, bez okna, limit 90 s; części wypisywane od razu wierszami JSON — po limicie zapis
  powstaje z części odczytanych, brakujące są lukami „nie zdążyło w limicie” (`stream.rs`, 01.11);
  karty sieciowe z `Win32_NetworkAdapter` — `Get-NetAdapter` wisiał > 80 s; rodzaj „ethernet”/„wifi” tylko
  dla sprzętu na PCI/USB/SDIO, karty programowe (VirtualBox, TAP, Hyper-V) jako „other”; atrapa: `pc-diag-fake`).

## Testy (`crates/pc-diag-impl`)
- Procesor: mieszanie 64-bitowe przez 1,5 s, jeden wątek i wszystkie (skalowanie).
- Pamięć: do 512 MB (≤ ¼ wolnej) — wzorzec i negacja z pełną weryfikacją, przepustowość.
- Dysk: plik próbny 256 MB w katalogu Alfy (C:) albo `<litera>:\.alfa-diag` — zapis i odczyt z
  pominięciem pamięci podręcznej (`NO_BUFFERING | WRITE_THROUGH`), suma każdego bloku, 400
  odczytów losowych 4 KB; plik zawsze usuwany; wymaga wolnych 256 MB + 2 GB zapasu.
- Sieć: DNS i połączenie TCP:443 z trzema serwerami, bez wysyłania danych.
- Historia: `%LOCALAPPDATA%\Alfa\diagnostics\pc\{snapshots,runs}.jsonl` (200 wpisów).

## Aplikacja (`crates/app-diag`)
Polecenia `diag_state`, `diag_snapshot`, `diag_run`, `diag_cancel`, `diag_runs`, `diag_export`;
strona Ustawienia → Diagnostyka komputera; codzienny automatyczny zapis parametrów; narzędzie
agentek `system_diagnostics` przez Brokera (`gui.control(system-info.exe)`, wynik niezaufany) —
agentka nie uruchamia testów dysków.

## Niezmienniki
Bez danych osobistych w zapisie (adresy sieci, pełne numery seryjne). Brak odczytu = `null`
i wpis w `gaps`, nigdy zgadywanie. Testy nie zostawiają plików. Jeden przebieg naraz.
