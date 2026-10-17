# Raport z dnia i nocy 2026-10-17 / 18 (wersja 0.0.63 — każda karta graficzna)

Polecenia: „Alfa ma działać na kartach AMD i Nvidia i iGPU różnego typu, więc musi być to dopracowane”, potem
„pobierz to, co potrzeba, i dodaj do programu, i zapisz odpowiednio wszystko w dokumentacji”, „najwyższa precyzja”,
a na noc: „pracuj dalej do 3:30, potem kopia bezpieczeństwa na dysk F: (katalog Alfa)”. Praca na **nowym
komputerze**: Ryzen 7 5700X3D, Radeon RX 9070 XT 16 GB, 32 GB RAM (dysk E: z projektem przeniesiony z laptopa).
Wersji 0.0.63 **nie wgrałam** do `Alfa-uruchomiona` (tam nadal 0.0.62) — to Twoja decyzja po lekturze.

## 1. Najważniejsze w pięciu punktach

- **Karty AMD, Intel i zintegrowane są obsługiwane na równi z NVIDIA**: wykrywanie od sterownika (DXCore), te same
  progi pamięci, klasa „karta zintegrowana”, pakiety z silnikiem Vulkan, pomiar pamięci kart bez `nvidia-smi`.
- **Mowa na AMD/Intel liczy na karcie**: Qwen3-ASR przez `llama-server` Vulkan — na RX 9070 XT 0,16–0,23 s na zdanie
  (wcześniej Whisper na procesorze, ok. 54 s na zdanie na laptopie). Whisper nie ma oficjalnej wersji Vulkan.
- **Głos Supertonic na karcie przez DirectML** (pobrane z NuGet Microsoftu, sumy zgodne z katalogiem NuGet):
  0,58 s zamiast 0,95 s na zdanie, 538 MB pamięci karty; włącza się na karcie z ≥ 12 GB. **ADR 0028 do podpisu.**
- **Trzy niezależne przeglądy kodu** (agenty AI, każde znalezisko sprawdzane przez 3 sceptyków): 15 + 14 + 4 usterki
  potwierdzone i poprawione, m.in. podwójne liczenie rezerwy pamięci APU.
- **Awaria komputera o 20:31** (brak pamięci + resety mostka USB dysku E:): dyski, repozytorium i pliki sprawdzone —
  nic nie stracone. Ciężkie zadania od tej pory z ograniczeniem równoległości.

## 2. Co doszło (commity na gałęzi `ccr-af4b63c6-3fyzaj`, niewypchnięte)

| Commit     | Co                                                                                                         |
| ---------- | ---------------------------------------------------------------------------------------------------------- |
| `6ff0c311` | Wykrywanie (LUID, zintegrowana, sterownik), klasy niezależne od producenta, klasa Integrated, PDH, pakiety |
| `72c4e1cb` | 0.0.63: DirectML dla Supertonic, Qwen3-ASR na AMD/Intel, APU z dużą rezerwą, 15 poprawek po przeglądzie    |
| `55bc0f06` | Qwen3-ASR przypięty do głównej karty przy kilku kartach Vulkan                                             |
| `d02237bd` | `verify.mjs` na tym komputerze: Git Bash zamiast aliasu WSL                                                |
| `0011a06b` | 14 poprawek po drugim przeglądzie, raport, poprawka znacznika w dzienniku zmian, `backup.sh`               |
| (ten)      | Poprawki po trzecim przeglądzie (rozdział 8), uzupełnienie raportu                                         |

Szczegóły dla użytkownika: [historia zmian](../../CHANGELOG.md) (0.0.63). Decyzja: [ADR 0028](../ADR/0028-directml-dla-glosu-i-mowa-na-kartach-amd-intel.md).

## 3. Jak to sprawdziłam

- **Na RX 9070 XT**: odczyt DXCore (sterownik 32.0.31041.1004) i liczników PDH (1189 MB zajęte, 22 programy);
  Supertonic procesor kontra DirectML — 3 przebiegi × 5 zdań (30,7 s mowy), stan ustalony RTF 0,154–0,159 kontra
  0,094–0,108; powrót na procesor przy bibliotece bez DirectML i przy nieistniejącej karcie; Qwen3-ASR na Vulkan —
  4 zdania wypowiedziane przez Supertonic: 3 identyczne, w jednym zła forma wyrazu („Zapisałem” zamiast „Zapisałam”
  — mówiła barwa męska); `llama-server --list-devices` widzi kartę pod tą samą nazwą co DXGI.
