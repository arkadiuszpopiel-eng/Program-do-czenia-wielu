# 25 — Do przeglądu właściciela po audycie z 14.10.2026

Audyt zewnętrzny (raport poza repozytorium: `../audyt-2026-10-14/RAPORT.md`) wskazał 17 pozycji. Wersja 0.0.51
naprawia je w zakresie, który wolno zmieniać bez przeglądu człowieka (AGENTS.md: Jądro, Broker, watchdog,
polityki bezpieczeństwa — tylko z przeglądem). Ten dokument zbiera to, co zostało, z propozycją.

## Co zrobiono w 0.0.51

| Pozycja | Stan | Gdzie |
| --- | --- | --- |
| A-01 test się nie kompilował | naprawione | `app-core/tests/model_routing.rs` |
| A-02 kontrakt poleceń doboru | naprawione; strona „Dobór modelu”, test w przeglądarce i w prawdziwej aplikacji | `routing_*`, `RoutingPage.svelte` |
| A-03 STOP WSZYSTKIEGO niepełne | naprawione (komunikat o zatrzymaniu niepełnym) | `app-core/src/commands/system.rs` |
| A-04 autonomia po opóźnionym starcie Brokera | naprawione (strażnik, poziomy sesji) | `app-core/src/commands/permissions.rs` |
| A-05 watchdog i nowy PID Brokera | naprawione w warstwie składającej (bez zmian w watchdogu) | `app-broker/src/kernel.rs` |
| A-06 cofanie częściowe | naprawione (ponowienie, rekordy `Restored`) | `undo-journal-contract` |
| A-07 analiza poleceń powłoki | klasyfikacja naprawiona; **izolacja procesu — do przeglądu** (niżej) | `tools-shell-contract` |
| A-08 zależności | naprawione | `Cargo.lock`, `pnpm-workspace.yaml` |
| A-09 verify ≠ CI | naprawione (bramki CI, wynik NIEKOMPLETNY, pokrycie łącza jako bramka) | `scripts/verify.mjs`, `verify-app.sh` |
| A-10 lint i format | naprawione | — |
| A-11 budżet pigułki | naprawione (7,9 KB / 8 KB) | `ui-kit/src/tooltips.ts` |
| A-12 widok Brokera | naprawione | `app-broker/src/status.rs`, `state/broker.svelte.ts` |
| A-13 tożsamość procesów pobocznych | naprawione (ścieżka + czas utworzenia) | `lib-sidecar-guard` |
| A-14 edycja propozycji pamięci | naprawione (edycja nie zatwierdza) | `memory-contract/src/engine/write.rs` |
| A-15 dokumentacja | częściowo: README, STATUS (dopisek), plan; **jedna macierz dowodów — do zrobienia** | `docs/STATUS.md` |
| A-16 rozmiary | naprawione (bramka `check-size.mjs`, jawne wyjątki) | `scripts/size-exceptions.json` |
| A-17 lock powłoki | naprawione i pilnowane przez `set-version.mjs` | `apps/desktop/src-tauri/Cargo.lock` |

## Do decyzji właściciela

### 1. Izolacja procesu poleceń powłoki (A-07, druga część)

Dziś polecenie agentki działa z integralnością **Medium** w Job Object (`tools-shell-impl/src/run.rs`,
`platform-windows-impl/src/process/exec.rs`). Job Object pilnuje drzewa procesów i limitów, ale nie ogranicza
dostępu do plików ani sieci — to robi dopiero Broker przez zgody i analizę polecenia. Analiza jest heurystyką;
po 0.0.51 widzi polecenia zagnieżdżone, zakodowane, `git clean`, `robocopy /MIR` i nadpisania, ale żadna
heurystyka nie zamknie tej klasy problemów.

Propozycja (zmiana w `platform-windows-impl` i polityce Brokera — wymaga przeglądu):

1. Polecenia bez zgody na zapis poza katalogiem roboczym — **AppContainer** z dostępem tylko do katalogu
   roboczego (capability `broadFileSystemAccess` wyłączona), bez sieci, gdy analiza nie zgłosiła hostów.
2. Polecenia z zapisem poza katalogiem roboczym — jak dziś (Medium), ale tylko po zgodzie w oknie Brokera
   z listą ścieżek; nieznany program albo cel nieustalony — zawsze zgoda.
3. Test: polecenie zagnieżdżone próbujące zapisać `%USERPROFILE%\x.txt` i odczytać `.ssh` w AppContainer
   kończy się odmową dostępu (bez zgody), a w katalogu roboczym działa.

### 2. Jedna macierz dowodów (A-15)

Zamiast tabel w `STATUS.md`: plik z wierszem na kryterium — kryterium, data, commit, sprzęt, polecenie,
artefakt (raport `target/verify/…`), ograniczenia. Generowany z raportów `verify.mjs`, nie pisany ręcznie.
Do ustalenia: czy przenosimy stare wpisy, czy zaczynamy od wersji 0.0.51.
