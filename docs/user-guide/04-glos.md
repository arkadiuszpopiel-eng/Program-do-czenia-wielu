# Głos

Z Alfą możesz rozmawiać głosem i przerywać jej w pół zdania — jak człowiekowi. Rozpoznawanie i synteza mowy działają
lokalnie, bez kluczy.

## Co trzeba przygotować

Modele i programy głosu pobierzesz w **Ustawienia → Modele i silniki** (zobacz
[Modele i silniki](10-modele-i-silniki.md)): `whisper-server (CPU)`, model **Whisper large-v3-turbo-q5_0** (albo
lżejszy small-q5_1), **piper** z głosem **Piper pl_PL gosia** i **Silero VAD**. Większość z nich jest jeszcze
**do potwierdzenia** (instalacja po zgodzie na pokazaną sumę SHA-256). Pocket TTS instalujesz ręcznie. Gdy wolisz
skopiować pliki samodzielnie, trafiają do `%LOCALAPPDATA%\Alfa`:

| Co                                                                                                | Gdzie                                                                                  |
| ------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Program rozpoznawania mowy `whisper-server.exe` (whisper.cpp)                                     | `sidecars\whisper\`                                                                    |
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

## Jeszcze niedostępne

Moduły są gotowe, ale aplikacja jeszcze ich nie pokazuje:

- słowa wywoławcze „Hej Alfa / Beta / Gama / Delta” (tryb „zawsze słucham”),
- weryfikacja Twojego głosu (rozpoznanie właściciela),
- dyktowanie do dowolnego programu (Notatnik, Word, przeglądarka…),
- czytanie zaznaczonego tekstu z innego programu,
- panel Głos (`Alt+6`) z pełnym trybem głosowym, nagrywanie korpusu głosu, głosy i rozpoznawanie mowy w chmurze.
