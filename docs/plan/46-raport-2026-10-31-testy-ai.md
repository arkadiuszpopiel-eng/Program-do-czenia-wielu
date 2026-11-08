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
| Pełne uprawnienia dla testów przez AI            | instancja testowa startuje z zapamiętanym poziomem Brokera z własnego profilu (`ALFA_TEST_AUTONOMY=L4`) — zmiana w Brokerze przeszła przegląd właściciela i jest scalona (`b5f7787c`); każde `verify wersja` sprawdza program także na L4 (§3) |
| Kontynuuj kolejne etapy                          | ścieżki względne w wynikach narzędzi plikowych (z planu raportu 44, C2.3); spłata długu katalogu (§2)                                                                                                    |

## 2. Spłata długu i znaleziska

Stan po spłacie: **601 funkcji, 601 z testem (100 %), dług 31 → 0**.

| Spłacone                                        | Testy                                                                                                                       |
| ----------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| 17 poleceń rdzenia bez testu                    | `crates/app-core/tests/catalog_debt_*.rs` — 27 testów zachowania (projekty, arena, gałęzie, karty, nauka pamięci, cennik, ustawienia firmowe, kanwa, schowek-atrapa, aktualizacje bez sieci, odsłuch głosu, umiejętności) |
| Narzędzie `system_software`                     | `crates/tools-system-impl/tests/software.rs` — filtr rodzaju, pamięć wyniku i `refresh`, brak wykrywania → „niedostępne”     |
| Narzędzie `system_diagnostics`                  | istniejący test `app-diag` sprawdza teraz nazwę, pod którą zna je model                                                     |
| 5 zdarzeń stanu okna                            | `state/__tests__/events-state.test.ts` (obsada, koszty, stan systemu, Broker, raport Marszałka)                             |
| Klawisze pola pisania (5)                       | `e2e/composer-keys.spec.ts`                                                                                                 |
| Esc w rozmowie, wklejanie bez formatu           | `e2e/composer-keys.spec.ts` — kolejność łańcucha Esc (lista podpowiedzi przed zatrzymaniem generowania); Ctrl+Shift+V i tekst z HTML przechodzą do pola bez załącznika (bez sięgania do schowka systemu) |
| Spacja „przytrzymaj, aby mówić”                 | podłączona (niżej) — `state/__tests__/push-to-talk.test.ts`, `e2e/voice-ptt.spec.ts`                                          |
| Klawisze globalne (3)                           | `e2e-app/zy-skroty-globalne.spec.ts` na prawdziwej instancji: STOP WSZYSTKIEGO, dyktowanie, czytanie zaznaczenia (opis: [45 §6.3](45-testy-ai.md)) |

Znaleziska:

- **Spacja „przytrzymaj, aby mówić” (`voice.ptt`) nic nie robiła w oknie.** Skrót był w rejestrze i na stronie
  Skróty, a Ustawienia → Głos oferowały „Przytrzymaj, aby mówić (PTT)”, ale żaden kod interfejsu nie wywoływał
  polecenia rdzenia `voice_ptt`. **Podłączona** (`state/push-to-talk.ts`): w trybie PTT przytrzymanie Spacji
  w widoku rozmowy poza polem tekstowym i kontrolką włącza mikrofon, puszczenie (albo utrata fokusu okna) —
  wyłącza; w trybie „Przełącznik” Spacja działa jak dotąd. Wolisz usunąć PTT zamiast tego — to jedna zmiana.
- **Esc w rozmowie — moje wcześniejsze znalezisko było błędne.** Esc ma pełny łańcuch (`escapeChain`: menu
  i okna → szuflady → mowa → czytanie → zatrzymanie generowania); patrzyłam tylko na obsługę w polu pisania.
  Test sprawdza teraz kolejność.
- **Test klawiszy globalnych w świeżym profilu:** obszar powiadomień istnieje dopiero w widoku programu (po
  wprowadzeniu) — test kończy wprowadzenie sam, gdy biegnie osobno.

## 3. Pełne uprawnienia instancji testowej

Instancja testowa (`scripts/verify-app.sh`, profil tymczasowy w `target/`) dostaje plik zapamiętanego poziomu
Brokera przed startem (`scripts/trial/test-autonomy.sh`; odmawia ścieżek spoza `target/`, nigdy nie dotyka
profilu właściciela ani rejestru). Broker w procesie czyta ten plik przy starcie (`crates/app-broker/src/inproc.rs`,
test `standing_level_from_the_broker_dir_is_restored`). Zmianę przygotowałam na gałęzi
`do-przegladu-testy-pelny-dostep`; właściciel obejrzał ją 31.10 i zatwierdził scalenie („Zatwierdzam scalenie”) —
scalone w `b5f7787c`, testy `app-broker` i clippy zielone. Broker w procesie działa tylko w budowach
deweloperskich i testowych — wydanie bez osobnego Brokera przechodzi w bezpieczny stan (`kernel.rs`), więc
Alfy właściciela to nie dotyczy (wgrana 0.0.79 nie wymaga ponownego wgrania).

