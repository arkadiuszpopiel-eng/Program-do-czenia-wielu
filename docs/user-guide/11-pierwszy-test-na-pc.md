# Pierwszy test na PC — lista kontrolna

Ta lista prowadzi Cię krok po kroku przez pierwsze zbudowanie i sprawdzenie Alfy na Twoich komputerach. Nie musisz
znać programowania: każde polecenie wklejasz w całości do okna Terminala i naciskasz `Enter`. Przy każdym kroku jest
opis tego, co powinieneś zobaczyć.

**Jak wpisywać wyniki.** Na końcu każdej części jest tabela z dwiema kolumnami — po jednej na komputer. Wpisz **✅**
(działa jak opisano), **❌** (nie działa — dopisz krótko, co widzisz) albo **⏭** (pominięte), a obok liczby, o które
prosimy (czas, MB). Najprościej: otwórz ten plik w Notatniku (`notepad docs\user-guide\11-pierwszy-test-na-pc.md`),
zapisz kopię na Pulpicie („Plik → Zapisz jako”) i wpisuj wyniki w kopii.

| Kolumna | Komputer                                         | Ścieżka karty graficznej |
| ------- | ------------------------------------------------ | ------------------------ |
| Desktop | Ryzen 7 5700X3D, Radeon RX 9070 XT, 32 GB RAM    | Vulkan (AMD)             |
| Laptop  | Core i7-13700H, GeForce RTX 4050 6 GB, 16 GB RAM | CUDA (NVIDIA)            |

Pierwszy komputer zajmie około 3–4 godzin (głównie czekanie na instalację i kompilację), drugi — około 2. Części są
niezależne: możesz przerwać i wrócić. Oznaczenia F0-11, F3-01 itd. to numery kryteriów z
[ACCEPTANCE.md](../ACCEPTANCE.md). Ta lista to pierwszy, ręczny przegląd; dokładne, powtarzalne pomiary zrobi później
automatyczny runner.

## Jutro na laptopie (RTX 4050) — skrócona ścieżka (60–90 min)

Kolejność według wartości: najpierw to, co najbardziej może zepsuć pierwszy start i pierwszy model lokalny. Pełne
opisy kroków są w częściach 0–13 — tu tylko kolejność i to, na co patrzeć na tym laptopie. Wyniki wpisuj w tabelach
tych części (kolumna „Laptop”).

1. **Zasilacz podłączony przez cały test** i tryb zasilania **Najlepsza wydajność** (Ustawienia → System → Zasilanie
   i bateria). Na baterii Alfa celowo przenosi model lokalny na procesor (wolniej, bez karty) — pomiary bez zasilacza
   nic nie mówią o karcie. Baterię sprawdzisz osobno na końcu (punkt 12.5).
2. **Sterownik NVIDIA** (5 min). W Terminalu wpisz `nvidia-smi`. W ramce u góry ma być `Driver Version: …` i
   `CUDA Version: 12.4` **lub wyższa**, a niżej `NVIDIA GeForce RTX 4050 Laptop GPU`. Niższa wersja albo
   „nie rozpoznano polecenia” → zainstaluj aktualny sterownik Game Ready albo Studio ze strony NVIDIA i uruchom
   komputer ponownie. **CUDA Toolkit nie jest potrzebny** — biblioteki CUDA (cudart, cuBLAS) są w paczkach, które
   pobiera menedżer Alfy.
3. **Narzędzia i budowa** (części 0–2; 30–60 min, głównie czekanie): `setup-dev.ps1`, przy `[BRAK]` — `-Install`,
   potem `-Build`. W czasie budowy przeczytaj punkty 4–9.
4. **Pierwszy start** (część 4, `-Run`) i **wprowadzenie** (część 5): w kroku **Sprzęt** ma być GeForce RTX 4050
   i profil **D** (CUDA); pozostałe kroki pomiń. Na karcie **Model lokalny** kliknij **Pobierz** (Bielik 4.5B Q8_0,
   ok. 4,8 GB — oficjalny plik autorów).
5. **Model lokalny na karcie** (część 6, ok. 15 min). Ustawienia → **Modele i silniki** → **llama-server (cuda, …)**
   (dwa archiwa: serwer i biblioteki cudart, razem ok. 550 MB), na zapas **llama-server (cpu, …)** (14 MB — gdy
   wersja CUDA nie wystartuje, Alfa przejdzie na nią) oraz Bielik, jeśli nie pobrał się we wprowadzeniu.
   Sumy SHA-256 na kartach zgody porównaj z tabelą niżej (pliki z GitHuba) albo z raportem CI. Ponowne uruchomienie
   Alfy **nie** jest potrzebne — serwer pobrany w Ustawieniach działa od następnej wiadomości. W drugim oknie
   Terminala włącz zapis pracy karty (zostaw go do końca testu):

   ```powershell
   nvidia-smi --query-gpu=timestamp,name,memory.used,memory.total,utilization.gpu,temperature.gpu,power.draw --format=csv -l 2 | Tee-Object "$env:USERPROFILE\Desktop\alfa-nvidia-smi.csv"
   ```

   Wyślij trzy pytania z części 6. Bielik 4.5B Q8_0 nie mieści się w 6 GB w całości: Alfa kładzie na kartę **34 z 60
   warstw** i zostawia ok. 1,5 GB na rozpoznawanie mowy (liczby — w ramce „Pamięć karty” niżej). Przy pierwszej
   odpowiedzi `memory.used` rośnie o ok. **3,3–4,0 GB**, a `utilization.gpu` jest wyraźnie powyżej zera (przy części
   warstw na procesorze karta nie pracuje na 100 % — to normalne); w zwykłym `nvidia-smi` (trzecie okno) na liście
   procesów jest `llama-server.exe`. Gdy karta stoi na 0 %, a odpowiedź i tak przychodzi — model liczy procesor:
   wpisz ❌ i dołącz log (punkt 9).
