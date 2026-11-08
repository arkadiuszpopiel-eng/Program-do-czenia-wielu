# Kolejność modeli, koszt i ciągłość pracy

1. Wspólna kolejka „Wszystkie konta” danej klasy ma pierwszeństwo przed
   przypisaniem konta agentce i kolejkami pojedynczych kont. Jawny wybór modelu
   przez użytkownika pozostaje jawnym wyborem. Awaria dostawcy przechodzi do
   następnej pozycji, a nieskuteczne wykonywanie zadania eskaluje do kolejnej
   pozycji zamiast ponawiać bez końca ten sam model.
2. Zakończenie budżetu kroków/czasu nie usuwa kontekstu i wyników. Stan jest
   lokalnie utrwalany atomowo w szyfrowanym magazynie sesji, obejmuje też
   Dyrygentkę, kolejność etapów i potomków. Wznowienie nie ponawia zakończonych
   operacji. Operacja przerwana w toku ma jawny nieznany stan do sprawdzenia.
3. UI pokazuje zapisany punkt i pozwala zatwierdzić dalszą pracę, pozostawić
   ją wstrzymaną lub anulować. Dostępne osobne przełączniki automatycznego
   przedłużania kroków, czasu i oczekiwania na zatwierdzenie.
4. Oczekiwanie na zgodę nie zużywa czasu aktywnej pracy; timeout zgody nie
   może być utożsamiony z odmową właściciela. Żadna opcja nie zatwierdza za
   niego działania wymagającego zgody Brokera.
5. Limity kosztu, odmowa Brokera, anulowanie i kill-switch nadal obowiązują.
   Automatyczne przedłużanie jest jawne i raportowane. Długie zadania mogą
   przekraczać godzinę bez utraty lokalnego stanu.
6. Ograniczyć zbędne powtórzenia i pełne historie przekazywane dzieciom;
   poprawić ścieżkę po błędzie zbyt dużego snapshotu i nie kazać agentce
   wielokrotnie próbować identycznej, znanej jako nieskuteczna operacji.

Dowody: testy kolejki z przypisanym kontem, eskalacji po braku postępu,
zapisu/ponownego otwarcia sesji i wznowienia na granicy limitu, osobnych zgód
auto-przedłużenia, rozliczania czasu oczekiwania i regresji odmowy/kill-switcha.
Płatne wywołania modeli nie są wymagane do testów; pomiary syntetyczne będą
opisane jako syntetyczne. Dotychczasowe zmiany w checkout pozostają zachowane.
