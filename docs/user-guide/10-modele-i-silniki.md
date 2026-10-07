# Modele i silniki

Wszystko, czego Alfa potrzebuje do pracy bez internetu i bez kluczy, pobierzesz w **Ustawienia → Modele i silniki**
(`Ctrl+,`). Są tam modele (rozmowy, rozpoznawania mowy, głosów, wykrywania mowy, słów wywoławczych, rozpoznawania
Twojego głosu, wyszukiwania w pamięci) i programy pomocnicze — „silniki” uruchamiane obok Alfy: `llama-server`
(model rozmowy), `whisper-server` (rozpoznawanie mowy) i `piper` (głos zapasowy).

## Pakiety 1–6 — wszystko naraz, dobrane do komputera

Na górze strony są **pakiety**: komplety modeli i silników w skali ocen od **6 (Wzorcowy)** do **1 (Minimalny)**.
Alfa sama dobiera do Twojego sprzętu wersje silników — karta NVIDIA: CUDA (z wersją na procesor w zapasie), karta
AMD lub Intel: Vulkan (z zapasem na procesor), bez karty: procesor.

| Ocena | Pakiet       | Co zawiera                                                                 | Wymagania (nominalnie)                         |
| ----- | ------------ | -------------------------------------------------------------------------- | ---------------------------------------------- |
| 6     | Wzorcowy     | Bielik 4.5B (z narzędziami agentek), Whisper turbo, Piper, wykrywanie mowy, słowo wywoławcze, weryfikacja głosu, wyszukiwanie znaczeniowe | RAM ≥ 16 GB, karta ≥ 8 GB                      |
| 5     | Bardzo dobry | jak 6, bez słowa wywoławczego i weryfikacji głosu                          | RAM ≥ 16 GB, karta ≥ 6 GB                      |
| 4     | Dobry        | Bielik 1.5B (bez narzędzi), Whisper turbo, Piper, wykrywanie mowy, wyszukiwanie znaczeniowe | RAM ≥ 12 GB, karta ≥ 4 GB albo procesor ≥ 8 rdzeni |
| 3     | Zrównoważony | Bielik 1.5B, Whisper small, Piper, wykrywanie mowy                         | RAM ≥ 8 GB, karta ≥ 4 GB albo procesor ≥ 6 rdzeni |
| 2     | Lekki        | Bielik 1.5B, Whisper small, wykrywanie mowy (odpowiedzi tekstem)           | RAM ≥ 8 GB                                     |
| 1     | Minimalny    | sam Bielik 1.5B — rozmowa tekstowa                                        | RAM ≥ 6 GB                                     |

- **Zalecany dla tego komputera** (zielona ramka) — najwyższa ocena, która działa bez kompromisów. **Na styk** —
  zadziała wolniej (np. część modelu na procesorze); powód jest napisany pod opisem. **Za słaby sprzęt** — pakiet
  pobierzesz dopiero po potwierdzeniu.
- **Pobierz pakiet** pobiera w tle wszystko, czego brakuje; **Dokończ pobieranie** — resztę; **Napraw pakiet** —
  uszkodzone elementy. **Sprawdź pliki (SHA-256)** liczy sumy zainstalowanych plików jeszcze raz.
- **Elementy pakietu** — lista z działaniami dla każdego elementu osobno: **Pobierz**, **Wznów**, **Sprawdź**
  i **Napraw** (usuwa pliki elementu i pobiera go od nowa — po potwierdzeniu; reszta pakietu zostaje). Element bez
  przypiętej sumy zatwierdzasz na karcie zgody w katalogu niżej.
- **Jakość i normy** — uwagi z odwołaniem do norm: ISO/IEC 25010 (wydajność, niezawodność), ISO/IEC 25059 (jakość
  systemów AI), WER (metodyka NIST SCLITE), ITU-T P.800/P.808 (ocena naturalności mowy MOS), ITU-T G.114
  (opóźnienie), ISO/IEC 19795-1 (biometria: FAR/FRR/EER). To zalecenia i metody pomiaru, nie certyfikaty; wartości
  „do zmierzenia” poznasz po pomiarze na swoim komputerze.
- Pakiety się nie wykluczają: wspólne elementy pobierają się raz.

## Pobieranie

1. Wybierz pozycję (filtry **Rodzaj** i **Stan** zawężają listę) i kliknij **Pobierz**. Naraz pobierają się
   najwyżej dwie pozycje — kolejne czekają w kolejce.
2. **Przerwij** zatrzymuje pobieranie, a **Wznów** zaczyna od miejsca przerwania (także po ponownym uruchomieniu
   Alfy albo zerwaniu połączenia).