- **Testy**: Rust w 14 zmienionych crate'ach 546/0, cały workspace w `verify wersja` 2893/1 (ten jeden — mój błąd
  formatu w dzienniku zmian, poprawiony, test zielony); vitest 393/393; svelte-check 0 błędów; clippy czysty.
- **`verify wersja` 0.0.63** (commit `d02237bd`, 61 min): 13 zielonych, 5 czerwonych, 1 nieuruchomiony — testy
  workspace (wyżej) i rozdział 4.

## 4. Czego nie sprawdziłam albo nie przeszło (i dlaczego)

| Etap / sprawa              | Stan                                                                                                                                                                       | Co dalej                                                                                                                                                                                                                                                                                                                                                                             |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Lint interfejsu i skryptów | czerwony: mój commit `d02237bd` wydłużył `scripts/verify.mjs` do 409 linii (limit 400)                                                                                     | poprawione w następnym commicie — wybór powłoki i otoczenia etapów przeniesiony do nowego `scripts/verify-shell.mjs` (37 linii), `verify.mjs` ma teraz 377 linii; lint zielony (rozdział 7)                                                                                                                                                                                          |
| Testy w przeglądarce (E2E) | nie wystartowały: `verify` uruchamia je w **Edge** (`PW_CHANNEL=msedge`), a **na tym komputerze nie ma Edge**; Chromium Playwrighta jest w złej wersji (1228 zamiast 1194) | etap `verify` odblokuje tylko instalacja Edge; te same testy uruchomiłam ręcznie na Chromium Playwrighta (`pnpm exec playwright install chromium` w `apps/desktop/ui`, potem `pnpm build` i `pnpm test:e2e`): **467/467** po dwóch poprawkach — rozdział 10                                                                                                                          |
| Prawdziwa aplikacja        | **37 z 38**, pokrycie łącza 75 %; czerwony test okna logowania Claude Code (34 s)                                                                                          | zbadane: na tym komputerze **nie ma Claude Code**, a test zakładał, że jest (przycisk logowania pokazuje się zawsze). Test sprawdza teraz wykrycie narzędzia przez rdzeń; komunikat programu mówi wprost „Nie znaleziono programu Claude Code (claude) na tym komputerze — zainstaluj go…” zamiast mylącego „…po podłączeniu modułu ui-terminal”. Wynik ponownego biegu — rozdział 7 |
| `cargo deny`               | czerwony: **nowe ostrzeżenia bezpieczeństwa** `ammonia 4.2.3` (RUSTSEC-2026-0373) i `zip 8.6.0` (RUSTSEC-2026-0367/0368/0369), opublikowane po 16.10                       | `ammonia 4.2.4` (zgodna łatka) — **zrobione** po północy, rozdział 10; `zip 9` to nowa wersja główna — Alfa nie używa podatnych funkcji (własne rozpakowywanie odrzuca dowiązania i nie przenosi SUID), więc to osobne zadanie                                                                                                                                                       |
| Graf zależności            | `jq` nie ma na tym komputerze                                                                                                                                              | sprawdziłam tymi samymi regułami bez `jq`: 1733 krawędzie, 0 naruszeń                                                                                                                                                                                                                                                                                                                |
| Sprzęt                     | laptop z grafiką zintegrowaną, APU AMD, karty Intel, DirectML na NVIDIA — brak tych maszyn                                                                                 | próby na sprzęcie                                                                                                                                                                                                                                                                                                                                                                    |

## 5. Decyzje dla Ciebie

1. **ADR 0028** — DirectML dla głosu (opcjonalnie, karta ≥ 12 GB) i Qwen3-ASR przez Vulkan na AMD/Intel: podpis.
2. **Licencja DirectML** — dopuszcza zbieranie danych diagnostycznych przez Microsoft (§2a); pozycja ma znaczek
   „Do potwierdzenia przez człowieka”.
3. **Pobrania**: `ammonia 4.2.4` — zrobione po północy w ramach Twojej stałej zgody na pobrania do 3 GB (rozdział 10);
   Edge (etap `verify` testów w przeglądarce) — instalacja programu w systemie należy do Ciebie; `zip` 9 (nowa wersja
   główna) — osobne zadanie.
