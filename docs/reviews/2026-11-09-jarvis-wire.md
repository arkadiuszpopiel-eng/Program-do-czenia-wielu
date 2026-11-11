# Jarvis na przewodzie i surowa odpowiedź Bielika, #48

Diagnoza po nieudanych próbach zwięzłości #41/#43; kod produkcyjny, ustawienia, progi evals
oraz polityki bez zmiany. Nie dodajemy kolejnego wariantu instrukcji dopasowanego do jednego pytania.

## Osobne etapy

1. Istniejące ręczne testy `jarvis_live` uruchomiono na fałszywym serwerze HTTP:40 tur wbudowanych
   i10 Luny po restarcie katalogu, oba testy zaliczone (2,20s i osobny krótki przebieg Luny).
   `FAKE_LLAMA_LOG` zapisuje tylko syntetyczne ciało, bez nagłówków i argumentów procesu.
   Raporty atrapy zostały jawnie oznaczone jako fake. To **nie** drugie zaliczenie rzeczywistego modelu.
2. Wszystkie50 ciał mają system+user,96 tokenów, temperaturę0; nie mają `tools` ani `response_format`.
   Jarvis i właściwa forma imienia są w25/25 odpowiednich instrukcji. Wszystkie10 instrukcji Luny
   są identyczne w HTTP, zapisie routera i wcześniejszym raporcie rzeczywistego modelu.
3. Te same50 ciał odtworzono kolejno na rzeczywistym Bieliku1.5B Q8_0, llama.cpp b11559,
   CPU4, kontekst8192, jedna kolejka. Bez innych lokalnych testów, kompilacji i obciążeń czasowych.
   05:00:05–05:03:56UTC,231,27s razem ze startem; to opis przebiegu, nie porównawczy benchmark.
   Wszystkie50 odpowiedzi zakończone `stop`, bez obcięcia i błędu transportu.
4. `/apply-template` dla Standard/Jarvis Luny zachowuje pełny system i wiadomość użytkownika
   w rzeczywistym szablonie serwera (2/2). Nie sprawdzono w tym kroku wszystkich10 różnych promptów.

## Wynik i granica diagnozy

Surowa odpowiedź HTTP, **przed dekoderem aplikacji**, nadal ma25/25 prawidłowych imion Jarvisa,
lecz **0/25 jednego zdania**. Są to te same dwa zdania co w25 odpowiedziach Jarvisa z #41/#43.
Zatem aplikacja nie dzieli w tym przypadku jednego zdania modelu na dwa, a serializacja nie usuwa
instrukcji zwięzłości. Nie dowodzi to ogólnej poprawności dekodera ani jakości modelu w innych zadaniach.

Porównanie dosłowne wszystkich tekstów z wcześniejszymi próbami daje **45/50**, a nie50/50.
Pięć Standard Luny ma „Luna, jestem gotowa do pomocy.” zamiast „…do rozmowy i pomocy.”.
Oba warianty mają jedno zdanie. Pierwotna asercja pełnej zgodności50/50 była nieudana;
raport zachowuje każdą różnicę. Tutaj Luna następuje po40 turach wbudowanych, wcześniej startowała
na osobnym serwerze. Przyczyna różnicy pozostaje otwarta; nie przypisujemy jej dekoderowi ani cache
bez dodatkowej kontrolowanej próby. Standard Bety/Gamy nadal ma10/10 błędnych form Betta/Gamma.

Niskie `input_tokens` z raportu rdzenia nie dowodzi braku promptu: kontrakt `Usage` oznacza
wejście poza cache. Na przykład powtórzenie Standard Luny ma w surowym HTTP `prompt_tokens=544`
i `cached_tokens=543`; po odjęciu pozostaje1. Pełny system nadal występuje w żądaniu.

Następne kroki: różnorodne zadania z niezależnym kryterium długości, osobno zimny/ciepły start
oraz porównanie odpowiedniejszego modelu na docelowym sprzęcie. Zestawy evals pozostają zamrożone.
Mikrofon, Windows i sprzęt właściciela nie są przedmiotem tej próby.

## Dowody i odtworzenie

[Surowe artefakty](artifacts/jarvis-wire-2026-11-09/summary.json): ciała HTTP JSONL,
nieprzetworzone teksty/usage/finish, dwa szablony i skrypt orkiestracji z SHA256 plików.
Publiczna stała fixture uwierzytelnia wyłącznie testowy serwer127.0.0.1; nie jest kluczem z konfiguracji
Alfy ani tokenem właściciela. Nie odczytywano sekretów ani tokenów CLI.

Dla tego samego Cloud skopiować zawartość katalogu artefaktów do
`/workspace/scratch/wire-diagnosis/`, zainstalować wskazane, zweryfikowane silnik i model,
następnie wykonać `python3 /workspace/scratch/wire-diagnosis/replay.py` bez konkurujących obciążeń.
Python tylko steruje procesem/HTTP; model wykonuje natywne llama.cpp. Skrypt zachowuje termin07:00UTC;
po nim odmawia uruchomienia. Zmiana terminu wymaga późniejszej instrukcji właściciela.
