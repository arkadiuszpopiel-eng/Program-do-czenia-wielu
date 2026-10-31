# Raport z 31.10: system automatycznego testowania przez AI, ścieżki względne, wersja 0.0.79

Właściciel 31.10 polecił: „kontynuuj kolejne etapy pracy! i ważne: opracuj system pełnego automatycznego
testowania przez AI każdej funkcji obecnej i przyszłej, jak będzie w programie, z pełnymi uprawnieniami dla
testów przez AI”. Ten raport mówi, co zrobiłam, gdzie to zobaczyć i czego nie sprawdziłam. Opis samego systemu:
[45-testy-ai.md](45-testy-ai.md).

## 1. Polecenia właściciela — punkt po punkcie

| Polecenie                                        | Co zrobiłam                                                                                                                                                                                                 |
| ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| System testowania każdej funkcji obecnej         | katalog 601 funkcji wyliczany z kodu (6 rodzajów), przy każdej — czym jest przetestowana; [`docs/features/KATALOG.md`](../features/KATALOG.md)                                                            |
| … i przyszłej                                    | bramka „nowa funkcja bez testu” w `verify szybki` i `verify wersja`; przeglądy ogólne palety i ustawień obejmują pozycje dodane później; nowe narzędzie agentki zatrzymuje test spisu narzędzi              |
| Pełne uprawnienia dla testów przez AI            | instancja testowa startuje z zapamiętanym poziomem Brokera z własnego profilu (`ALFA_TEST_AUTONOMY=L4`) — zmiana w Brokerze, więc **na gałęzi do przeglądu** `do-przegladu-testy-pelny-dostep` (§3)      |
| Kontynuuj kolejne etapy                          | ścieżki względne w wynikach narzędzi plikowych (z planu raportu 44, C2.3); spłata długu katalogu (§2)                                                                                                    |

## 2. Spłata długu i znaleziska

Stan po spłacie: **601 funkcji, 595 z testem (99 %), dług 31 → 6**.

| Spłacone                                        | Testy                                                                                                                       |
| ----------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| 17 poleceń rdzenia bez testu                    | `crates/app-core/tests/catalog_debt_*.rs` — 27 testów zachowania (projekty, arena, gałęzie, karty, nauka pamięci, cennik, ustawienia firmowe, kanwa, schowek-atrapa, aktualizacje bez sieci, odsłuch głosu, umiejętności) |
| Narzędzie `system_software`                     | `crates/tools-system-impl/tests/software.rs` — filtr rodzaju, pamięć wyniku i `refresh`, brak wykrywania → „niedostępne”     |
| Narzędzie `system_diagnostics`                  | istniejący test `app-diag` sprawdza teraz nazwę, pod którą zna je model                                                     |
| 5 zdarzeń stanu okna                            | `state/__tests__/events-state.test.ts` (obsada, koszty, stan systemu, Broker, raport Marszałka)                             |
| Klawisze pola pisania (5)                       | `e2e/composer-keys.spec.ts`                                                                                                 |

Zostały: Spacja „przytrzymaj, aby mówić”, Esc w rozmowie, wklejanie bez formatu (zachowanie samego pola WebView2)
oraz trzy klawisze globalne (wyłącznik awaryjny, dyktowanie, czytanie zaznaczenia) — rejestruje je powłoka
w systemie, test wymaga naciśnięcia klawisza poza oknem.

Znaleziska — katalog wskazał funkcje zadeklarowane, których nic nie obsługuje:

- **Spacja „przytrzymaj, aby mówić” (`voice.ptt`) nic nie robi w oknie.** Skrót jest w rejestrze i na stronie
  Skróty, rdzeń ma polecenie `voice_ptt`, ale żaden kod interfejsu go nie wywołuje. Do decyzji: podłączyć
  (Spacja poza polem pisania, przytrzymanie) albo usunąć z listy skrótów.
- **Esc w rozmowie (`conversation.escape`)** zamyka tylko listę podpowiedzi w polu pisania; opis skrótu sugeruje
  więcej (zatrzymanie, wyjście z edycji). Do sprawdzenia przy przeglądzie skrótów.

## 3. Pełne uprawnienia instancji testowej

