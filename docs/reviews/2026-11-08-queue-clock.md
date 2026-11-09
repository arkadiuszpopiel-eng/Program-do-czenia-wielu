# Kolejka w koszu a zegar testu

Karta [#34](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/34), źródła testów
`c7956f66`, Linux Cloud. [Wyniki](artifacts/queue-clock-results.json).

Pełny Rust na988d13f3 miał **3485 zaliczonych / 1 nieudany / 24 pominięte**. Jedyna porażka:
`queued_message_of_a_trashed_session_waits_for_undo` widział0 wiadomości zamiast1. Harness daje
tylko300ms na Cofnij, a test sprawdzał wiele operacji na rzeczywistym zegarze. Po upływie tego
okna produkcja celowo usuwa sesję i kolejkę. Sprawdzenie źródeł nie potwierdziło wstępnej hipotezy
automatycznego, równoległego `flush_queue` po `set_online`.

Test scenariusza „Cofnij jest jeszcze dostępne” używa zegara Tokio wstrzymanego na czas tego
fragmentu. Zachowuje wszystkie asercje kolejki, trzech wywołań modelu, braku wywołania dla kosza
i odpowiedzi po przywróceniu. Nowy test sprawdza osobno wpis przed upływem299ms oraz jego usunięcie
po obsłużeniu tego samego terminu300ms. Produkcyjny kod, budżet300ms i timeouty pozostają dotychczasowe.

Pierwsza próba nowego przypadku: **3 zaliczone / 1 nieudany** — samo `advance` przesunęło czas,
lecz nie czekało na obsługę timera. Wirtualna sonda `sleep_until` tego samego terminu synchronizuje
odczyt stanu; nie dodaje czekania według czasu ściennego ani nie wydłuża terminu.

Końcowy cel `review`: **4/4**. Następnie **20 kolejnych powtórzeń pary scenariuszy, łącznie40
wykonań testów bez błędu**. Sąsiednie cele `review_queue` i `accounts_shared_queue` oraz Clippy
lib/tych celów sprawdzone oddzielnie; dokładne liczby w artefakcie. `cargo fmt` i rozmiary zaliczone.
Nie uznano wcześniejszego pełnego przebiegu za zielony po samym teście ukierunkowanym.

Logi: `/workspace/scratch/queue-clock-tests.log`, `queue-clock-tests-final.log`,
`queue-clock-stress.log`, `queue-adjacent-tests.log`, `queue-clock-clippy.log`.
Niezależny przegląd i Windows CI otwarte; nie jest to odbiór sprzętu. Wersja0.0.85, etap J w toku.
