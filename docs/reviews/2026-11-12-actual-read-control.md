# #58 — rzeczywista operacja przed pytaniem o historię

**Po integracji poprawek #83 i diagnostyki wyszukiwania:** dziewięć celów app-core
**17/0/2ignored**, Clippy app-core+app-chat/all-targets0 bez ostrzeżeń.
Nowy test i workflow mają identyczne bajty z wcześniej sprawdzonym źródłem.
Nie ponawiano rzeczywistej próby modelu; wcześniejsza porażka poniżej pozostaje.
[Komendy i hashe po integracji](artifacts/2026-11-12-actual-read-integrated/README.md).

Windows116 na5f265ba3 ukończył30/30transportu i wejścia, lecz wszystkie odpowiedzi
zaczynają się „Nie”. Dziesięć pozytywnych wpisówmacierzy ma tylko TurnMeta,
`tool:null` i `origin:turn_metadata_only`, bez Replay/akcji. Nie wystarczają do odbioru
rozróżniania prawdziwych działań. Nie zmieniono starych przypadków lub kryteriów.

Nowa osobna kontrola: skryptowany dostawca wybiera fs_read, a rzeczywisty wykonawcaCore
czyta świeży losowy tekst z pliku we własnymTEMP. ZwróconyToolResult musi go zawierać;
Replay musi zawierać dokładnie jeden fs_read. Następnie Core jest odbudowany nad tą
samą zaszyfrowaną historią i sejfemMemory; wyłączony katalog odbiera dostępne narzędzia,
lecz kolejne żądanie nadal ma prawdziwy fs_read/native_replay iRecordedDone1/Unverified0.
Nie przekazuje zawartości pliku. Pytanie jest tym samym niezmienionymQUERY co wcześniej.

Pierwsza tura to **skryptowany wybór narzędzia z rzeczywistym odczytem**, nie decyzja
Qwena. Dopiero następna tura pyta rzeczywistyLocalProvider. To kontrola ciągłości
historii przy zmianie dostawcy, nie odbiór autonomicznego kodowania lub uprawnieńWindows.

Dane w tej ścieżce zaliczone1/1. Regresje actual_read/context/live: **5/0/2ignored**,
Clippy app-core/all-targets **0 bez ostrzeżeń**. Jedna próbaCPU/Qwen8B/Delta/Standard:
wejście poprawne,0nowych narzędzi, **0tekstu/FirstToken60s**, całość99337ms, kod101.
Semantyka **NOT_EVALUATED**. Nie podniesiono limitów i nie ponawiano modelu.
Model i serwer zweryfikowaneSHA; przed próbą czytano wagi, więc brak deklaracji cold-cache.
[Surowy raport, komendy, błędy, źródła i hashe](artifacts/2026-11-12-actual-read/README.md).

WorkflowWindows przygotowany do osobnej próby actual-read-screen.json po dotychczasowej
macierzy30. Na źródlea2b47cf2 błąd wcześniejszego kroku pomija tę próbę; ten wynik
pozostaje w historii. Dalsza korekta pozwala wykonać niezależną kontrolę po ukończonej
kompilacji także wtedy, gdy stara macierz nie przejdzie. Anulowanie, harmonogram
lub niezaliczony build nadal ją pomijają. Kroki są kolejne, nigdy równoległe.
Bez nowych progów/timeoutów lub continue-on-error.
Raporty silników i starejmacierzy pozostają oddzielne i zachowane.

Korekta raportowania po przeglądzie: brak pliku sam w sobie nie dowodzi pominięcia.
Krok ma własny ID, a podsumowanie odczytuje jego rzeczywisty outcome. NOT_RUN tylko
dla skipped; przy pozostałych stanach brak raportu jest wynikiem niepotwierdzonym.
Nie zmieniono komend, limitów, wyzwalaczy lub współbieżności. Powyżej opisano
jedyną zmianę warunku wykonania: niezależność od wyniku starszej macierzy.
Sprawdzenie struktury całego YAML:0; lokalny PowerShell niewykonany (brak pwsh).
[Zapis kontroli](artifacts/2026-11-12-actual-read-integrated/report-status-structure.json).

**#58 pozostaje otwarte.** Brak zaliczonej odpowiedzi na tej mocniejszej kontroli;
brak odbioru wszystkich person, pięciu trybów, GPU12GB, telefonu, prawdziwegoAPI i mikrofonu.
Kolejny krok: odczytać surową odpowiedź Windows, ocenić względem prawdziwego Replay,
potem pełne persony/profile/Undo/odmowa. Brak nowej poprawki produkcyjnej lub przepisywania
odpowiedzi modelu. Chronionych modułów/polityk/sekretówCLI/evals nie zmieniono.
