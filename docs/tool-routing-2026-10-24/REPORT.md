# Szybszy dostęp agentek do narzędzi — 24.10.2026

Agentka otrzymuje mały zestaw definicji dobrany do celu i swojego dostępu.
Pełny katalog nadal istnieje lokalnie. Użytkownik może pisać normalnie;
poniższe komendy są wywołaniami modeli, nie nowym językiem poleceń użytkownika.

## Zmiany

- Zestawy do 24 narzędzi zachowują dotychczasowe definicje. Przy większym
  katalogu pakiet modelu ma najwyżej 24 pozycje, w tym lokalne wyszukiwanie
  i krótka komenda. Kolejność definicji pozostaje stabilna.
- `tools_find` dobiera operacje po nazwie, krótkiej komendzie, kategorii albo
  opisie PL/EN. Dokładna nazwa kończy wyszukiwanie od razu. Podpowiedź toleruje
  jedną literówkę; podobna nazwa nigdy nie jest samoczynnie wykonywana.
- Odnalezione schematy są dostępne w kolejnej turze. Ostatnie odkrycia są
  zachowane w checkpoincie (limit 48 nazw), także po restarcie. Stare checkpointy
  bez nowego pola nadal są odczytywane.
- `tool_call` rozwija krótką komendę do zwykłego narzędzia przed walidacją,
  wykrywaniem pętli i wykonaniem. Przez to zachowuje Brokera, cofanie, limity,
  proweniencję oraz prawdziwą nazwę operacji w śladzie wykonania.
- Nieznane narzędzie daje najwyżej trzy podpowiedzi. Powtarzane lokalnie
  odrzucone skróty uruchamiają detektor pętli. Nie są z nimi utożsamiane
  uszkodzone odpowiedzi dostawcy, w których utracono pełne argumenty.
- Prompt kieruje od razu do odczytu znanej ścieżki, wyszukania pliku lub
  konkretnej operacji. Niezależne odczyty mogą być zlecane razem; zapisy
  pozostają sekwencyjne. Nie dodano równoległych zapisów ani omijania zgód.
- Preflight Routera używa tego samego selektora co runtime, więc nie szacuje
  kosztu etapu na podstawie pełnego katalogu, którego model nie otrzyma.
- Czat i zadania w tle korzystają ze wspólnego wyboru katalogu roboczego.
  Odczyt metadanych arkusza faktów został przeniesiony do modułu zadań,
  zachowując pracę poza wątkami async i limit wyniku.

## Krótkie komendy

| Komenda modelu | Operacja |
| --- | --- |
| `read` / `czytaj` | `fs_read` |
| `list` / `lista` | `fs_list` |
| `find` / `szukaj` | `fs_search` — pliki, nie katalog narzędzi |
| `write` / `edit` / `zapisz` | `fs_write`, wymagane `content` i właściwy tryb zapisu |
| `run` / `uruchom` | `shell_run` |
| `fetch` / `pobierz` | `net_fetch` |
| `tools` / `narzędzia` | `tools_find` |

Przykład odczytu ograniczonego do 8 KiB:

```json
{"command":"read","args":{"path":"E:/Projekt/README.md","max_bytes":8192}}
```

Powyższy obiekt jest wejściem `tool_call`. Gdy model nie zna schematu operacji,
może wywołać `tools_find` z `{"query":"read"}`. `edit` jest skrótem zapisu
treści pliku, nie operacją zamiany fragmentu tekstu.

## Pomiary i granice dowodu

Powtarzalny test z syntetycznym katalogiem 86 narzędzi:

| Miara | Pełny zestaw | Nowy pakiet startowy |
| --- | ---: | ---: |
| Definicje | 86 | 12 |
| JSON schematów, bajty | 20 956 | 4 620 |
| Zmniejszenie danych definicji | — | 77,95% |

Pomiar lokalnego indeksu na 80 syntetycznych narzędziach, build debug,
1000 powtórzeń: dokładna nazwa około 6,7 µs na wyszukanie, opis około 1,7 ms.
To pomiar selektora na tej maszynie i wielkości pakietu, a nie czasu sieci,
odpowiedzi konkretnego płatnego modelu ani całkowitego rachunku API.

Testy obejmują zakres uprawnień i rolę odczytu, weryfikację, aliasy, pętle,
odtworzenie schematów z pliku checkpointu i zgodność starszego formatu.
Przegląd w osobnym kontekście zamknięty po poprawkach preflight oraz detektora
odrzuconych komend. Kontrolę kodu i wykonania zapisują logi w
`target/tool-routing-verification`.

Kopia właściciela: `../Alfa-uruchomiona`. Powrót do wersji sprzed optymalizacji:
`target/tool-routing-rollback-2026-10-24`. Dane rozmów i klucze nie są przenoszone.

Wgrano EXE i DLL oraz potwierdzono zgodność SHA-256 z końcowym buildem.
Manifest plików: `target/tool-routing-verification/deployment.json`.
Aplikacja była zamknięta i pozostała zamknięta. Nie kontynuowano sterowania
ekranem po jego zatrzymaniu przez użytkownika; poprzednie poprawki interfejsu
zostały wcześniej sprawdzone w natywnym oknie.

Końcowe sprawdzenia: pełna regresja runtime (78 testów w zapisanym przebiegu),
osobno najnowszy zestaw 10 testów katalogu, integracje agentek/Pipeline/zadań,
Clippy bez ostrzeżeń oraz lint projektu. Odczyt wszystkich plików źródłowych,
także nowych: `app-core` mieści się w limicie 8000 linii (7998).