6. **Głos** (część 9, ok. 15 min): pobierz **whisper-server (CPU)**, **whisper-server (CUDA 12.4)** (ok. 430 MB),
   model **Whisper large-v3-turbo-q5_0**, **piper**, głos **Piper pl_PL gosia** i **Silero VAD**. Ponowne
   uruchomienie Alfy **nie** jest potrzebne — wejdź jeszcze raz w Ustawienia → **Głos**: komunikat „Głos niedostępny”
   znika. W trakcie rozmowy głosowej w `nvidia-smi` widać też `whisper-server.exe` (rozpoznawanie mowy na karcie),
   a `memory.used` rośnie o kolejne ok. **1,0–1,5 GB** — razem ok. 4,6–5,4 GB z 6141 MiB. Model rozmowy **nie**
   przeładowuje się między wypowiedziami (w logu jedno „llama-server gotowy” na całą rozmowę).
7. **STOP WSZYSTKIEGO** (część 8): `Ctrl+Shift+F12` w trakcie długiej odpowiedzi i w trakcie czytania na głos.
8. Jeśli zostanie czas: część 12 (pomiary; bateria — punkt 12.5), potem część 10 (instalator). Dla porównania
   szybkości pobierz też **Bielik 1.5B** (1,6 GB) i wybierz go w rozmowie — mieści się na karcie w całości.
9. **Co zapisać i przekazać:** wypełnione tabele części 4–9; plik `alfa-nvidia-smi.csv` z Pulpitu (zapis zatrzymasz
   `Ctrl+C`); logi z dnia testu z `%LOCALAPPDATA%\Alfa\logs` (`alfa.<data>.000.log` — szczegóły w części 13); wynik
   `setup-dev.ps1` (część 13). Przed wysłaniem przejrzyj pliki (część 13: czego nie wysyłać).

**Pamięć karty (`nvidia-smi`, laptop RTX 4050 6 GB).** Alfa rozlicza pamięć karty z zapasem: z 5921 MB odejmuje
768 MB na pulpit, zostaje **5153 MB** na modele. Szacunki (do sprawdzenia Twoimi pomiarami — wpisz je w część 12):

| Sytuacja                                       | Co na karcie                                  | Szacunek Alfy | `memory.used` (ok.) |
| ---------------------------------------------- | --------------------------------------------- | ------------- | ------------------- |
| Alfa włączona, bez rozmowy                     | nic (pulpit liczy zintegrowana Iris Xe)       | 0 MB          | 0–0,4 GB            |
| Rozmowa tekstowa, Bielik 4.5B Q8_0 (`-c 8192`) | 34 z 60 warstw + pamięć kontekstu tych warstw | 3570 MB       | 3,3–4,0 GB          |
| … i rozmowa głosowa (whisper turbo CUDA)       | jw. + `whisper-server.exe`                    | 3570 + 1500   | 4,6–5,4 GB          |
| Bielik 1.5B Q8_0 + rozmowa głosowa             | cały model (32 warstwy) + whisper             | 2456 + 1500   | 3,4–4,2 GB          |
| Na baterii                                     | tylko whisper (model rozmowy liczy procesor)  | 1500 MB       | 1,0–1,6 GB          |

W logu (`%LOCALAPPDATA%\Alfa\logs`) zapisz linię `llama-server gotowy` (liczba warstw `gpu_layers=34`) oraz linie
celu `providers_local_impl::llama_server` (zapisywane domyślnie): `offloaded 34/61 layers to GPU`, `n_layer`,
`n_head_kv` i `llama_kv_cache … size = … MiB` (Alfa zakłada 480 MiB dla 4.5B i 256 MiB dla 1.5B przy `-c 8192`;
inna liczba = do poprawki w manifeście). Rozpoznawanie mowy pisze podobnie pod `voice_stt_impl::whisper_server`
(`using CUDA0 backend` = whisper na karcie).

**Jak rozpoznać brak pamięci karty (OOM).** (1) W logu — ostrzeżenia (`WARN`) celów `…::llama_server`
i `…::whisper_server`: `out of memory`, `cudaMalloc failed`, `failed to allocate CUDA0 buffer` albo zdarzenia `local.backend.fallback` (karta → procesor) i `local.sidecar.crashed`; odpowiedź się
urywa albo przychodzi wyraźnie wolniej niż poprzednie. (2) W `nvidia-smi` `memory.used` dobija do ok. 6100 MiB
i tam stoi. (3) Sterownik NVIDIA na Windows potrafi zamiast błędu przenieść nadmiar do pamięci RAM („Udostępniona
pamięć GPU” w Menedżerze zadań → Wydajność → GPU 1 rośnie powyżej ~0,5 GB) — wtedy błędu nie ma, ale tokeny na
sekundę spadają kilkukrotnie. W każdym z tych przypadków wpisz ❌, godzinę i dołącz `alfa-nvidia-smi.csv` oraz log.

**Sumy SHA-256 do kart zgody.** Pełny raport robi automat CI na Windows: GitHub → zakładka **Actions** → **Próba
generalna (Windows)** → najnowszy przebieg → **Summary** → tabela „Silniki na żywo (CPU) — raport do przypięcia
SHA-256” (adres, rozmiar i suma każdego pobranego pliku, także modeli z Hugging Face) oraz plik **alfa-live-report**
w sekcji **Artifacts** (JSON z sumami, układem archiwów i czasami). Archiwa z GitHuba sprawdziła też sesja AI
(2026-10-06: pobranie, `sha256sum`, lista plików w archiwum zgodna z katalogiem):

