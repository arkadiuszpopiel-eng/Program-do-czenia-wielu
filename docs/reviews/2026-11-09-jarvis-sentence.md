# Jarvis: ponowienie próby jednego zdania, #41

9.11.2026, Cloud Linux CPU. Źródło bazowe `169a6004b414d9fa610f10fdacde565de3bd2410`.
Karta [#41](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/41).
**Obie małe zmiany instrukcji nie poprawiły wyniku i zostały cofnięte.** Nie ma nowej zmiany
produkcyjnej app-chat ani podniesienia progów. Jakość Jarvisa pozostaje częściowa.

## Protokół i wyniki

Bielik1.5B v3 Q8_0, llama.cpp b11559; te same zweryfikowane pliki co w poprzedniej próbie #31.
4 persony wbudowane × 2 profile × 5 nowych rozmów, kolejno. W każdym przebiegu świeży proces
serwera i rdzenia.96 tokenów, temperatura0, timeout120s na turę,4 wątki CPU, `tools=false`.
Pytanie, limity i harness `jarvis_live.rs` pozostały dotychczasowe. Model działał bez równoległych
kompilacji i testów UI; osobno przed próbami51/51 testów ukierunkowanych i Clippy bez ostrzeżeń.

| Próba | Transport / autor / brak obcięcia | Imię Jarvisa | Żądane jedno zdanie Jarvisa | Czas całego testu |
| --- | --- | --- | --- | --- |
| Świeża baza | 40/40 | 20/20 | 0/20 |236,32s |
| Warunek długości w akapicie profilu | 40/40 | 20/20 | 0/20 |211,03s |
| Ten sam warunek na końcu, po formach imienia | 40/40 | 20/20 | 0/20 |238,42s |

W obu eksperymentach wszystkie40 tekstów są identyczne z nową bazą, także Standard.
Każda persona Jarvisa pięć razy odpowiada „Jestem Alfą/Betą/Gamą/Deltą. Jestem gotowa pomóc.”:
poprawne imię i rodzaj żeński, lecz dwa zdania. W Standardzie pozostają „Betta” i „Gamma”.
Warunek jednego zdania oceniono na zachowanych tekstach; zliczenie końców zdań jest tylko
pomocnicze dla tego prostego pytania, nie nową bramką evals. Czasy są opisowe; nie dowodzą
zmiany wydajności. Sukces testu integracji nie oznacza zaliczenia jakości.

## Zachowane dowody i ograniczenia

- [Surowa baza](artifacts/jarvis-bielik-2026-11-09-before.json),
  [pierwszy eksperyment](artifacts/jarvis-bielik-2026-11-09-after.json),
  [drugi eksperyment](artifacts/jarvis-bielik-2026-11-09-after-v2.json).
- [Podsumowanie i SHA256](artifacts/jarvis-sentence-summary-2026-11-09.json).
- Odrzucone zmiany, wyłącznie jako dowód eksperymentu:
  [wariant1](artifacts/jarvis-sentence-attempt1.patch), [wariant2](artifacts/jarvis-sentence-attempt2.patch).
  Nie stosować ich jako naprawy: wynik jakości nie poprawił się.

Zmiany były ogólnym warunkiem długości, bez wpisania gotowej odpowiedzi na pytanie próby.
Przeniesienie warunku wymagało sprawdzania obecności formy imienia zamiast jej położenia na końcu;
pozostałe asercje tożsamości, uprawnień, narzędzi i mikrofonu przeszły. Po cofnięciu przywrócono
także dotychczasowe asercje.51 testów:43 app-chat,5 Jarvis,2 własnej Luny,1 syntetycznego głosu.
Pierwszy Clippy1m56s, drugi18,91s; format i rozmiary3487 plików/264 crate'ów/0 naruszeń.

To próba headless na czterech personach wbudowanych, z atrapami audio i bez Windows/WebView2,
prawdziwego mikrofonu czy wykonania narzędzi. Luna była sprawdzona na atrapach, nie w tej próbie
Bielika. Nie ustalono przyczyny braku wpływu dodatkowego warunku; kolejnym krokiem diagnostycznym
jest zapis faktycznego żądania adaptera i odrębne pytania o długość, przed dalszą zmianą instrukcji.
Nie dopasowywać odpowiedzi na sztywno ani nie zwiększać budżetu, żeby zaliczyć ten przykład.
