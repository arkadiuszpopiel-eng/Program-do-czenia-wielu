# Głos

Z Alfą możesz rozmawiać głosem i przerywać jej w pół zdania — jak człowiekowi. Rozpoznawanie i synteza mowy działają
lokalnie, bez kluczy.

## Co trzeba przygotować

Modele i programy głosu pobierzesz w **Ustawienia → Modele i silniki** (zobacz
[Modele i silniki](10-modele-i-silniki.md)): `whisper-server (CPU)`, na komputerze z kartą NVIDIA także
`whisper-server (CUDA 12.4)` (rozpoznawanie na karcie; gdy nie wystartuje, Alfa sama wraca do procesora), model
**Whisper large-v3-turbo-q5_0** (z kartą NVIDIA) albo lżejszy **small-q5_1** (na procesorze), **piper** z głosem
**Piper pl_PL gosia** i **Silero VAD**. Po pobraniu silników mowy uruchom Alfę ponownie (zasobnik → **Wyjście**) —
silnik mowy jest wykrywany przy starcie. Większość z nich jest jeszcze
**do potwierdzenia** (instalacja po zgodzie na pokazaną sumę SHA-256). Pocket TTS instalujesz ręcznie. Gdy wolisz
skopiować pliki samodzielnie, trafiają do `%LOCALAPPDATA%\Alfa`:

