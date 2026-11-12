# Pierwszy format klient–host po zatwierdzeniu D.0

**PełneCI192 zakończone:**8/10, Windows1164/4; zachowane dwie porażki i przygotowane naprawy.
[Wyniki przypisane do źródła i ograniczenia](2026-11-12-ci192-and-fixes.md).

Właściciel zatwierdził projekt telefonu jako podpis ADR0019–0023 i pierwsze wyszukiwanie
w przeglądarce bezAPI. [Dokładny zakres](2026-11-12-owner-decisions.md).
Ten przyrost realizuje wyłącznie format polecenia przyszłego kanału, bez włączenia dostępu.

## Kod i granice

Nowa czysta biblioteka `lib-client-wire` (wyjątek lib-* z crates/README, bez logiki modułu):
ścisła ramka `{v,id,cmd,args}`, generator schemaRust→JSON→TS, obowiązkowy limit bajtów,
wersja połączenia, dodatni identyfikator do2^53−1 i odrzucanie powtórzeń/starszychID.
Błędy i Debug nie zawierają nazwy komendy ani argumentów. Limit rekurencjiserde pozostaje.
Zamknięcie dekodera jest nieodwracalne; zużytegoID nie można wykorzystać po późniejszej odmowie.
Negocjacja wybiera bieżącą lub poprzednią wersję, respektując minimum hosta.
Brak nowych wartości domyślnych budżetu i brak zmian tabeli local_only/polityk.

`app-api` ma już7993linie źródła; dwa wiersze wpięcia schematu dają7995, poniżej8000.
Parser jest odrębną biblioteką współdzieloną. Cargo.lock dodaje tylko pakiet workspace;
wersje, źródła i checksums wszystkich dotychczasowych paczek pozostały identyczne.

**Dekodowanie nie jest autoryzacją.** Brak rejestru/ACL/wykonawcy w bibliotece:
nieznana komenda jest zwykłym DTO do odrzucenia przez przyszłą bramę, nigdy dowodem zgody.
Przed dekodowaniem przyszły transport musi uwierzytelnić kanał E2E i urządzenie;
przed wykonaniem sprawdzić rejestr, ACL, local_only, typedDTO i Broker. Licznik lokalnego
połączenia nie zastępuje nonce kryptograficznego ani związania sesji. Pole approved
wewnątrz args pozostaje niezaufaną treścią. Nie dodano sieci/listenera, parowania,
kluczy, dowodu zgody, dispatchera ani zmiany Brokera. Panel telefonu nadal podglądem.

## Testy

BehawioralnyRED **6/4** → **10/0**; po testach granic/właściwości15/0.
Pełny app-api+lib-client-wire **55/0/0**. Clippyworkspace/all-targets **0 bez ostrzeżeń**,
fmt/graf zależności/lint/format/generatory **0**. Cargo-deny **4OK**, bazowe ostrzeżenia pozostają.
Typy obu pakietówUI **0błędów/0ostrzeżeń** po poprawnej instalacji zależności nowego worktree;
wcześniejszy błąd setupu i pierwszy błąd kompilacji testu zachowane w surowych logach.
[Archiwum, hashe źródeł i pełne komendy](artifacts/2026-11-12-client-wire/README.md).

CI19110/10 i Windows1153/4 dotyczą **372bfcd5 przed tym przyrostem**; nie przenosimy ich
na nowy parser. [Raport z rzeczywistą porażką Core8B i29NOT_RUN](2026-11-12-execution-records-personas.md).
Brak nowego odbioru Windows/telefonu/API/GPU/mikrofonu oraz świeżego niezależnego przeglądu.

## Następne zależności

Kontrakt i fake bramy, sprawdzenieACL/local_only po stronie hosta, związanie sesji,
odwołanie, połączenie z istniejącymEventLog i resync; potem przypięta biblioteka Noise,
własnyWSSrelay i pierwsze parowanie lokalne. Dowody każdej zgody i konkretne zmiany
Brokera nadal osobno do przeglądu. Zewnętrzna strona przeglądarki nie dostaje IPC Alfy.
Wybór ścieżki bezAPI jest zapisany; nie pytamy ponownie o dostawcęAPI przed tą ścieżką.