Instancja testowa (`scripts/verify-app.sh`, profil tymczasowy w `target/`) dostaje plik zapamiętanego poziomu
Brokera przed startem (`scripts/trial/test-autonomy.sh`; odmawia ścieżek spoza `target/`, nigdy nie dotyka
profilu właściciela ani rejestru). Broker w procesie czyta ten plik dopiero po zmianie na gałęzi
`do-przegladu-testy-pelny-dostep` (`crates/app-broker/src/inproc.rs`, test
`standing_level_from_the_broker_dir_is_restored` zielony, clippy i fmt czyste). Do czasu przeglądu i scalenia
plik w profilu testowym nic nie zmienia, a test `e2e-app/z-pelny-dostep` jest pomijany bez `ALFA_TEST_AUTONOMY`.

**Sprawdzone w prawdziwym oknie (31.10 ok. 12:10):** powłoka zbudowana z gałęzi (przeniesionej na `112c421c`,
osobny katalog `alfa-testy-pelny-dostep`), `ALFA_TEST_AUTONOMY=L4`:

- `e2e-app/z-pelny-dostep` — instancja testowa startuje z poziomem L4 („Pełna automatyka”) bez okna zgód: zielony;
- `e2e-app/z-katalog` (każde ustawienie zapisane, odczytane i przywrócone) na L4: zielony.

Pierwsza próba wykryła błąd w moim podłączeniu: `run.ps1` wymaga świeżego profilu i odmawiał startu, bo plik
poziomu leżał już w `Local\Alfa`. Poprawka na głównej gałęzi (`112c421c`): strażnik pomija wyłącznie ten jeden
plik, każdy inny plik danych Alfy nadal zatrzymuje bieg.

## 4. Sprawdzenie

`verify wersja` na `ad95c3ec`: **18 z 19 zielonych** (dziewiętnasty, `audyt-npm`, naprawiony po przebiegu — niżej): Rust 3306/0/41, interfejs 535, przeglądarka 754,
prawdziwa aplikacja 53 (pokrycie łącza 92,5 %), katalog, dokumenty, zależności, SBOM.

- Pierwszy przebieg (`0fef03bc`) wykrył nową podatność **wasmtime 36.0.19 (RUSTSEC-2026-0424)** — podbiłam
  łatkę w linii LTS 36.0.20 (`docs/vendor/wasmtime.md` na to pozwala), `cargo deny` zielony, wtyczki 23/23.
- **`audyt-npm` naprawiony po przebiegu (`2ef398ca`):** `minimatch` < 11.0.1 (DoS przy wzorcach glob) pod
  `eslint` i `typescript-eslint` — narzędzia programisty, nie trafiają do programu. Poprawka jest tylko w 11.x;
  wszyscy użytkownicy (`eslint`, `@eslint/config-array`, `@eslint/eslintrc`, `typescript-estree`) sięgają po klasę
  `Minimatch` albo funkcję `minimatch`, które 11.x eksportuje tak samo, więc nadpisanie w `pnpm-workspace.yaml`
  jest bezpieczne. Sprawdzone: `pnpm lint` zielony (eslint wykrywa błąd w pliku próbnym i pomija wykluczone
  katalogi), `svelte-check` 0/0, `pnpm audit --audit-level moderate` czysty. Zostaje `deepmerge` (niska, bez
  poprawki, poniżej progu bramki).
- Przegląd ustawień w prawdziwym oknie (`e2e-app/z-katalog`, 8:25) i przegląd palety (Edge) — zielone.

## 5. Czego nie sprawdziłam

- **Alfy właściciela po wgraniu** — pierwsze uruchomienie skrótem „Alfa” należy do właściciela.
- **Kopii zapasowej na F:** — dysk F: nie jest podłączony do laptopa (brak sprzętu). Kopię robi
  `scripts/backup.sh "/f/Kopie zapasowe programów Spopielony/Alfa"` po podłączeniu dysku.

## 6. Wgranie

0.0.79 wgrana 31.10 ok. 11:19 do `Alfa-uruchomiona` (`deploy-owner.sh --no-start`, Alfa nie była uruchomiona;
skróty „Alfa” na pulpicie i w menu Start odświeżone). Program otwiera właściciel skrótem.

## 7. Do decyzji właściciela

1. Przegląd i scalenie gałęzi `do-przegladu-testy-pelny-dostep` (Broker czyta poziom z profilu instancji testowej;
   działanie sprawdzone w prawdziwym oknie — §3). Do scalenia instancja testowa w `verify` działa na poziomie domyślnym.
2. Spacja „przytrzymaj, aby mówić”: podłączyć czy usunąć ze skrótów.
