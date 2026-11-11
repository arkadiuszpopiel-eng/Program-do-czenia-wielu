# Postęp długich argumentów narzędzia — #77, 10.11.2026

Kod69e336560ad0d6eade8a98fc3b30269fecb1dbf9 zachowuje postęp generacji przed buforującą
kopertą narzędzia. Przestój mierzymy od rzeczywistych niepustych delt, a gotowe wyjście
i after_output nadal od dekodera. Fragment JSON nie potwierdza wywołania lub wykonania akcji.

## Dlaczego potrzebna była druga korekta

Pierwszy przyrost8116740d/b85c5371 odróżnił pingi od zdekodowanej treści. Przegląd wskazał
przypadek kodowania: model może długo generować poprawne argumenty write_code, a EnvelopeGuard
czeka z ich ujawnieniem do walidacji całego obiektu. Zegar po dekoderze mógł więc przerwać
generację, mimo że kolejne tokeny przychodziły prawidłowo. Nie podniesiono limitów.

Reproduktor na b85c5371: **RED0/2**, kod101. Niezależnie sprawdzono poprawność składanego
JSON przed testem; zawiera funkcję Rust. Pierwszy przypadek generuje sześć porcji co100ms,
przy first-token300ms/idle200ms; dotychczas kończył się błędem FirstToken zamiast poprawnym
wywołaniem. Drugi daje początek JSON, następnie same pingi: powinien zakończyć się Idle,
z after_output=false i bez gotowego wywołania. Wartości są skalą fixture, nie progami evals.

## Końcowe zachowanie

- WireCodec.event_progress ma domyślne None: dotychczasowe kodeki zachowują swój tryb.
  LlamaCodec wskazuje niepuste delta.content, reasoning, refusal oraz nazwę/argumenty
  natywnego tool_calls. Puste pola, role, usage, komentarze i przydzielony sam identyfikator
  narzędzia nie są sygnałem generacji.
- Ten sygnał aktualizuje **wyłącznie zegar generacji**. Długie argumenty mogą powstawać
  dłużej niż first-token, jeżeli rzeczywisty postęp nie ustaje. Nie jest wymagane ukończenie
  całego narzędzia przed pierwszym terminem.
- TTFT i after_output pozostają oparte na niepustym zdekodowanym wyjściu. Niedokończona
  koperta i późniejsze pingi dają Idle/after_output=false, bez ToolCallStart/End.
- Dopiero kompletne, zwalidowane argumenty są wywołaniem modelu; do wykonania nadal
  potrzeba istniejącego runtime, Brokera i wykonawcy. Test nie wykonuje wygenerowanego kodu.
- Lokalna konfiguracja60/30s i wszystkie dotychczasowe wartości budżetów pozostają bez zmian.
  Chmurowe API zachowują domyślny transportowy tryb; anulowanie/Stop i terminalne zdarzenia
  działają nadal. Nie dodano retries ani zmian polityk, uprawnień, wag lub evals.

## Wyniki i zakres

Pełne compat + API + local, wszystkie features: **163/0/1**, w tym12 biblioteki,62 API,
89 lokalnego dostawcy;1 ręczna próba modelu pominięta. Clippy wszystkich celów/funkcji,
format i lint: **kod0**, bez ostrzeżeń. Oprócz dwóch regresji koperty są dwa testy protokołu
z14 wariantami metadanych i generacji. Nie doliczamy wariantów jako osobnych testów.

Pierwsza kompilacja helpera zwróciła101: parser.finish daje Option, a pomocnik przyjmował
Vec. Poprawiono go na IntoIterator; oba niezaliczone buildy zachowano. Dopiero końcowy
zestaw jest zaliczony. [Komendy, kody, źródła i SHA256](artifacts/local-tool-envelope-progress-validation.json).

Poprzednik b85c5371 ma pełne [CI186](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42912970574)
**10/10**: Linux3600/0/27, Windows3622/0/48, po1143 wyników celów,573Vitest i945Chromium;
[Windows109](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42912970490)
**4/4**, WebView2 **63/0/5**. [Pełne dane](artifacts/ci186-complete.json).
Ten nowy kod wymaga własnego CI, dopisywanego w PR24; poprzedniego przebiegu nie przerwano.

Nie powtarzano realnego modelu tylko po zmianie obsługi koperty. Replay Bety w źródle03680732
dał2/2 poprawnych tekstów, lecz ręczna kontrola zimnego czasu widocznego tekstu60s nie przeszła:
83,898s od rozpoczęcia adaptera wobec414ms po rozgrzaniu. Ten dodatkowy zegar obejmuje start
wag i nie jest zamrożonym limitem HTTP60s. Wynik ręcznego testu0/1 pozostaje zachowany;
nie zamieniamy go na zaliczenie istniejącego transportu lub ogólnej wydajności.

Modelowa semantyka sprzecznej historii #58 nadal niezaliczona. Rzeczywisty GPU12GB,
mikrofon, telefon, API i pełny cykl kodowania przez prawdziwy model pozostają do odbioru.
PR jest draft; bez merge przed zielonym CI i świeżym niezależnym przeglądem.