| Pozycja w menedżerze          | Plik na karcie zgody                  | Rozmiar (B) | SHA-256                                                            |
| ----------------------------- | ------------------------------------- | ----------- | ------------------------------------------------------------------ |
| llama-server (cuda, b6710)    | `llama-b6710-cuda-12.4.zip`           | 155967638   | `7c8e461cfa5f8c28d40c0f66bd1a551d421edbde7a411029ae4761fe7953615b` |
| llama-server (cuda, b6710)    | `cudart-cuda-12.4.zip`                | 391443627   | `8c79a9b226de4b3cacfd1f83d24f962d0773be79f1e7b75c6af4ded7e32ae1d6` |
| llama-server (vulkan, b6710)  | `llama-b6710-vulkan.zip`              | 27456102    | `3fe05bec64f07c0134cbdb6d14bb136f32b44296704e29eb3a095d4e758ba3d8` |
| llama-server (cpu, b6710)     | `llama-b6710-cpu.zip`                 | 14265804    | `fe53ba46d2c8d3d785e3b4ba27ba0aa1eb1b2aeaf23394ef0ff7c2517a2cb45d` |
| whisper-server (CUDA 12.4)    | `whisper-cublas-12.4.0-bin-x64.zip`   | 450211367   | `66638830959008de552de41223e718351abcf4e1b5c59188d1ae04ec20047c6b` |
| whisper-server (CPU)          | `whisper-bin-x64.zip`                 | 3831744     | `cc5d6126e025ef463524ed74c94d4b6a40bb67c2d1c3cb5aca02c773c388bdad` |
| piper (2023.11.14-2)          | `piper_windows_amd64.zip`             | 22477236    | `f3c58906402b24f3a96d92145f58acba6d86c9b5db896d207f78dc80811efcea` |
| WeSpeaker ResNet34 (VoxCeleb) | `wespeaker_en_voxceleb_resnet34.onnx` | 26534365    | `5ef208a9da1453335308a6b6f4e6dfbd7e183a38b604de0a57664f45d257fe94` |

Modele z Hugging Face (Bielik, Whisper, głos Piper) porównaj z raportem CI albo z opisem pliku na Hugging Face
(„SHA256”). Inna suma niż w tabeli → **nie** klikaj „Ufam temu plikowi”, zapisz obie sumy i zgłoś.

## 0. Przygotowanie (raz na każdym komputerze)

1. Zaktualizuj Windows (**Ustawienia → Windows Update**) i sterownik karty graficznej: na desktopie **AMD Software:
   Adrenalin Edition** ze strony AMD, na laptopie sterownik **NVIDIA** (Game Ready albo Studio) ze strony NVIDIA.
   Laptop podłącz do zasilacza.
2. Sprawdź, że na dysku `C:` jest co najmniej **40 GB** wolnego miejsca.
3. Otwórz **Terminal**: menu Start → wpisz `Terminal` → `Enter`. Zwykły — **nie** „Uruchom jako administrator”.
   Polecenia wklejasz prawym przyciskiem myszy albo `Ctrl+V`.
4. Zainstaluj Git:

   ```powershell
   winget install --id Git.Git -e --source winget
   ```

5. Zamknij Terminal, otwórz nowy i pobierz kod Alfy do krótkiej ścieżki `C:\alfa`:

   ```powershell
   git clone https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu C:\alfa
   cd C:\alfa
   ```

   Git poprosi o zalogowanie do GitHuba w przeglądarce — to logowanie samego Gita, Alfa go nie widzi. Jeśli sesja AI
   podała Ci nazwę gałęzi do testu, wpisz jeszcze `git checkout <nazwa-gałęzi>`. Po każdym otwarciu nowego Terminala
   najpierw wpisz `cd C:\alfa`.

| Punkt                                                      | Desktop (Vulkan/AMD) | Laptop (CUDA) |
| ---------------------------------------------------------- | -------------------- | ------------- |
| 0.1 Windows i sterownik zaktualizowane (wersja sterownika) |                      |               |
| 0.2 Kod pobrany do `C:\alfa`                               |                      |               |

## 1. Narzędzia: sprawdzenie i instalacja

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\setup-dev.ps1
```

Skrypt tylko sprawdza — niczego nie instaluje ani nie zmienia. `-ExecutionPolicy Bypass` pozwala uruchomić ten jeden
skrypt bez zmiany ustawień Windows. Zobaczysz listę: `[OK]` — gotowe, `[BRAK]` — trzeba doinstalować, `[UWAGA]` —
zalecenie, `[INFO]` — tylko informacja. Pod brakującymi pozycjami jest linia „co zrobić”.

Jeśli są pozycje `[BRAK]`:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\setup-dev.ps1 -Install
```

Przy każdej brakującej pozycji skrypt pokazuje polecenie i pyta `Uruchomić? [t/N]`: `t` + `Enter` instaluje, sam
`Enter` pomija. Instalatory Visual Studio, Node.js, Perla i Gita pokażą okno **Kontrola konta użytkownika** — to
prośba samego instalatora, kliknij **Tak**. Visual Studio Build Tools instaluje się 10–30 minut. Potem **zamknij
Terminal, otwórz nowy** (`cd C:\alfa`) i uruchom sprawdzenie jeszcze raz. Jeśli instalator Visual Studio poprosi o
restart — najpierw zrestartuj komputer.

Skrypt może doinstalować (tylko za Twoją zgodą, z oficjalnych źródeł: winget, rustup, npm): Visual Studio Build
Tools 2022 z C++ i Windows SDK (kompilator), Strawberry Perl (budowa szyfrowanej bazy danych), Rust 1.94, Node.js 22
i pnpm 10 (interfejs) oraz WebView2 (okno Alfy; w Windows 11 zwykle już jest). Vulkan SDK i CUDA Toolkit **nie są
potrzebne** — wystarczy sterownik karty (skrypt pokaże je jako `[INFO]`).

| Punkt                                                              | Desktop (Vulkan/AMD) | Laptop (CUDA) |
| ------------------------------------------------------------------ | -------------------- | ------------- |
| 1.1 Sprawdzenie wyświetla listę                                    |                      |               |
| 1.2 Instalacja brakujących pozycji bez błędu                       |                      |               |
| 1.3 Ponowne sprawdzenie: „Wszystkie wymagane narzędzia są gotowe.” |                      |               |