3. Każdy plik jest sprawdzany sumą **SHA-256**. Pozycje oznaczone **SHA-256 przypięty** mają sumę zapisaną
   w aplikacji — instalują się od razu, a plik z inną sumą jest usuwany.
4. Pozycja bez przypiętej sumy zatrzymuje się na karcie **Plik bez przypiętej sumy kontrolnej**: zobaczysz nazwę
   pliku, policzoną sumę SHA-256 (w grupach po 8 znaków, do porównania ze stroną źródła), adres i licencję.
   **Ufam temu plikowi — zainstaluj** instaluje plik i zapamiętuje sumę; **Odrzuć i usuń** kasuje pobrany plik.
   Jeśli plik zmieni się między pobraniem a zgodą, Alfa go usunie.
5. Programy pomocnicze przychodzą jako archiwa ZIP. Alfa rozpakowuje je bezpiecznie: odrzuca archiwum ze
   ścieżkami wychodzącymi poza katalog, dowiązaniami, powtórzonymi nazwami albo podejrzanie dużą kompresją;
   poprzednia wersja zostaje nietknięta, jeśli coś pójdzie nie tak.
6. Silnik pobrany tutaj działa bez ponownego uruchamiania Alfy: `llama-server` — od następnej wiadomości,
   `whisper-server` — od następnego włączenia rozmowy głosowej, **piper** (i Pocket TTS) — od następnego czytania
   na głos albo włączenia rozmowy.
7. Na komputerze z kartą NVIDIA pobierz wersje **CUDA** (`llama-server (cuda, …)`, `whisper-server (CUDA 12.4)`) —
   zawierają biblioteki CUDA, wystarczy sterownik karty. Gdy wersja CUDA nie wystartuje, Alfa przejdzie na
   procesor — dlatego pobierz też wersję CPU (dla `whisper-server` jest wymagana).

Pobieranie korzysta wyłącznie z HTTPS i niczego nie wysyła poza samym żądaniem pliku (bez telemetrii).

**Do potwierdzenia przez człowieka** — tak oznaczone są pozycje, których adresu, rozmiaru i licencji nie
sprawdzono jeszcze przy wydaniu (dziś: większość modeli z HuggingFace i programy z GitHuba). Działają, ale zawsze
przez kartę zgody. **Instalacja ręczna** (np. Pocket TTS) — opis pozycji mówi, co i gdzie skopiować.

## Sprawdzanie i usuwanie

- **Sprawdź pliki** liczy SHA-256 zainstalowanych plików jeszcze raz. Niezgodny albo brakujący plik zmienia stan
  na „Uszkodzone — pobierz ponownie”. Pliki skopiowane ręcznie mają stan „Zainstalowano ręcznie (bez weryfikacji)”.
- **Napraw** usuwa pliki pozycji (z częściowymi pobraniami) i pobiera ją od nowa — gdy plik się uszkodził albo
  źle zainstalował, a wznowienie nie pomaga (z potwierdzeniem).
- **Usuń** kasuje pliki pozycji razem z częściowymi pobraniami (z potwierdzeniem). Modelu, którego używa
  wyszukiwanie, nie da się usunąć — najpierw przełącz wyszukiwanie na inny.

## Wyszukiwanie w pamięci

Karta **Wyszukiwanie w pamięci** pokazuje, czym Alfa porównuje znaczenie tekstów:

- **Leksykalne (bez modelu)** — działa zawsze, porównuje słowa.
- **Multilingual E5 small** (zalecany, ok. 490 MB, ok. 640 MB pamięci) — rozumie znaczenie po polsku i w wielu
  językach. Po pobraniu wybierz go i kliknij **Używaj** (albo **Używaj do wyszukiwania** przy pozycji).

Po zmianie modelu Alfa przelicza w tle wektory wszystkich rozmów i pamięci (pasek **Przebudowa wektorów**). Do
końca przebudowy wyszukiwanie działa pełnotekstowo, więc nic nie znika. **Przerwij przebudowę** wstrzymuje ją —
wznowi się od miejsca przerwania przy następnym uruchomieniu albo po **Przebuduj teraz**. Model ładuje się do
pamięci przy pierwszym użyciu i zwalnia ją po kilku minutach bezczynności.

## Gdzie trafiają pliki

| Co                                   | Katalog w `%LOCALAPPDATA%\Alfa`                            |
| ------------------------------------ | ---------------------------------------------------------- |
| Modele                               | `models\` (np. `models\whisper\`, `models\embed\<model>\`) |
| Programy pomocnicze                  | `sidecars\<silnik>\` (np. `sidecars\llama-vulkan\`)        |
| Pobrania w toku i czekające na zgodę | `downloads\<pozycja>\`                                     |
| Rekordy instalacji (sumy SHA-256)    | `state\models\`                                            |
