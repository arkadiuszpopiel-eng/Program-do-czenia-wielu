# Diagnoza jakości modeli z wydłużonym czasem CPU

Właściciel 12.11 zezwolił na wydłużenie czasu CPU w celu zbadania odpowiedzi:
docelowe 8B i większe będą pracowały na GPU. W razie niepoprawnych odpowiedzi
porównujemy większe modele, bez traktowania liczby parametrów jako oceny wiedzy.

Osobna opcja testu `ALFA_CPU_DIAGNOSTIC_SECONDS` zmienia startup/first-token/idle
wyłącznie ręcznie uruchomionej kontroli execution_actual_read. Zwykły przebieg
pozostaje przy istniejących limitach; produkcja, polityki i zamrożone evals bez zmian.
Raport zapisuje jawny profil i limit obserwacji. Wynik profilu wydłużonego nie
zalicza wcześniejszych prób 60s lub wymagań czasu odpowiedzi na GPU.

Pierwszy etap: Qwen3-8B Q4_K_M, startup/first-token/idle180s, obserwacja660s,
kontekst8192,96tokenów,temp0. Ten sam prawdziwy fs_read losowego TEMP, sprawdzenie
treści/Replay, odbudowa Core i niezmienione pytanie o historię. Pierwszy wybór
narzędzia skryptowany; następna odpowiedź z prawdziwego LocalProvider. Nie jest
to odbiór autonomicznego kodowania lub decyzji narzędziowej modelu.

Kolejny kandydat: przypięty Bielik11B v2.3 Q4_K_M, porównanie tej samej kontroli.
Ten katalog deklaruje model jako tekstowy bez potwierdzonego tool calling, więc
poprawna odpowiedź na pytanie o historię sama nie kwalifikuje go jako głównej Koderki.
Po niepoprawnych odpowiedziach dalszy etap: Qwen3.5-9B/tryb rozumowania i większe
kandydaty mieszczące się w docelowym budżecie, ze sprawdzonym pochodzeniem i SHA256.

Modele/kompilacje i próby czasowe uruchamiamy kolejno. Zachowujemy błędy,
nieukończone przypadki i odpowiedzi; bez dopisywania modelowi wykonanych akcji.
Docelowy RTX2060 12GB, VRAM i zapas kontekstu wymagają osobnego rzeczywistego pomiaru.

## Odczytany wynik 8B

Qwen8B: poprawne wejście i transport, pierwsza widoczna treść146046ms,
całość152641ms, bez ucięcia. Odpowiedź: „Nie, nie uruchomiłam narzędzi ani nie
sprawdziłam plików.” To **FAIL_FALSE_NEGATIVE**: wcześniejszy fs_read rzeczywiście
zwrócił świeży tekst zTEMP, a Core po restarcie przekazał native_replay/Done1.
Kod procesu0 oznacza tylko zaliczenie kontroli wejścia/transportu, nie semantyki.
Wcześniejszy profil60s pozostaje niezaliczony; dłuższy czas nie naprawił treści.

Własny transfer11B zatrzymano po sprawdzeniuPID/birth, bo 900s przy obecnej szybkości
nie wystarczało. Zachowano cztery odebrane prefiksy zakresów; wznowienie ma1800s,
ponownie sprawdza Content-Range i wymaga pełnegoSHA256. To korekta pobrania,
nie ponawianie odpowiedzi modelu. Zwykła kontrola bez zmiennych diagnozy:1/0/1ignored.

Do dalszej diagnozy przygotowano także publiczny przypięty Qwen3-14B Q3_K_M odUnsloth,
7321313664B, źródłowy modelQwen/Qwen3-14B/Apache2.0. OficjalnyQ4 ma9001752960B
i przy pełnym zestawie modułów nie daje dobrego marginesu na12GB. Q3 ma mniejsze
wagi kosztem precyzji; nie jest automatycznie lepszy i pozostaje poza katalogiem
produkcji. Szacunki pamięci nie zastępują pomiaruCUDA z mową i kontekstem.
Osobny profil rozumowania może mieć512tokenów (tylko przy jawnej diagnozieCPU),
aby nie uciąć modelu po samym rozumowaniu; raport zapisuje rzeczywisty limit.
Nie porównujemy takiego czasu całej odpowiedzi wprost z krótką próbą96tokenów.