## 2. Budowa

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\setup-dev.ps1 -Build
```

Kolejno: zależności interfejsu (`pnpm install`), interfejs, procesy Jądra bezpieczeństwa (Broker, okno zatwierdzeń,
watchdog) i powłoka Alfy. Pierwsza budowa trwa **20–60 minut**, kolejne — kilka minut. Przewijające się napisy
`Compiling …` są normalne. Na końcu tabela **Podsumowanie** — każdy krok powinien mieć `OK`.

| Punkt                                                    | Desktop (Vulkan/AMD) | Laptop (CUDA) |
| -------------------------------------------------------- | -------------------- | ------------- |
| 2.1 Wszystkie kroki budowy `OK` (łączny czas w minutach) |                      |               |

## 3. Szybkie testy

Zamknij Alfę, jeśli działa (także ikonę w zasobniku), potem:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\setup-dev.ps1 -Test
```

Najpierw dwa zestawy automatyczne — nic nie musisz robić. Potem skrypt proponuje po kolei **testy na żywym systemie**:
przed każdym pisze, co zrobi, i pyta `Uruchomić? [t/N]`. Na co uważać:

| Test                            | Co zrobić                                                                                        |
| ------------------------------- | ------------------------------------------------------------------------------------------------ |
| Kosz, skróty globalne i schowek | test nadpisze schowek — skopiuj wcześniej ważną treść                                            |
| Notatnik                        | zamknij wcześniej wszystkie okna Notatnika; przez ok. 30 s nie ruszaj myszą ani klawiaturą       |
| okno zatwierdzeń                | kursor sam kliknie 100 razy — nie dotykaj myszy (ok. 1 min)                                      |
| mikrofon i głośniki             | usłyszysz kilka krótkich tonów                                                                   |
| blokada ekranu                  | po napisie `running 1 test` w ciągu 20 s naciśnij `Win+L`, potem odblokuj komputer               |
| tryb gry                        | po napisie `running 1 test` w ciągu 30 s włącz film na pełnym ekranie (np. wideo w przeglądarce) |

Na końcu **Podsumowanie**: `OK`, `BŁĄD (kod …)` albo `pominięty`. Przy błędzie skopiuj fragment od słowa `failures:`
do końca (część 13).

| Punkt                                                                           | Desktop (Vulkan/AMD) | Laptop (CUDA) |
| ------------------------------------------------------------------------------- | -------------------- | ------------- |
| 3.1 Testy interfejsu (vitest)                                                   |                      |               |
| 3.2 Testy crate'ów Windows                                                      |                      |               |
| 3.3 Na żywo: system (procesy, usługi, dziennik zdarzeń, zmienne)                |                      |               |
| 3.4 Na żywo: Kosz, skróty globalne i schowek                                    |                      |               |
| 3.5 Na żywo: Notatnik (drzewo UIA, wpisywanie, zrzut okna)                      |                      |               |
| 3.6 Na żywo: okno zatwierdzeń — 0 ze 100 kliknięć programowych uznanych (F0-16) |                      |               |
| 3.7 Na żywo: rejestr Windows                                                    |                      |               |
| 3.8 Na żywo: mikrofon i głośniki                                                |                      |               |
| 3.9 Na żywo: blokada ekranu                                                     |                      |               |
| 3.10 Na żywo: tryb gry (pełny ekran)                                            |                      |               |

