# #58 — nowy Cloud, zachowane niezaliczone próby, 11.11.2026

**Nie usunięto halucynacji dotyczących wcześniejszego wykonania narzędzi.**
Nie wdrożono kolejnego promptu, fikcyjnych operacji ani podmiany odpowiedzi modelu.
Wynik0/30 wcześniejszego V1 pozostaje zachowany; ta próba go nie zastępuje.

## Odtworzenie i zakres hipotezy

Nowy Cloud nie miał poprzedniego scratch, modeli ani procesów. Pobrano opublikowany
`bfd69b974f4cab75d25f3328abe1996b10def1dc`, sprawdzono zegar, git, procesy i zakończone
CI188/Windows111. Pracuje jeden edytor; stare automatyzacje nie były wznawiane.
Rust1.94.1,pnpm10.33.0,llama.cppb11559 i Qwen3-8B Q4_K_M odtworzono z przypiętych źródeł.

`history::branch_messages` nadal przekazuje teksty historii, bez strukturalnych faktów
narzędzi. Sprawdzono hipotezę innego nośnika: rzeczywisty odczyt syntetycznego fixture
przez kontroler, z danymi jako para native-call/result przed bieżącym pytaniem.
Nie wykonuje to operacji komputera i nie jest wywołaniem zleconym przez model.
Kontrakt `providers_contract::ToolUse` opisuje wywołanie modelu z identyfikatorem
dostawcy; dlatego taka syntetyzowana para jest **wyłącznie prototypem laboratoryjnym**,
a nie przyjętym projektem produkcyjnej historii. Nie wolno przedstawiać jej jako akcji agentki.

Test rzeczywistej ścieżki Core na niezmienionej bazie wykonał się: **0/1, kod101**,
brak wyniku odczytu rejestru proponowanego przez prototyp;1test odfiltrowany.
To sprawdzenie hipotezy przyszłego kontekstu, nie gotowa implementacja lub naprawa.
Zachowano [źródła testu jako łatkę](patches/58-host-prototype-tests.patch), bez włączenia
niezaliczonego testu do normalnego CI. Druga integracja z zapisem/Undo w tej łatce
nie została w tym przebiegu wykonana. Nie przypisujemy jej starszych zaliczeń V1.

Pierwsze przygotowanie fixture miało błąd asercji, a Cargo101 zgłosiło brak celu testu:
**to nie był behavioralRED**. Po naprawie przygotowania kompilacja trwała17min10s,
następnie rzeczywiście uruchomiono wskazany test i otrzymano powyższy wynik0/1.

## Rzeczywisty model — nie podniesiono limitów

Qwen3-8B Q4_K_M, SHA256
`d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785`,
**5027783488bajtów**, MATCH. Oficjalny pin Qwen `7c41481f57cb95916b40956ab2f0b139b296d974`.
Oficjalne archiwum llama.cppb11559 SHA
`a454867b975dec288f731c6d3589f132b0e356f7dbad8d711f48d231ee67d7d9`,
binarka serwera SHA
`16c4e5fc3e0a74400033d958722e487a797dbb51c676f5ce1ec9066f1673efd5`.

CPU4, kontekst8192, max96tokenów,temp0,bez GPU. Startup60s, pierwsza generacja60s,
idle30s, tura120s; pingi/metadane nie są postępem. Każda wersja ma osobny serwer,
uruchomiony kolejno po ukończeniu kompilacji, bez czasowych testów UI.

| Wariant surowego HTTP, poza Core | Wynik |
|---|---|
| Obecny tekst bez odrzuconej notyV1 |0odpowiedzi,1timeout pierwszej generacji60061ms,2niewykonane |
| Laboratoryjny host_lookup |0odpowiedzi,1timeout60057ms,2niewykonane |
| Osobna diagnoza minimalna,8tokenów limitu |„GOTOWA”,5tokenów; pierwszy6489ms, całość11728ms, stop |

W ostatnim pomiarze serwer raportuje22tokeny promptu,6467,87ms/3,40tok/s prefill
i5231,80ms/0,76tok/s generacji. Ten krótki pomiar potwierdza działanie serwera i odczytu
SSE; **nie jest oceną #58, wszystkich person ani benchmarkiem docelowego GPU**.
Nie wolno liniowo przeliczać małego prefill na długą rozmowę. Przy obu pełnych
żądaniach nie otrzymano treści w60s, więc ocena semantyczna jest **NOT_EVALUATED**.
Nie uruchomiono pełnych30przypadków po niezaliczonym ekranie.

Serwery zamknięto; oba pierwsze wymagały SIGKILL po5s własnego SIGTERM, minimalny
zakończył się kodem0. Pierwszy transfer modelu przerwano świadomie sygnałem15,
zachowano prefiks i dokończono4walidowanymi zakresamiHTTP206; cały plik ma właściwySHA.
To nie błąd aplikacji ani porażka jakości modelu. Nie odczytano sekretów właściciela;
klucze testowe wygenerowano tylko dla lokalnych serwerów i zredagowano w logach.

## Następny rzeczywisty przyrost

1. Projekcja konkretnych wpisów wybranej gałęzi: nazwa znanego narzędzia i stan,
   powiązanie z rzeczywistym Replay/checkpointem; bez wymyślania ToolUse. Same sześć
   liczników nie jest wystarczającym dowodem skutku, testu lub przyczyny błędu.
2. Brak metadanych, legacyDone+intent, CLI, Running, odmowa/propozycja i Undo pozostają
   jawnie niepotwierdzone lub odrębne. Bez wejść/wyjść/etykiet/ścieżek innych gałęzi.
3. Testy izolacji, limitów, braków i rzeczywisty zapisTEMP → następna tura → Undo/restart
   oraz odmowa. Dopiero potem produkcyjny provider i model, bez dostrajania evals.
4. Na dostępnej maszynie z RTX2060 **12GB**:8k/16k, zimny/ciepły start, pięć person,
   Standard/Jarvis, dotychczasowe30sprzecznych historii i osobne nieznane wcześniej
   scenariusze. Chmura/hybryda wymagają dostępnego uprawnionego API; nie używamy
   przypadkowego endpointu. 1.5B pozostaje pomocniczy.

Zmiana zwykłego `app-*` nie wymaga dodatkowej zgody na chroniony `core-*`; wymaga
poprawnego projektu, testów i niezależnego przeglądu przed merge. Ten raport nie stawia
nowej bramki zgody dla app-chat. #22/#41/#48/#58/#77 nadal otwarte; starsze50+50
odpowiedzi z632e5d43 nie są nowymi wynikami na bfd69b97.

[Surowe dane, źródła i komendy](artifacts/2026-11-11-cloud/README.md) ·
[Odrzucony V1](2026-11-10-history-receipts.md) ·
[Wcześniejsze9B](2026-11-10-qwen35-9b-cpu.md).
