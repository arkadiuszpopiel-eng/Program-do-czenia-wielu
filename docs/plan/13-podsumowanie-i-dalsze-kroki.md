# 13 — Podsumowanie pracy (2026-10-09 → 10) i dalsze kroki

Stan na **2026-10-10 rano**, wersja programu **0.0.16**. Dokument dla właściciela: co zostało zrobione od wersji
0.0.7, co jest sprawdzone, a co nie, i co proponuję dalej. Szczegóły zmian: [`../../CHANGELOG.md`](../../CHANGELOG.md);
stan zadań: [`03-roadmapa.md`](03-roadmapa.md).

## 1. Twoje zgłoszenia i ich stan

| Zgłoszenie | Wersja | Stan | Czego nie sprawdziłam |
| --- | --- | --- | --- |
| Klucz API dodany, a rozmowa nie działa (`tts-1`) | 0.0.8 | naprawione: rozmowa wybiera model rozmowy | rozmowa na Twoim prawdziwym kluczu |
| Zakładka API z kolejkami do 10 modeli per zadanie | 0.0.8, 0.0.10 | zrobione; kolejka to kafelki „podnieś i upuść” | — |
| Mikrofon łapie szum wentylatorów | 0.0.8 | redukcja szumów w 3 poziomach | ocena na słuch w Twoim otoczeniu |
| Program słucha, ale nie odpowiada | 0.0.8 | naprawione (zegar potoku głosowego) | cała rozmowa głosowa na żywo |
| Surowy JSON narzędzia w odpowiedzi | 0.0.8 | odfiltrowany | — |
| Elementy systemowe w interfejsie (menu, dymki, belka) | 0.0.10–0.0.13 | menu, podpowiedzi i belka okna rysuje Alfa | — |
| Pasek LED w górnej belce | 0.0.11, 0.0.14 | jedzie od lewej do prawej, we własnym torze | — |
| Nawigacja Ustawień | 0.0.11 | od nowa: ikony, zwijane grupy, szukanie, klawiatura | — |
| Obsada, mikrofon, stan w belce | 0.0.12 | od nowa, z objaśnieniem stanu | — |
| Znak programu i belka okna | 0.0.13 | znak Alfy, nowa ikona programu | ikona na pasku zadań (pamięć podręczna Windows) |
| Ekran ładowania | 0.0.14 | jest | wygląd w prawdziwym oknie przy starcie |
| Ikony agentek, mini awatary w rytm mowy | 0.0.14 | twarz dla mówiącej i pracującej agentki | ruch ust przy prawdziwej mowie |
| Sztywny, niemiły głos | 0.0.14 | strojenie mowy (styl, tempo, pauzy, różnice głosów) | **brzmienie — do Twojej oceny**; lepsze głosy to osobna decyzja (punkt 4) |
| Kilka kart graficznych | 0.0.11 | role kart, przypinanie modelu | praca na karcie zintegrowanej (brak silnika Vulkan) |
| Kamery, rozpoznawanie osób, opis obrazu | 0.0.15, 0.0.16 | plan C3; zrobiony pierwszy krok: kamery USB, podgląd, zdjęcie jako załącznik | **prawdziwa kamera** — kamera laptopa jest odłączona sprzętowo |
| Porządki w interfejsie | 0.0.9–0.0.16 | nazwy stron, grupy, panel „Agentki”, strona urządzeń w „System” | — |

## 2. Co powstało tej nocy (wersja 0.0.16)

- **Kamery:** strona „Kamery” w Ustawieniach, podgląd z kilku kamer naraz, przełączniki, znak działającej kamery
  w belce, przycisk „zdjęcie z kamery” w polu pisania, zamykanie kamer przez „STOP WSZYSTKIEGO”.
- **Klucze:** „Importuj ze zmiennych środowiskowych” na stronie kont.
- **Interfejs:** zwarty panel „Agentki”, „Urządzenia” w grupie „System”.
- **Pakiet decyzji:** [`../reviews/2026-10-10-pakiet-decyzji-wlasciciela.md`](../reviews/2026-10-10-pakiet-decyzji-wlasciciela.md)
  — 44 decyzje z opisem po ludzku i rekomendacją.