## 4. Pierwsze uruchomienie (tryb deweloperski)

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\setup-dev.ps1 -Run
```

To samo, co `cargo tauri dev` z `apps/desktop/README.md`. Pierwszy start dokompilowuje powłokę (1–5 minut), potem
otwiera się okno Alfy. Terminal zostaw otwarty. W tym trybie Broker działa w procesie aplikacji — u góry rozmowy
zobaczysz baner „Tryb deweloperski: Broker w procesie aplikacji, bez okna zatwierdzeń.” To oczekiwane; pełnego Brokera
sprawdzisz w części 10. Zamknięcie okna chowa Alfę do zasobnika; całkiem wyłączysz ją: ikona Alfy w zasobniku →
**Wyjście** (albo `Ctrl+C` w Terminalu).

| Punkt                                            | Desktop (Vulkan/AMD) | Laptop (CUDA) |
| ------------------------------------------------ | -------------------- | ------------- |
| 4.1 Okno otwiera się (czas od polecenia do okna) |                      |               |
| 4.2 Baner trybu deweloperskiego widoczny         |                      |               |

## 5. Wprowadzenie (onboarding)

Przy pierwszym starcie Alfa prowadzi przez 8 kroków ([Pierwsze uruchomienie](01-pierwsze-uruchomienie.md)):

1. **Mikrofon** — wybierz mikrofon i powiedz kilka słów: pasek poziomu się porusza.
2. **Profil głosu** — zostaw propozycję Alfy i zapisz, jaki profil zaproponowała.
3. **Sprzęt** — pomiar. Zapisz wykrytą kartę i zalecany profil (desktop: Radeon i Vulkan; laptop: GeForce i CUDA).
4. **Konta i klucze** — „Pomiń — dodam później”. Na karcie **Model lokalny** kliknij **Pobierz** (dalej w części 6).
5. **Autonomia** — zostaw **L3**; **Korpus głosu** — „Później”; **Mosty CLI** i **Import** — pomiń. Na końcu
   kliknij **Zaczynamy**.

| Punkt                                               | Desktop (Vulkan/AMD) | Laptop (CUDA) |
| --------------------------------------------------- | -------------------- | ------------- |
| 5.1 Mikrofon: pasek poziomu reaguje na głos         |                      |               |
| 5.2 Sprzęt: wykryta karta i zalecany profil (wpisz) |                      |               |
| 5.3 Wprowadzenie kończy się bez błędu               |                      |               |

## 6. Model lokalny i pierwsza rozmowa (F1-03)

1. **Ustawienia** (`Ctrl+,`) → **Modele i silniki** ([opis](10-modele-i-silniki.md)). Pobierz model **Bielik 4.5B
   v3.0 Instruct** (Q8_0, ok. 4,8 GB), jeśli nie pobrał się we wprowadzeniu, oraz program do jego uruchamiania:
   - **Desktop:** **llama-server (vulkan)**.
   - **Laptop:** **llama-server (cuda, …)** — dwa archiwa (serwer i biblioteki cudart). Bez niej Alfa użyje wersji
     Vulkan albo CPU z ostrzeżeniem w logu. Gdyby pobieranie w menedżerze się nie udało — ręcznie, punkt 6a.

   Serwer pobrany w Ustawieniach działa od następnej wiadomości — bez ponownego uruchamiania Alfy.
2. Plik bez przypiętej sumy zatrzyma się na karcie **„Plik bez przypiętej sumy kontrolnej”** (zgoda TOFU — „zaufaj
   przy pierwszym użyciu”). Porównaj sumę SHA-256 z karty ze stroną źródła (link na karcie; na GitHubie suma jest
   przy pliku w sekcji _Assets_, na Hugging Face — w opisie pliku jako „SHA256”). Gdy się zgadza, kliknij **„Ufam temu
   plikowi — zainstaluj”**. Zapisz pierwsze 8 znaków każdej sumy — pomogą przypiąć sumy na stałe.
3. Nowa rozmowa (`Ctrl+N`). Wyślij kolejno: „Przedstaw się w dwóch zdaniach.”, „Wyjaśnij prostymi słowami, czym jest
   fotosynteza.”, „Podaj pięć pomysłów na obiad bez mięsa.” Stoperem w telefonie zmierz czas od wysłania do pierwszych
   słów odpowiedzi. Pierwsza trwa dłużej (model ładuje się do pamięci). Kapsuła „Odpowiada … · model” przy odpowiedzi
   powinna wskazywać model lokalny. Pełny zestaw 20 promptów do F1-03 przygotowuje sesja AI w `evals/F1/prompts-pl/`.

**6a. Laptop — llama-server w wersji CUDA ręcznie (tylko gdy menedżer zawiedzie).** Na stronie
<https://github.com/ggml-org/llama.cpp/releases/tag/b6710>, w sekcji _Assets_, pobierz dwa pliki:
`llama-b6710-bin-win-cuda-<wersja>-x64.zip` i `cudart-llama-bin-win-cuda-<ta sama wersja>-x64.zip`. Potem w Terminalu:

```powershell
New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\Alfa\sidecars\llama-cuda"; explorer "$env:LOCALAPPDATA\Alfa\sidecars\llama-cuda"
```

Rozpakuj zawartość **obu** archiwów do otwartego folderu (`llama-server.exe` ma leżeć bezpośrednio w nim) i uruchom
Alfę ponownie (zasobnik → **Wyjście**, potem `-Run`). Sumy SHA-256 porównaj ze stroną wydania tak jak w punkcie 2.

| Punkt                                                          | Desktop (Vulkan/AMD) | Laptop (CUDA) |
| -------------------------------------------------------------- | -------------------- | ------------- |
| 6.1 Model i llama-server zainstalowane (pierwsze 8 znaków sum) |                      |               |
| 6.2 Odpowiedź 1 — czas do pierwszych słów (s)                  |                      |               |
| 6.3 Odpowiedzi 2 i 3 — czas (s); po polsku i z sensem          |                      |               |
| 6.4 Kapsuła wskazuje model lokalny                             |                      |               |

## 7. Zadanie na plikach i „Cofnij” (F3-02)

1. Utwórz folder testowy z 10 plikami — wklej w Terminalu (trzy linie naraz):

   ```powershell
   $d = "$env:USERPROFILE\Alfa-test\porzadki"; New-Item -ItemType Directory -Force $d | Out-Null
   'faktura-01.pdf','faktura-02.pdf','umowa.pdf','wakacje-1.jpg','wakacje-2.jpg','zrzut.png','notatka.txt','lista-zakupow.txt','plan.txt','raport.docx' | ForEach-Object { Set-Content -Path (Join-Path $d $_) -Value "plik testowy $_" }
   explorer $d
   ```

2. W Alfie: nowa rozmowa → panel **Agentki** (`Alt+1`) → karta „Katalog roboczy agentek” → **Wybierz katalog…** →
   folder `Alfa-test\porzadki` w Twoim folderze użytkownika.
3. Wyślij: „Uporządkuj pliki w katalogu roboczym: utwórz podfoldery Dokumenty, Obrazy i Notatki i przenieś do nich
   pliki według rozszerzenia.”
4. Zobaczysz kapsułę aktywności („krok 1/…”), po zakończeniu toast **Cofnij** i pod odpowiedzią kartę „N kroków można
   cofnąć”. W Eksploratorze pliki są w podfolderach.
5. Kliknij **Cofnij** w toaście (8 s) albo **Oś czasu** (`Alt+2`) → **Replay** → **Cofnij krok** przy każdym kroku.
   Wszystkie pliki wracają na miejsce, podfoldery znikają.

Mały model lokalny bywa słaby w używaniu narzędzi — jeśli agentka nie wykona zadania, wpisz ❌ i jej odpowiedź.

| Punkt                                                              | Desktop (Vulkan/AMD) | Laptop (CUDA) |
| ------------------------------------------------------------------ | -------------------- | ------------- |
| 7.1 Agentka uporządkowała pliki (liczba kroków)                    |                      |               |
| 7.2 Toast „Cofnij” i karta kroków się pojawiły                     |                      |               |
| 7.3 Po cofnięciu stan jak na początku (10 plików, bez podfolderów) |                      |               |

## 8. STOP WSZYSTKIEGO — `Ctrl+Shift+F12` (F3-01)

1. Wyślij „Napisz bardzo długie opowiadanie o podróży pociągiem przez Polskę.” i w trakcie pisania naciśnij
   `Ctrl+Shift+F12`. Pisanie zatrzymuje się natychmiast, pojawia się toast „STOP WSZYSTKIEGO: zatrzymano pracę agentek.”
2. Powtórz przy zminimalizowanym oknie Alfy (skrót działa w całym systemie) i z menu ikony w zasobniku → **STOP
   WSZYSTKIEGO**.
3. Po części 9 powtórz w trakcie czytania odpowiedzi na głos — mowa milknie od razu.

Próg kryterium to poniżej 200 ms — szybciej niż mrugnięcie. Jeśli widzisz albo słyszysz opóźnienie, wpisz ❌ i ile
mniej więcej trwało.

| Punkt                                    | Desktop (Vulkan/AMD) | Laptop (CUDA) |
| ---------------------------------------- | -------------------- | ------------- |
| 8.1 Skrót zatrzymuje pisanie natychmiast |                      |               |
| 8.2 Działa przy zminimalizowanym oknie   |                      |               |
| 8.3 STOP z menu zasobnika                |                      |               |
| 8.4 Mowa milknie od razu (po części 9)   |                      |               |

## 9. Mikrofon i rozmowa głosowa (F2-01, F2-04, F2-06)

1. **Ustawienia → Modele i silniki** — pobierz **whisper-server (CPU)**, **piper**, głos **Piper pl_PL gosia**
   i **Silero VAD** (każdy przez kartę zgody jak w części 6) oraz model rozpoznawania mowy:
   - **Laptop:** **whisper-server (CUDA 12.4)** i model **Whisper large-v3-turbo-q5_0** — rozpoznawanie na karcie
     (gdy wersja CUDA nie wystartuje, Alfa sama przejdzie na procesor).
   - **Desktop:** tylko model **Whisper small-q5_1** — rozpoznawanie działa tu na procesorze (wersji Vulkan
     whisper-server nie ma w wydaniach), a model turbo jest na procesorze za wolny do rozmowy. Nie pobieraj obu
     modeli: Alfa bierze pierwszy alfabetycznie (turbo).

   Ponowne uruchomienie Alfy nie jest potrzebne — silniki są wykrywane przy użyciu ([Głos](04-glos.md)).
2. **Ustawienia → Głos**: znika komunikat „Głos niedostępny”. **Test mikrofonu** — pasek reaguje. Odsłuchaj próbki
   czterech głosów.
3. **Włącz rozmowę** (albo `Ctrl+Shift+M`) i powiedz „Jaka jest stolica Francji?”. Wypowiedź pojawia się jako
   wiadomość, odpowiedź jest czytana na głos. Stoperem zmierz 5 razy czas od końca Twojego zdania do pierwszego dźwięku
   odpowiedzi (cel: zwykle do 2 s).
4. **Przerywanie:** poproś „Opowiedz długo o historii Krakowa.” i w połowie zacznij mówić — głos cichnie i milknie, w
   rozmowie widać „Przerwano — usłyszano: …”.
5. W trakcie mowy powiedz „mhm” albo „tak” — to **nie** przerywa. Powiedz „stop” — mowa milknie od razu.
6. Desktop: sprawdź na słuchawkach i na głośnikach. Laptop: na **wbudowanym mikrofonie i głośnikach** (najtrudniejsze
   echo) — czy Alfa nie przerywa sama sobie?

| Punkt                                                          | Desktop (Vulkan/AMD) | Laptop (CUDA) |
| -------------------------------------------------------------- | -------------------- | ------------- |
| 9.1 Modele głosu zainstalowane, „Głos niedostępny” zniknął     |                      |               |
| 9.2 Rozpoznanie wypowiedzi i odpowiedź głosem                  |                      |               |
| 9.3 Czas od końca zdania do pierwszego dźwięku (5 pomiarów, s) |                      |               |
| 9.4 Przerwanie w pół zdania i „usłyszano: …”                   |                      |               |
| 9.5 „mhm” nie przerywa, „stop” zatrzymuje                      |                      |               |
| 9.6 Na głośnikach Alfa nie przerywa sama sobie                 |                      |               |

## 10. Instalator NSIS i Broker poza aplikacją

**Budowa i instalacja.** Wyłącz Alfę z trybu deweloperskiego (zasobnik → **Wyjście**), potem:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\setup-dev.ps1 -Installer
```

