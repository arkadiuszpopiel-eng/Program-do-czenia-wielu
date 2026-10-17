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
- **Dwa niezależne przeglądy kodu** (agenty AI, każde znalezisko sprawdzane przez 3 sceptyków): 15 + 14 usterek
  potwierdzonych i poprawionych, m.in. podwójne liczenie rezerwy pamięci APU.
- **Awaria komputera o 20:31** (brak pamięci + resety mostka USB dysku E:): dyski, repozytorium i pliki sprawdzone —
  nic nie stracone. Ciężkie zadania od tej pory z ograniczeniem równoległości.

## 2. Co doszło (commity na gałęzi `ccr-af4b63c6-3fyzaj`, niewypchnięte)

| Commit | Co |
| ------ | -- |
| `6ff0c311` | Wykrywanie (LUID, zintegrowana, sterownik), klasy niezależne od producenta, klasa Integrated, PDH, pakiety |
| `72c4e1cb` | 0.0.63: DirectML dla Supertonic, Qwen3-ASR na AMD/Intel, APU z dużą rezerwą, 15 poprawek po przeglądzie |
| `55bc0f06` | Qwen3-ASR przypięty do głównej karty przy kilku kartach Vulkan |
| `d02237bd` | `verify.mjs` na tym komputerze: Git Bash zamiast aliasu WSL |
| (ten) | 14 poprawek po drugim przeglądzie, raport, poprawka znacznika w dzienniku zmian, `backup.sh` |

Szczegóły dla użytkownika: [historia zmian](../../CHANGELOG.md) (0.0.63). Decyzja: [ADR 0028](../ADR/0028-directml-dla-glosu-i-mowa-na-kartach-amd-intel.md).

## 3. Jak to sprawdziłam

- **Na RX 9070 XT**: odczyt DXCore (sterownik 32.0.31041.1004) i liczników PDH (1189 MB zajęte, 22 programy);
  Supertonic procesor kontra DirectML — 3 przebiegi × 5 zdań (30,7 s mowy), stan ustalony RTF 0,154–0,159 kontra
  0,094–0,108; powrót na procesor przy bibliotece bez DirectML i przy nieistniejącej karcie; Qwen3-ASR na Vulkan —
  4 zdania wypowiedziane przez Supertonic: 3 identyczne, w jednym zła forma wyrazu („Zapisałem” zamiast „Zapisałam”
  — mówiła barwa męska); `llama-server --list-devices` widzi kartę pod tą samą nazwą co DXGI.
- **Testy**: Rust w 14 zmienionych crate'ach 546/0, cały workspace w `verify wersja` 2893/1 (ten jeden — mój błąd
  formatu w dzienniku zmian, poprawiony, test zielony); vitest 393/393; svelte-check 0 błędów; clippy czysty.
- **`verify wersja` 0.0.63** (commit `d02237bd`, 61 min): 13 zielonych, 5 czerwonych, 1 nieuruchomiony — rozdział 4.

## 4. Czego nie sprawdziłam albo nie przeszło (i dlaczego)

| Etap / sprawa | Stan | Co dalej |
| --- | --- | --- |
| Testy w przeglądarce (E2E) | nie wystartowały: **na tym komputerze nie ma Edge**, a Chromium Playwrighta jest w złej wersji (1228 zamiast 1194) | Twoja zgoda na `pnpm exec playwright install chromium` (ok. 150 MB) albo instalacja Edge |
| Prawdziwa aplikacja | **37 z 38**, pokrycie łącza 75 %; czerwony test okna logowania Claude Code (34 s) | zbadane: na tym komputerze **nie ma Claude Code**, a test zakładał, że jest (przycisk logowania pokazuje się zawsze). Test sprawdza teraz wykrycie narzędzia przez rdzeń; komunikat programu mówi wprost „Nie znaleziono programu Claude Code (claude) na tym komputerze — zainstaluj go…” zamiast mylącego „…po podłączeniu modułu ui-terminal”. Wynik ponownego biegu — rozdział 7 |
| `cargo deny` | czerwony: **nowe ostrzeżenia bezpieczeństwa** `ammonia 4.2.3` (RUSTSEC-2026-0373) i `zip 8.6.0` (RUSTSEC-2026-0367/0368/0369), opublikowane po 16.10 | zgoda na pobranie `ammonia 4.2.4` (zgodna łatka — **przed wydaniem**); `zip 9` to nowa wersja główna — Alfa nie używa podatnych funkcji (własne rozpakowywanie odrzuca dowiązania i nie przenosi SUID), więc to osobne zadanie |
| Graf zależności | `jq` nie ma na tym komputerze | sprawdziłam tymi samymi regułami bez `jq`: 1733 krawędzie, 0 naruszeń |
| Sprzęt | laptop z grafiką zintegrowaną, APU AMD, karty Intel, DirectML na NVIDIA — brak tych maszyn | próby na sprzęcie |

## 5. Decyzje dla Ciebie

1. **ADR 0028** — DirectML dla głosu (opcjonalnie, karta ≥ 12 GB) i Qwen3-ASR przez Vulkan na AMD/Intel: podpis.
2. **Licencja DirectML** — dopuszcza zbieranie danych diagnostycznych przez Microsoft (§2a); pozycja ma znaczek
   „Do potwierdzenia przez człowieka”.
3. **Pobrania**: `ammonia 4.2.4` (bezpieczeństwo), Chromium dla testów w przeglądarce (albo Edge).
4. **Wgranie 0.0.63** do `Alfa-uruchomiona` (`scripts/deploy-owner.sh`).
5. **Resztki starego `node_modules`** w `Alfa AI Program\stare-node_modules-acl-2026-10-17` (1447 plików starej
   instalacji Windows) — usunąć z uprawnieniami administratora (`takeown /f … /r` i usunięcie), jeśli chcesz.

## 6. Porządki i kopie

- Usunięte (odtwarzalne): stare artefakty kompilacji ok. 97 GB (E: był pełny — 0 GB), w tym 5,5 GB starej kompilacji
  powłoki (za Twoją zgodą), moje pliki testowe; scalona gałąź `seria3`. `PLAN/`, próby głosu i kopie na F: zostały.
- Kopia zabezpieczająca `F:\Alfa\kopia-2026-10-17_2115` (po commicie 0.0.63): 5837 sum zgodnych, paczka git pełna.
  Kolejna o 3:30. Skrypt `backup.sh` naprawiony (na tym komputerze paczka git po cichu się nie tworzyła).

## 7. Po poprawkach drugiego przeglądu

- Rust w 10 dotkniętych crate'ach 339/0 i `app-terminal` zielone; clippy workspace i powłoki czysty; vitest 393/393;
  lint, rozmiary i typy interfejsu zielone.
- Prawdziwa aplikacja (`scripts/verify-app.sh`, zbudowana od nowa): **37 zielonych, test logowania Claude Code
  pominięty** (brak narzędzia), pokrycie łącza 198 z 264 (75 %), kod wyjścia 0. Skrypt etapu kopiuje bibliotekę
  ONNX Runtime z `%LOCALAPPDATA%/Alfa/sidecars/onnxruntime` — na tym komputerze jej nie ma (brak danych Alfy), więc
  instancja testowa działała bez niej; bez wpływu na wynik.