| Co                                                                                                | Gdzie                                                                                  |
| ------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Program rozpoznawania mowy `whisper-server.exe` (whisper.cpp)                                     | `sidecars\whisper\` (wersja CUDA: `sidecars\whisper-cuda\`)                            |
| Model rozpoznawania mowy, plik `ggml-….bin` (np. `ggml-large-v3-turbo-q5_0.bin`)                  | `models\whisper\`                                                                      |
| Synteza mowy: Pocket TTS (`pocket-tts.exe` + modele) **albo** Piper (`piper.exe` + głosy `pl_PL`) | `sidecars\pocket-tts\` i `models\pocket-tts\` albo `sidecars\piper\` i `models\piper\` |
| (opcjonalnie) detektor mowy Silero `silero_vad.onnx`                                              | `models\silero\`                                                                       |

**Ustawienia → Głos** pokazuje „Głos niedostępny” z listą brakujących elementów, dopóki czegoś nie ma. Wskaźnik
mikrofonu w pasku tytułu podpowiada to samo.

## Ustawienia → Głos

- stan rozmowy głosowej i przycisk **Włącz rozmowę** / **Wyłącz rozmowę**,
- wybór mikrofonu i **Test mikrofonu** z paskiem poziomu,
- **Włączanie mikrofonu**: „Przełącznik” albo „Przytrzymaj, aby mówić (PTT)”,
- **Pigułka głosowa** włączona/wyłączona,
- **Głosy agentek (v0)** — odsłuchaj próbkę każdej z czterech: Alfa (ciepły, spokojny), Beta (pogodny, wyraźna
  dykcja), Gama (niższy, wolniejszy), Delta (jaśniejszy, żwawy).

## Mówienie

- **Przełącznik**: `Ctrl+Shift+M`, przycisk mikrofonu albo „Głos wł./wył.” w zasobniku. Mów naturalnie — Alfa sama
  rozpozna koniec Twojej wypowiedzi.
- **Przytrzymaj, aby mówić**: trzymaj `Spację` (poza polem tekstowym) tak długo, jak mówisz.
- **Do konkretnej agentki**: zacznij od imienia, np. „Delta, …”.
- Wypowiedź zapisuje się w rozmowie jak zwykła wiadomość; odpowiedź pojawia się na ekranie i jest czytana na głos.

Stan mikrofonu zawsze widać ikoną i tekstem: wyłączony, słucha, słyszy Cię, przetwarza, agentka mówi, wyciszony,
nie przeszkadzać.

## Przerywanie

Zacznij mówić, kiedy agentka mówi — od razu ścisza głos i milknie. Alfa zapamiętuje, ile odpowiedzi zdążyłeś
usłyszeć („Przerwano — usłyszano: …”), i bierze to pod uwagę w następnej odpowiedzi. Krótkie „mhm”, „tak” w trakcie
nie przerywają. Samo „nie” przerywa tylko wtedy, gdy agentka mówi i powiesz je osobno.

## Komendy głosowe (działają od razu, bez modelu językowego)

| Powiedz                             | Skutek                                           |
| ----------------------------------- | ------------------------------------------------ |
| „stop”, „stój”                      | Agentka przestaje mówić (praca w tle trwa dalej) |
| „czekaj”, „pauza”                   | Wstrzymanie mowy                                 |
| „wznów”, „kontynuuj”, „mów dalej”   | Dalej od miejsca przerwania                      |
| „powtórz”                           | Powtórzenie ostatniej wypowiedzi                 |
| „anuluj”                            | Stop mowy i bieżącego zadania                    |
| „głośniej”, „ciszej”                | Głośność                                         |
| „wycisz mikrofon”                   | Wyciszenie mikrofonu                             |
| „przełącz na Deltę” (lub inne imię) | Rozmowę przejmuje inna agentka swoim głosem      |
| „nie przeszkadzać”                  | Tryb nie przeszkadzać                            |
| „stop wszystko”                     | **STOP WSZYSTKIEGO** — jak `Ctrl+Shift+F12`      |

## Pigułka głosowa

Gdy rozmawiasz głosem przy ukrytym oknie głównym, w rogu ekranu pojawia się mała pigułka: kto mówi, poziom głosu,
to, co Alfa właśnie usłyszała, oraz przyciski **Stop** i **Wycisz**.

## Czytanie na głos

Akcja **Przeczytaj na głos** przy każdej wiadomości czyta ją głosem agentki, która ją napisała.

## Panel Głos (`Alt+6`): słowa wywoławcze, Twój głos, dyktowanie, czytanie

Te funkcje są w aplikacji, ale każdą włączasz sam — w panelu Głos albo w **Ustawienia → Głos**. Potrzebują modeli
z menedżera (**Ustawienia → Modele i silniki**, zobacz [Modele i silniki](10-modele-i-silniki.md)); bez modelu
panel pokazuje „niedostępne” i mówi, czego brakuje (**Wymaga plików**).

- **Słowa wywoławcze** „Hej Alfa / Beta / Gama / Delta” — tryb „zawsze słucham”. Dopóki model nie jest
  skalibrowany na Twoim głosie, włączenie wymaga potwierdzenia ryzyka. Przycisk **Testuj słowo wywoławcze** pokazuje,
  czy wykrycie działa. Opcja **Tylko mój głos budzi Alfę** odrzuca wykrycia z telewizora czy rozmowy obok.
- **Rozpoznawanie mojego głosu** — kreator rejestracji (kilka fraz). Profil jest zaszyfrowany, zostaje na tym
  komputerze i nie trafia do eksportu. Akcja ryzykowna zlecona głosem bez kliknięcia przechodzi tylko po
  zweryfikowaniu Twojego głosu (i zgodnie z poziomem autonomii); usuwanie plików głosem zawsze wymaga kliknięcia.
- **Dyktowanie** (`Ctrl+Alt+D`) — tekst trafia do okna, które było na pierwszym planie w chwili startu, nigdy do okien
  Alfy ani pól haseł. Komendy: „kropka”, „przecinek”, „znak zapytania”, „nowa linia”, „cofnij to”,
  „koniec dyktowania”. Dyktowany tekst nie trafia do agentek, logów ani pamięci.
- **Czytanie zaznaczenia** (`Ctrl+Alt+R`) — czyta zaznaczony tekst, dokument albo schowek głosem bieżącej agentki;
  `Esc` albo „stop” przerywa. Czytana treść jest traktowana jako niezaufana.

Skróty zmienisz w **Ustawienia → Skróty**. Okna programów uruchomionych jako administrator nie przyjmą dyktowania
(wymagają osobnego pomocnika — **jeszcze niedostępne**).

## Jeszcze niedostępne

- **Szybka rozmowa w chmurze** (mowa-na-mowę u dostawcy) — **wymaga klucza** i adaptera dostawcy, którego jeszcze nie ma.
- Nagrywanie korpusu głosu w aplikacji (do lepszego rozpoznawania mowy) i głosy agentek wybrane w castingu.