Budowa wersji wydaniowej trwa 30–90 minut. Na końcu skrypt pisze ścieżkę instalatora, np.
`C:\alfa\apps\desktop\src-tauri\target\release\bundle\nsis\Alfa_0.0.1_x64-setup.exe`. Uruchom go. Windows może pokazać
„System Windows ochronił ten komputer” — to Twoja własna kompilacja bez podpisu: **Więcej informacji → Uruchom mimo
to**. Instalacja nie pyta o administratora (trafia do `%LOCALAPPDATA%\Alfa`). Alfę uruchom z menu Start. Rozmowy i
modele z trybu deweloperskiego są widoczne (te same katalogi danych). Aktualizacje są w tej kompilacji wyłączone —
to oczekiwane.

**Tryb przenośny Brokera.** Wersja z instalatora uruchamia Brokera jako osobny proces. **Ustawienia → Uprawnienia
i bezpieczeństwo** pokazuje: „Tryb przenośny — Broker bez osobnego konta”, „Połączono”, „Okno zatwierdzeń: działa”,
„STOP WSZYSTKIEGO (Ctrl+Shift+F12): obsługuje watchdog, poza aplikacją” i „Izolacja: słabsza” (w tym trybie
oczekiwane). Test okna zatwierdzeń — na tej samej stronie zmień poziom autonomii z L3 na **L4**:

