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

## Rozstrzygnięte (0.0.53, 2026-10-14 w nocy)

Właściciel przekazał decyzje: „wykonaj to tak, jak uważasz za najlepsze rozwiązanie dla programu i jego
funkcjonalności”. Rozstrzygnięcia:

1. **Izolacja poleceń powłoki — integralność Low zamiast AppContainer.** AppContainer odciąłby polecenia od
   profilu użytkownika (git nie przeczyta `~/.gitconfig`, cargo i npm swoich katalogów) — agentki straciłyby
   zwykłe narzędzia. Wybrano niską integralność: polecenie, które według analizy działa tylko w katalogu
   roboczym (bez sieci, instalacji i celów spoza katalogu), dostaje token Low, a katalog roboczy etykietę Low
   z dziedziczeniem. Proces czyta to co użytkownik, ale **zapis poza katalogiem roboczym odrzuca system**
   (No-Write-Up), także gdy analiza czegoś nie zauważyła. `TEMP` w `%USERPROFILE%\AppData\LocalLow\Alfa\tmp`.
   Polecenia sieciowe, instalacje i z celami spoza katalogu — jak dotąd: integralność Medium, zgoda Brokera.
   Kod: `platform-windows-impl/src/process/win/low.rs`, `tools-shell-impl/src/run.rs`; wyłącznik
   `tools.shell.low_integrity` (domyślnie włączony). Testy na prawdziwych procesach: `tests/exec_low.rs`
   (cmd i PowerShell: zapis w katalogu działa, obok — odmowa, odczyt systemu i `TEMP` działają). Skutek
   uboczny: katalog roboczy ma etykietę Low — inne procesy Low (rzadkie) też mogą w nim pisać.
2. **Macierz dowodów** — rejestr `docs/DOWODY.md` generowany z raportów `verify` (0.0.51+), stare tabele
   `STATUS.md` zostają jako historia, bez przenoszenia.
3. **Antigravity w rejestrze** — zmiana z 0.0.52 zatwierdzona zgodnie z wyborem właściciela; status szary,
   domyślnie wyłączony, ostrzeżenie na karcie. Test na prawdziwym `agy` z właścicielem 2026-10-15 o 10:00.
4. **Budżet Markdown** — bez zmiany budżetu i bez starszej ammonii: czyszczenie dużych odpowiedzi w kilku
   wątkach (grupy zbalansowanych bloków, wynik identyczny — test). 100 KB: 7,8 ms (było 22–24 ms).

## Archiwum: opis decyzji (stan przed rozstrzygnięciem)

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

### 3. Antigravity w rejestrze zgodności (0.0.52)

Na polecenie właściciela (2026-10-14: „Terminal Alfy i dodatkowo pełny most z ostrzeżeniem jako opcja łatwego
włączenia”) wpis `agy-antigravity` w `docs/compliance/compliance-registry.json` zmienił listy: zamiast zakazu
„użycie agy z Alfy” — dozwolone `agy -p` na własnym koncie po świadomym włączeniu, logowanie wyłącznie przez
człowieka, sprawdzanie stanu bez danych logowania; zakazane: włączanie domyślne lub bez ostrzeżenia,
automatyczne zgody, harmonogram bez zgody, czytanie `~/.agy` i `~/.gemini`. Status zostaje **szary**,
`enabled_by_default: false`. Regulamin (pkt 6) nadal zabrania korzystania z usługi w produktach innych niż
Google — ryzyko konta ponosi właściciel; karta to mówi. Rejestr jest polityką bezpieczeństwa — zmiana czeka
na podpis.

Kształt zdarzeń `stream-json` potwierdzony na prawdziwym `agy` 1.2.11 (2026-10-15, za zgodą właściciela,
jedno krótkie pytanie) — parser poprawiony w 0.0.56.

### 4. Budżet renderowania Markdown (ADR 0009)

Pomiar 0.0.51 na sprzęcie: dokument 100 KB 22–24 ms przy budżecie 20 ms. Przyczyna potwierdzona: ammonia 4.2.3
(4.2.0 dawała 16,5–17,3 ms). Do wyboru: zmiana budżetu, przypięcie starszej ammonii (ostrzeżenia bezpieczeństwa)
albo przyspieszenie czyszczenia po naszej stronie.

