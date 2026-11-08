# Szybki dostęp agentek do narzędzi

Cel: krótsza droga od zadania do właściwej operacji, mniejsza ilość definicji
narzędzi wysyłanych modelowi i zgodność z profilami dostępu każdej agentki.

Kryteria:
1. Małe zestawy zachowują dotychczasowe definicje i wywołania.
2. Dla dużego zestawu model otrzymuje mały pakiet startowy dobrany do celu
   (operacje plikowe, terminal, sieć, dokumenty, przeglądarka itd.).
3. `tools_find` znajduje dostępne operacje po krótkiej komendzie, nazwie,
   kategorii albo opisie po polsku/angielsku. Wyniki mają stabilną kolejność.
   Znalezione schematy są dostępne w kolejnej turze, również po wznowieniu.
4. `tool_call` pozwala używać krótkich komend: read, list, find, edit, write,
   run, fetch. Używa tej samej walidacji, Brokera, cofania i limitów co
   kanoniczne narzędzie. Nie wykonuje dowolnego kodu ani poleceń z nazw komend.
5. Odkrywanie i skróty nie ujawniają ani nie wywołują narzędzi spoza profilu,
   koperty delegacji i trybu weryfikacji. Nie obchodzą detektora pętli.
6. Błędna komenda daje krótką podpowiedź właściwego narzędzia zamiast listy
   wszystkich narzędzi. Brak dopasowania jest jawny.
7. Benchmark porównuje rozmiar serializowanych definicji oraz czas lokalnego
   doboru na tym samym katalogu. Nie udaje pomiaru opóźnienia płatnego modelu.

Plan: testy regresji katalogu i skrótów → implementacja wyboru i wyszukiwarki →
integracja z pętlą/checkpointami → testy uprawnień i regresji runtime → pomiar →
przegląd osobnym kontekstem → build i aktualizacja kopii właściciela.