1. Otwiera się osobne, małe **okno Brokera**. Naciśnij `Enter` — nic się nie dzieje (`Enter` niczego nie zatwierdza).
2. Naciśnij `Esc` — odmowa, poziom zostaje L3.
3. Zmień znowu na L4 i w oknie Brokera kliknij **Zezwól tylko teraz** — poziom L4.
4. Wróć na L3 — obniżenie działa od razu, bez okna.

Potem powtórz część 8 (STOP) — teraz skrót obsługuje osobny proces watchdog.

**Usługa Brokera (pełna izolacja).** Usługa na osobnym koncie Windows wymaga jednorazowej zgody administratora (UAC)
i pliku konfiguracji przygotowanego dla Twojego komputera (bramka ludzka #10). **Nie instaluj jej samodzielnie** —
sesja AI przygotuje osobną instrukcję. Po instalacji Ustawienia pokażą „Usługa Brokera na osobnym koncie Windows”
i „Izolacja: pełna”.

| Punkt                                                                         | Desktop (Vulkan/AMD) | Laptop (CUDA) |
| ----------------------------------------------------------------------------- | -------------------- | ------------- |
| 10.1 Instalator zbudowany (czas), Alfa zainstalowana i startuje z menu Start  |                      |               |
| 10.2 Ustawienia: tryb przenośny, połączono, okno zatwierdzeń, watchdog        |                      |               |
| 10.3 Okno Brokera: `Enter` nic nie robi, `Esc` odmawia, kliknięcie zatwierdza |                      |               |
| 10.4 STOP (`Ctrl+Shift+F12`) obsłużony przez watchdog                         |                      |               |
| 10.5 Usługa Brokera                                                           | ⏭                    | ⏭             |

## 11. Eksport i import `.alfa` (F1-10)

Najpierw zrób części 1–6 na obu komputerach.

1. **Desktop:** **Ustawienia → Import i eksport → Eksport** — zaznacz konfigurację wspólną i sesje z tego testu →
   **Eksportuj…** → zapisz plik np. na pendrive albo w OneDrive.
2. **Laptop:** **Ustawienia → Import i eksport → Wybierz plik .alfa…** — podgląd pokazuje, skąd i z kiedy jest
   paczka i stan elementów („nowe”). Nic jeszcze nie zostało zmienione.
3. Tryb **Scal** → **Importuj**. Sesje z desktopu są na liście i da się je czytać.
4. **Cofnij import** — stan sprzed importu. Zaimportuj jeszcze raz.
5. Na laptopie dopisz wiadomość w zaimportowanej sesji, wyeksportuj i zaimportuj z powrotem na desktopie (**Scal**) —
   nowa wiadomość jest widoczna.
6. Klucze API nigdy nie trafiają do paczki: jeśli dodałeś jakiś klucz, na drugim komputerze konto jest bez klucza.

| Punkt                                          | Desktop (Vulkan/AMD) | Laptop (CUDA) |
| ---------------------------------------------- | -------------------- | ------------- |
| 11.1 Eksport                                   |                      |               |
| 11.2 Podgląd importu niczego nie zmienia       |                      |               |
| 11.3 Import (Scal), sesje czytelne             |                      |               |
| 11.4 Cofnij import                             |                      |               |
| 11.5 Powrót laptop → desktop bez utraty danych |                      |               |

## 12. Pomiary (F0-11, F1-08, F0-13, F2-01, F9-04)

Mierz na wersji z instalatora — tryb deweloperski zawyża zużycie pamięci.

1. **Start (F1-08):** wyłącz Alfę (zasobnik → **Wyjście**), odczekaj minutę, uruchom ją z menu Start i zmierz
   stoperem czas do pojawienia się okna. 3 próby (cel: do 1,5 s).
2. **Pamięć (F0-11, F1-08):** po minucie bezczynności otwórz Menedżera zadań (`Ctrl+Shift+Esc`) → **Procesy** →
   rozwiń **Alfa** → zsumuj kolumnę **Pamięć** wszystkich pozycji. Potem wyślij jedną wiadomość (ładuje się model
   lokalny) i odczytaj jeszcze raz.
3. **Dokładny pomiar (opcjonalnie):** skrypt spike'u (f) wymaga PowerShell 7 —
   `winget install --id Microsoft.PowerShell -e --source winget`, potem w nowym Terminalu (Alfa wyłączona):

   ```powershell
   pwsh -NoProfile -ExecutionPolicy Bypass -File .\evals\spikes\f-ram-tauri\measure-ram.ps1 -Machine desktop -NoInteractive -ExePath "$env:LOCALAPPDATA\Alfa\versions\0.0.1\alfa-desktop.exe"
   ```

   (na laptopie `-Machine laptop`). Wyniki trafiają do `evals\spikes\f-ram-tauri\results\`.

4. **Szybkość modelu (F0-13, F1-03)** — czasy z części 6; liczbę tokenów na sekundę mierzy
   `evals/spikes/h-sprzet/README.md`. **Głos (F2-01)** — czasy z części 9.
5. **Bateria (F9-04, tylko laptop, opcjonalnie):** odłącz zasilacz, zostaw otwartą, bezczynną Alfę na 15 minut;
   Menedżer zadań → **Alfa** → kolumna **Procesor** powinna pokazywać około 0 %.

| Punkt                                     | Desktop (Vulkan/AMD) | Laptop (CUDA) |
| ----------------------------------------- | -------------------- | ------------- |
| 12.1 Start z menu Start (3 pomiary, s)    |                      |               |
| 12.2 Pamięć w bezczynności (MB)           |                      |               |
| 12.3 Pamięć z załadowanym modelem (MB)    |                      |               |
| 12.4 Skrypt measure-ram (opcjonalnie)     |                      |               |
| 12.5 Bateria: procesor w bezczynności (%) | —                    |               |

## 13. Gdy coś nie działa — co zgłosić

Przy każdym ❌ dopisz: numer punktu, komputer, co zrobiłeś, czego się spodziewałeś i co widzisz (dokładny komunikat).
Okna Alfy celowo **nie pojawiają się na zrzutach ekranu** (ochrona przed agentkami) — zrób zdjęcie telefonem albo
przepisz komunikat.

| Co dołączyć                             | Jak                                                                                                                                                 |
| --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Wynik sprawdzenia narzędzi              | `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\setup-dev.ps1 *> "$env:USERPROFILE\Desktop\alfa-sprawdzenie.txt"` — plik na Pulpicie |
| Komunikaty z Terminala                  | zaznacz myszą ostatnie ok. 50 linii (przy błędzie testu — od `failures:`), `Ctrl+C`, wklej                                                          |
| Stan Alfy                               | **Ustawienia → Zdrowie systemu → Sprawdź teraz** — przepisz stan i listę incydentów                                                                 |
| Logi Alfy                               | `Win+R` → `%LOCALAPPDATA%\Alfa\logs` → `Enter`: pliki `alfa.<data>.000.log` (i `alfa-broker…`, `alfa-watchdog…`) z dnia testu — patrz niżej         |
| Zdarzenia Alfy                          | podkatalogi `diagnostics`, `model_calls` w tym samym folderze (pliki `.ndjson`); albo **Eksport** `.alfa` z zaznaczonymi tylko **Logami**           |
| Log uruchamiania (wersja z instalatora) | `%LOCALAPPDATA%\Alfa\launcher.log`                                                                                                                  |

### Gdzie są logi

- **Dziennik diagnostyczny** — `%LOCALAPPDATA%\Alfa\logs\alfa.<RRRR-MM-DD>.000.log` (aplikacja), obok
  `alfa-broker.…`, `alfa-broker-ui.…`, `alfa-watchdog.…`. Zwykły tekst; czas w UTC (latem w Polsce +2 h).
  Nowy plik codziennie i po 10 MB, najwyżej 14 plików na proces, 7 dni. Przy STOP WSZYSTKIEGO szukaj linii
  `kill-switch` z polem `latency_us` (czas w mikrosekundach).
- W trybie deweloperskim (część 4) te same linie widać na bieżąco w Terminalu.
- Przed powtórzeniem błędu włącz więcej szczegółów: `$env:ALFA_LOG = "debug"` w tym samym oknie PowerShell
  przed `setup-dev.ps1 -Run` albo `[logs]` / `level = "debug"` w `%APPDATA%\Alfa\config\shared.toml`
  (od następnego uruchomienia; szczegóły: `09-dane-i-zdrowie.md` → „Gdzie są logi”). Pełne wyjście serwerów modeli
  (każdy wiersz `llama-server` i `whisper-server`) — tylko na prośbę:
  `$env:ALFA_LOG = "debug,llama_server=debug,whisper_server=debug"`; te wiersze mogą zawierać fragmenty rozmowy,
  więc przed wysłaniem logu je przejrzyj.
- Gdy Alfa (wersja z instalatora) w ogóle się nie otwiera — najpierw zajrzyj do najnowszego `alfa.….log`
  (np. wpis o zmiennej `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`: Alfa odmawia startu, gdy zmienna włącza
  zdalne debugowanie WebView2 — usuń ją w „Zmiennych środowiskowych” i uruchom ponownie).

Klucze API i hasła są w logach zawsze zamazywane, treść wiadomości — domyślnie też; mogą zostać nazwy plików
i ścieżki, więc przejrzyj pliki przed wysłaniem. **Nie wysyłaj** kluczy API, haseł, zawartości folderów `.claude`,
`.codex`, `.ssh`, eksportów z Menedżera poświadczeń ani folderów `%LOCALAPPDATA%\Alfa\sessions` i `memory` (Twoje
rozmowy). Skrypt `setup-dev.ps1` niczego z tych miejsc nie czyta.

Typowe problemy:

| Objaw                                                      | Co zrobić                                                                                                            |
| ---------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| „uruchamianie skryptów jest wyłączone w tym systemie”      | wklej polecenie w całości, razem z `-ExecutionPolicy Bypass`                                                         |
| `winget` nie jest rozpoznawany                             | Microsoft Store → wyszukaj „Instalator aplikacji” → **Aktualizuj**                                                   |
| po instalacji dalej `[BRAK]`                               | nowe okno Terminala; po Visual Studio — restart komputera                                                            |
| skrypt: „Uruchom skrypt w zwykłym oknie Terminala”         | zamknij Terminal otwarty jako administrator i otwórz zwykły                                                          |
| `link.exe` not found, błędy `LNK…`                         | `-Install` jeszcze raz albo Visual Studio Installer → Modyfikuj → „Programowanie aplikacji klasycznych w języku C++” |
| błąd przy `openssl-sys` / `openssl-src` ze słowem `perl`   | `-Install` (Strawberry Perl), potem nowe okno Terminala                                                              |
| `os error 112` albo brak miejsca                           | zwolnij miejsce; `cargo clean` w `C:\alfa` usuwa wyniki kompilacji (następna budowa potrwa dłużej)                   |
| `the lock file … needs to be updated`                      | zgłoś sesji AI (nieaktualny `Cargo.lock` w repozytorium)                                                             |
| model nie odpowiada, komunikat o braku `llama-server`      | część 6 (pobierz właściwy llama-server; na laptopie w razie kłopotów punkt 6a)                                       |
| „Głos niedostępny” mimo pobranych silników                 | wejdź ponownie w Ustawienia → Głos; dalej — w Modele i silniki sprawdź stan „Zainstalowano” przy każdej pozycji      |
| odpowiedź się urywa, w logu `out of memory` / `cudaMalloc` | brak pamięci karty — ramka „Pamięć karty” w skróconej ścieżce laptopa; zapisz `nvidia-smi` i log                     |
| test na żywo: skrót zajęty                                 | wyłącz Alfę (także w zasobniku) i powtórz                                                                            |
| instalator: „System Windows ochronił ten komputer”         | **Więcej informacji → Uruchom mimo to**                                                                              |