Skróty globalne w instancji testowej sprawdza polecenie budowy testowej `e2e_global_shortcut` (tylko cecha
`e2e`, w programie właściciela odmawia) — bez wstrzykiwania klawiszy w sesję Windows ([45 §6.3](45-testy-ai.md)).
Pierwsza próba z prawdziwymi klawiszami została przerwana: zrzut pulpitu pokazał, że właściciel pisał w tym
czasie w innym programie.

Od `8c6dde32` etap „aplikacja” w `verify wersja` po pełnym biegu uruchamia program jeszcze dwa razy na L4
(świeży profil za każdym razem): `z-pelny-dostep` i `z-katalog`. Czerwony przebieg L4 = czerwony etap.

**Sprawdzone w prawdziwym oknie (31.10 ok. 12:10):** powłoka zbudowana z gałęzi (przeniesionej na `112c421c`,
osobny katalog `alfa-testy-pelny-dostep`), `ALFA_TEST_AUTONOMY=L4`:

- `e2e-app/z-pelny-dostep` — instancja testowa startuje z poziomem L4 („Pełna automatyka”) bez okna zgód: zielony;
- `e2e-app/z-katalog` (każde ustawienie zapisane, odczytane i przywrócone) na L4: zielony.

Pierwsza próba wykryła błąd w moim podłączeniu: `run.ps1` wymaga świeżego profilu i odmawiał startu, bo plik
poziomu leżał już w `Local\Alfa`. Poprawka na głównej gałęzi (`112c421c`): strażnik pomija wyłącznie ten jeden
plik, każdy inny plik danych Alfy nadal zatrzymuje bieg.

## 4. Sprawdzenie

**Końcowy `verify wersja` na `ddb13fa0` (dług 0, Spacja PTT, klawisze globalne): 19 z 19 zielonych** jednym
pełnym biegiem, bez osobnych powtórek etapów — Rust 3307/0/41, interfejs 540, przeglądarka 758, prawdziwa
aplikacja 57 (w tym 4 testy klawiszy globalnych; pokrycie łącza 92,5 %), na L4: `z-pelny-dostep` 1/1,
`z-katalog` 1/1; katalog 601/601, dokumenty, zależności, audyt npm, SBOM. Bieg przed nim (`9c0bff97`) miał 18 z 19:
nowy test klawiszy globalnych czekał na pole pisania, a poprzedni plik testów zostawia okno w Ustawieniach —
test wraca teraz do rozmowy przyciskiem (`ddb13fa0`).

Wcześniej, przed spłatą ostatnich sześciu pozycji długu:

`verify wersja` na `8c6dde32` (po scaleniu i z przebiegiem L4): 19 z 19 zielonych — Rust 3307/0/41,
interfejs 535, przeglądarka 754, prawdziwa aplikacja 53 (pokrycie łącza 92,5 %) i na L4: `z-pelny-dostep` 1/1,
`z-katalog` 1/1; katalog, zależności, audyt npm, SBOM. Etap „dokumenty” był czerwony tylko dlatego, że po
ostatniej edycji dokumentu 45 nie odświeżyłam HTML — po `node scripts/docs-html.mjs` ten sam etap
(`docs-html.mjs --verify`) zielony, 0 problemów.

Wcześniejsze przebiegi:

- Pierwszy przebieg (`0fef03bc`) wykrył nową podatność **wasmtime 36.0.19 (RUSTSEC-2026-0424)** — podbiłam
  łatkę w linii LTS 36.0.20 (`docs/vendor/wasmtime.md` na to pozwala), `cargo deny` zielony, wtyczki 23/23.
- `ad95c3ec`: 18 z 19 — **`audyt-npm` naprawiony potem (`2ef398ca`):** `minimatch` < 11.0.1 (DoS przy wzorcach glob) pod
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
- **Spacji PTT z prawdziwym mikrofonem** — sprawdzona na atrapie głosu i w testach stanu; mówienie do
  mikrofonu z przytrzymaną Spacją ocenia właściciel (zmysły). Wejdzie w następnej wersji — wgrana 0.0.79 jej
  jeszcze nie ma.

Uwaga: pierwsza wersja testu wklejania (ok. 13:05) zapisywała przykładowy tekst „Pogrubiony tekst” do schowka
przez przeglądarkę testową — mogła nim zastąpić zawartość schowka systemu. Test przepisany tak, że schowka
systemu nie dotyka (zdarzenia tworzone w stronie).

## 6. Wgranie

0.0.79 wgrana 31.10 ok. 11:19 do `Alfa-uruchomiona` (`deploy-owner.sh --no-start`, Alfa nie była uruchomiona;
skróty „Alfa” na pulpicie i w menu Start odświeżone). Program otwiera właściciel skrótem.

## 7. Do decyzji właściciela

1. Spacja „przytrzymaj, aby mówić” — podłączyłam (§2); jeśli wolisz ją usunąć ze skrótów i ustawień, daj znać.

## 8. Kolejny krok (31.10, po wgraniu i resecie)

Przegląd ogólny narzędzi agentek i zdarzeń ([45 §6.4](45-testy-ai.md)): każde narzędzie (także przyszłe) ma
sprawdzony manifest i odrzuca błędne argumenty przed działaniem; każda ze 118 próbek zdarzeń (40 typów) trafia
do okna bez błędu. Oba zielone od razu — nie znalazły uchybień; test kontrolny potwierdza, że przegląd je łapie.