4. **Wgranie 0.0.63** do `Alfa-uruchomiona` (`scripts/deploy-owner.sh`).
5. **Resztki starego `node_modules`** w `Alfa AI Program\stare-node_modules-acl-2026-10-17` (1447 plików starej
   instalacji Windows) — usunąć z uprawnieniami administratora (`takeown /f … /r` i usunięcie), jeśli chcesz.

## 6. Porządki i kopie

- Usunięte (odtwarzalne): stare artefakty kompilacji ok. 97 GB (E: był pełny — 0 GB), w tym 5,5 GB starej kompilacji
  powłoki (za Twoją zgodą), moje pliki testowe; scalona gałąź `seria3`. `PLAN/`, próby głosu i kopie na F: zostały.
- O 23:05 na E: zostało znów tylko 8 GB: wieczorne kompilacje testów zostawiły w `target\debug` 170 GB — każda zmiana
  ustawień kompilacji tworzy pełny nowy komplet plików testowych obok starego. `verify` pominął przez to etapy Rust
  (próg miejsca). Usunęłam tylko odtwarzalne pozostałości: starsze wersje plików testowych, które ma już nowsza
  kompilacja (722 programy z symbolami), i pamięć podręczną kompilacji przyrostowej (od awarii kompiluję bez niej)
  — zwolnione 130 GB, wolne 138 GB.
- Kopia zabezpieczająca `F:\Alfa\kopia-2026-10-17_2115` (po commicie 0.0.63): 5837 sum zgodnych, paczka git pełna.
  Kolejna o 3:30. Skrypt `backup.sh` naprawiony (na tym komputerze paczka git po cichu się nie tworzyła).

## 7. Po poprawkach drugiego przeglądu

- Rust w 10 dotkniętych crate'ach 339/0 i `app-terminal` zielone; clippy workspace i powłoki czysty; vitest 393/393;
  lint, rozmiary i typy interfejsu zielone.
- Prawdziwa aplikacja (`scripts/verify-app.sh`, zbudowana od nowa): **37 zielonych, test logowania Claude Code
  pominięty** (brak narzędzia), pokrycie łącza 198 z 264 (75 %), kod wyjścia 0. Skrypt etapu kopiuje bibliotekę
  ONNX Runtime z `%LOCALAPPDATA%/Alfa/sidecars/onnxruntime` — na tym komputerze jej nie ma (brak danych Alfy), więc
  instancja testowa działała bez niej; bez wpływu na wynik.

## 8. Trzeci przegląd (poprawki z commitu `0011a06b`)

Te same zasady (3 obszary, każde znalezisko u 3 sceptyków): 10 zgłoszeń, 5 utrzymanych (jedno podwójne) —
wszystkie poprawione:

- **Lista kart co 20 ms**: widok silników w trakcie dyktowania i czytania na głos odczytywał karty (DXGI i DXCore)
  przy każdym takcie, choć miał tylko przed pierwszym złożeniem rozpoznawania mowy (`map_or` liczył wartość domyślną
  zawsze) — teraz leniwie.
- **Dwie karty NVIDIA**: wolna pamięć była brana z karty z największą wolną pamięcią, nie z głównej (na której liczy
  Qwen3-ASR) — przy jednej karcie NVIDIA (laptop RTX 4050) bez zmian; przy kilku — wiersz głównej karty po nazwie,
  a bez pasującego wiersza liczniki systemu dla tej karty.
- **Ocena „mieści się na karcie”** na stronie modeli: przy APU z dużą rezerwą obok karty osobnej brała rezerwę APU,
  a model rozmowy trafia na kartę osobną — teraz ta sama reguła co w rdzeniu.
- **Ten raport**: brakowało czerwonego etapu lint (tabela w rozdziale 4) i tego, że etap `verify` testów
  w przeglądarce wymaga Edge.
- Przy okazji (zgłoszenie odrzucone przez 2 z 3 sceptyków, bo to nie regresja): angielski komunikat braku programu
  nadal zaczynał się od „This feature becomes available once its module is connected” — kod błędu to teraz
  „nie znaleziono”.

