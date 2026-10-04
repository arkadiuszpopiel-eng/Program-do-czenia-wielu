# Modele i silniki

Wszystko, czego Alfa potrzebuje do pracy bez internetu i bez kluczy, pobierzesz w **Ustawienia → Modele i silniki**
(`Ctrl+,`). Są tam modele (rozmowy, rozpoznawania mowy, głosów, wykrywania mowy, słów wywoławczych, rozpoznawania
Twojego głosu, wyszukiwania w pamięci) i programy pomocnicze — „silniki” uruchamiane obok Alfy: `llama-server`
(model rozmowy), `whisper-server` (rozpoznawanie mowy) i `piper` (głos zapasowy).

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

Pobieranie korzysta wyłącznie z HTTPS i niczego nie wysyła poza samym żądaniem pliku (bez telemetrii).

**Do potwierdzenia przez człowieka** — tak oznaczone są pozycje, których adresu, rozmiaru i licencji nie
sprawdzono jeszcze przy wydaniu (dziś: większość modeli z HuggingFace i programy z GitHuba). Działają, ale zawsze
przez kartę zgody. **Instalacja ręczna** (np. Pocket TTS) — opis pozycji mówi, co i gdzie skopiować.

## Sprawdzanie i usuwanie

- **Sprawdź pliki** liczy SHA-256 zainstalowanych plików jeszcze raz. Niezgodny albo brakujący plik zmienia stan
  na „Uszkodzone — pobierz ponownie”. Pliki skopiowane ręcznie mają stan „Zainstalowano ręcznie (bez weryfikacji)”.
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
