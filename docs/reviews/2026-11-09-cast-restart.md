# Trwała obsada rozmowy, usterka #37

Luna po restarcie traciła rolę Czytelniczki. Usunięcie ręcznego ponownego przypisania z testu
odtworzyło błąd: **0 zaliczonych/1 nieudany,2,08s**. Po naprawie ten sam test zachowuje
rolę, narzędzia wyłącznie do odczytu, profil Jarvis i dotychczasowe uprawnienia bez włączania mikrofonu.

`CastStore` zapisuje zatwierdzoną obsadę przed zmianą RAM i zdarzeniem. Adapter `app-store`
używa append-only tabeli `app_casts` (migracja0004) w istniejącej szyfrowanej bazie rozmowy.
Odtworzenie następuje po katalogu własnych person/rol, waliduje całość przed zmianą stanu i nie
publikuje ponownie polecenia. Błąd zapisu pozostawia poprzednią obsadę. Import wielu sesji jest
sekwencją osobnych zapisów, bez obietnicy transakcji obejmującej wiele baz.

Usunięcie rozmowy usuwa bazę/klucz, a po powodzeniu także obsadę RAM. Cofalny kosz zachowuje
zapis. Jednorazowe odczyty zwalniają nieużywane połączenia; aktywna referencja zachowuje tę samą
instancjęDb. Pozwala to odtworzyć obsady bez trzymania wszystkich baz rozmów otwartych.

## Dokładne sprawdzenie lokalne

- **76 zaliczonych/0 nieudanych/0 pominiętych**:7 crate'ów person, sesji i magazynu,31 wyników celów.
- Rozszerzone trzy testy magazynu powtórzone **3/3** (bez doliczania ponownie do76).
- **17/17 integracji AppCore**:agents8,Jarvis5,własne persony2,scenario2 (w tym usunięcie rozmowy).
- Clippy dziewięciu zmienionych crate'ów, `--all-targets -D warnings`:**0 ostrzeżeń/2m03s**.
- Format0; graf zależności1809 krawędzi/0 naruszeń; rozmiary3501 plików/0 naruszeń,
  app-core pozostaje dokładnie8000 linii. Skrócono opisy przy niewielkim podłączeniu adaptera.
- Nowych crate'ów i zmian wersji zależności brak. Oba lockfile otrzymują tylko zależność
  app-store→personas-contract. Pełny `cargo metadata --locked` powłoki wymaga WindowsCI;
  lokalnie zaliczono wariant `--no-deps`. Początkowe błędy narzędzi/kompilacji zachowane w metadanych.

[Komendy, zakres i SHA256 logów](artifacts/cast-restart-validation.json).
**To Linux z atrapą dostawcy/OS i prawdziwą bazą SQLCipher, nie próba modelu ani mikrofonu.**
Pełny nowy Rust Linux/Windows musi przejść w CI. Poprzednie3495/0/25 i3521/0/46 dotyczą
wcześniejszej produkcji; nie przypisujemy ich tej zmianie.

## Pozostałe znane usterki

Etykiety #53 mają już905/905 Chromium i63/0/5 Windows/WebView2 na a3041b6f;
zweryfikowany obraz Windows pokazuje „narzędzia systemowe”, „analiza obrazu” i „Kolor2”.
Bielik nadal wymaga naprawy zwięzłości/tożsamości; nowe30 odtworzeń trzech wariantów promptu
nie spełniło warunku jednego zdania dla wszystkich person, wariantów nie wprowadzono do produktu.
HTTP504 powtórzył się w silnikach85 (sześć archiwów,0/1/2filtered,207,90s), pomimo zaliczenia84.
CloudHEAD tego samego archiwum llamaCPU zwrócił200, co nie jest zaliczeniem pobrania Windows.

Właściciel odpowiedział „ok” na konkretne pytanie o łatkę importów #51. Zatwierdzenie obejmuje
wyłącznie KillReason/KillSwitch podcfg(windows); jej integracja i pełnyClippy są kolejnym krokiem.
Nie obejmuje to scalenia ani szerszych zmian Brokera. Po obecnych naprawach: raport, zapis i STOP.