Po poprawkach (`c3ec80fb`): clippy i testy trzech dotkniętych crate'ów 86/0, vitest `card-fit` 4/4, lint zielony.
Czwarty przegląd tego commitu (2 obszary): kod bez uwag; 2 drobne poprawki tego raportu (liczba linii przy złym
pliku, zaniżony rozmiar pobrania Chromium) — wprowadzone.

## 9. Ostatni `verify wersja` przed kopią (commit `c3ec80fb`, 23:26–23:59)

**16 zielonych, 2 czerwone, 1 nieuruchomiony.** Testy całego projektu: 1003 programy, **2897 zielonych, 0 czerwonych**,
40 pominiętych; powłoka Tauri zielona; interfejs 394/394; prawdziwa aplikacja 37 zielonych (test logowania Claude Code
pominięty), pokrycie łącza 198 z 264 (75 %); formatowanie, lintery, schematy, typy, plan HTML, `pnpm audit`, SBOM —
zielone. Czerwone — te same przyczyny co w rozdziale 4: testy w przeglądarce (brak Edge; budowa i budżet rozmiaru
przeszły) i `cargo deny` (`ammonia`, `zip`). Nieuruchomiony: graf zależności (brak `jq`).

## 10. Po północy (18.10)

- **Twoje zasady pracy z laptopa** (zgody na pobrania, praca nocna, weryfikacja UI i inne) nie były w pamięci asystenta
  na tym komputerze — `memory-sync.sh` jako lustro skasowałby je z kopii w repozytorium. Przywróciłam je u źródła
  z kopii w `docs/pamiec-projektu/2-zasady-pracy` i zapisałam, jak to sprawdzać na nowej maszynie.
- **`ammonia` 4.2.4** (łatka RUSTSEC-2026-0373) — pobrana z crates.io w ramach Twojej stałej zgody na pobrania do 3 GB,
  oba pliki `Cargo.lock`; w pliku powłoki cargo przy okazji ujednolicił zależność `cssparser-macros` z `syn` 3 na już
  obecny `syn` 2 (żaden nowy crate). `cargo deny`: zostały tylko 3 ostrzeżenia `zip`; sprawdziłam w kodzie, że Alfa nie
  woła podatnego `ZipArchive::extract` (własne rozpakowywanie w `app-models` i `updater-impl` odrzuca dowiązania).
  Testy `lib-markdown`, `app-chat`, `app-files` 63/0, `app-updates`, `app-modules`, `app-core` 186/0, clippy czysty.
- **Testy w przeglądarce na Chromium Playwrighta** (pobrany: wersja 1194 z `cdn.playwright.dev`, ok. 530 MB na C:,
  poza repozytorium — także w ramach stałej zgody): pierwszy bieg 464/466. Dwa czerwone — dwa prawdziwe błędy:
  - test strony Urządzenia szukał starego napisu „Odczyt na żywo działa dziś dla kart NVIDIA…”, który sama zmieniłam
    w `6ff0c311` (pomiar działa już dla każdego producenta) — poprawiony test w przeglądarce i w prawdziwej aplikacji
    (tam nie było go widać, bo karta RX 9070 XT daje miernik);
  - **lista wyboru pomijała klik w pierwszych 0,3 s po wczytaniu okna** (zabezpieczenie przed ponownym otwarciem
    liczyło od zera, a nie od ostatniego zamknięcia) — na szybkim komputerze test archiwum klikał po 190–250 ms
    i lista się nie otwierała. Zmierzone, poprawione w `ui-kit` (`Select.svelte`), nowy test cofa zegar strony do zera
    tuż przed kliknięciem — na starej kompilacji czerwony, po poprawce zielony.
  - Po poprawkach: **467/467** (5 min), budżet rozmiaru zielony (434,6 KB gzip), vitest 394/394, `svelte-check` 0,
    lint zielony.
- **Końcowa zbudowana wersja** (Twoja zasada pracy nocnej): `alfa/target/shell/debug/alfa-desktop.exe` z commitu
  `7698da68` (00:37) — prawdziwa aplikacja jako instancja testowa: **37 zielonych, 1 pominięty** (brak Claude Code),
  pokrycie łącza 198 z 264 (75 %). Do `Alfa-uruchomiona` jej nie wgrałam — decyzja 4 w rozdziale 5 (ADR 0028
  i licencja DirectML czekają na Ciebie; pakiety z głosem Supertonic na karcie ≥ 12 GB, jak Twój RX 9070 XT,
  zawierają DirectML).