- **Przegląd kodu kamer w świeżym kontekście** (ADR 0016): wynik w sekcji 6.

## 3. Co zrobić rano (ok. 15 minut)

1. **Kamera:** odsłoń kamerę laptopa (zasłonka albo klawisz z ikoną kamery). W Alfie: Ustawienia → Kamery → włącz
   „Kamery włączone” → „Szukaj kamer” → „Podgląd”. Jeśli obraz jest odwrócony, ma złe kolory albo go nie ma —
   napisz, co widzisz; ten fragment kodu nie widział jeszcze prawdziwej kamery.
2. **Głos:** Ustawienia → Głos → „Głosy agentek” → „Odsłuchaj”. Zmień styl na „Żywy” albo „Spokojny” i różnice
   głosów na „Jeden głos” — powiedz, które ustawienie brzmi najlepiej.
3. **Zdjęcie z kamery:** przy włączonych kamerach kliknij aparat w polu pisania i napisz „opisz mi to, co widzisz”.
   Działa z modelem, który przyjmuje obrazy (np. przez Twoje konto API).
4. **Pakiet decyzji:** przy pięciu najważniejszych pozycjach (B-03, S-01, S-03, B-01, R-05) wystarczy „tak”.

## 4. Decyzje, które odblokowują najwięcej

| Decyzja | Propozycja | Co odblokuje |
| --- | --- | --- |
| Lepszy głos agentek | **A:** inny głos lokalny (pobranie, licencja do sprawdzenia — ADR 0024) albo **B:** głos z chmury przez Twoje konto API, włączany przełącznikiem, nigdy w sesji prywatnej. Proponuję B jako opcję obok głosu lokalnego | naprawdę różne i naturalne głosy; B wymaga adaptera mowy z dostępem do klucza — dlatego nie robiłam tego bez Ciebie |
| ADR 0025 — kamery i biometria | podpisać w zaproponowanym kształcie | rozpoznawanie osób i pamięć osób (C3.8–C3.13) |
| Dostęp agentek do kamer (`camera.look`) | zgoda na nową zdolność Brokera `camera.view` | „opisz, co widzisz” bez ręcznego dołączania zdjęcia, także głosem |
| Obraz z kamery do modelu w chmurze | za zgodą dla konkretnej kamery | droga chmurowa opisu obrazu |
| Pobranie silnika „llama.cpp Vulkan” | zgoda | lekkie zadania na karcie zintegrowanej |
| Pięć najważniejszych decyzji bezpieczeństwa | „tak” dla rekomendacji | wdrożenie i przegląd Jądra, wejście na `main` |
| `gh auth login` i wypchnięcie commitów | Twoje działanie | kopia pracy poza tym dyskiem — dziś wszystko jest tylko lokalnie |

## 5. Plan następnych sesji (kolejność proponowana)

| Kolejność | Zadanie | Wymaga decyzji? |
| --- | --- | --- |
| 1 | Poprawki po Twoim teście kamery i ocenie głosu | nie |
| 2 | C3.5 — kamery sieciowe (RTSP, wyszukiwanie w sieci lokalnej), znak kamery w pigułce głosowej | zakres wyszukiwania w sieci — krótka zgoda |
| 3 | Głos z chmury albo nowy głos lokalny | **tak** (punkt 4) |
| 4 | C3.6–C3.7 — opis obrazu modelem lokalnym, narzędzie `camera.look` | zgoda na pobranie modelu i zdolność Brokera |
| 5 | B.1–B.3 — generator typów, numery zdarzeń, błędy strukturalne (szwy pod panel WWW i telefon) | nie |
| 6 | C2.14, C2.15 — okresowe sprawdzanie kluczy, konto zapasowe | nie |
| 7 | C2.1 — zestaw ewaluacyjny po polsku | nie |
| 8 | C3.8–C3.13 — osoby i ich pamięć | **tak** (ADR 0025) |
| 9 | A.7–A.8 — wdrożenie decyzji bezpieczeństwa, przegląd Jądra, wejście na `main` | **tak** (pakiet decyzji) |

## 6. Przegląd kodu kamer i wyniki testów

Wypełnione na końcu sesji — patrz niżej.
