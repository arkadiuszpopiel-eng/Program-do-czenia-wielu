# Przegląd kodu kamer w świeżym kontekście (2026-10-10)

Przegląd wg ADR 0016: osobna sesja bez kontekstu autorskiego, tylko poprawność, bezpieczeństwo i zgodność ze SPEC.
Zakres: `platform-contract/src/camera.rs`, `platform-windows-impl/src/camera/`, moduł `camera-hub`, polecenia
`cameras_*` i `attachments_add_camera` w `app-core`, wskaźnik i podgląd w interfejsie. **Kod przechwytywania nie
działał jeszcze na prawdziwej kamerze** (kamera laptopa odłączona sprzętowo) — przegląd był odczytem kodu.

## Ustalenia

| ID   | Waga   | Ustalenie                                                                                                                                | Stan                                                                                                                                   |
| ---- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| K-01 | ważne  | Znak kamery w belce świecił tylko przy stanie „działa”, a kamera jest aktywna już podczas otwierania (do 15 s)                           | **naprawione** — znak świeci przy „otwieram”, „działa” i „zamykam”                                                                     |
| K-02 | ważne  | Zatrzymanie zgłaszało „uśpiona”, zanim strumień został naprawdę zamknięty                                                                | **naprawione** — nowy stan „zamykam”; „uśpiona” dopiero po potwierdzeniu zamknięcia (`Registry::settle`)                               |
| K-03 | ważne  | „STOP WSZYSTKIEGO” czekał na zamknięcie kamer przed zatrzymaniem Brokera i agentek; zawieszony sterownik opóźniałby wszystko do ok. 20 s | **naprawione** — kamery zatrzymują się w osobnym zadaniu, żądania są unieważniane od razu, na zamknięcie czekamy najwyżej 2 s na końcu |
| K-04 | ważne  | Wątek zablokowany w `ReadSample` (zawieszony sterownik, odłączenie bez błędu) trzyma kamerę po zamknięciu strumienia                     | **otwarte** — wymaga `IMFMediaSource::Shutdown` z innego wątku i próby na sprzęcie                                                     |
| K-05 | ważne  | Porzucenie żądania klatki (przerwane polecenie) zostawiało licznik żądań w górze — kamera nigdy nie zasypiała                            | **naprawione** — rozliczenie żądania działa w osobnym zadaniu                                                                          |
| K-06 | drobne | Zawieszona kamera i częste żądania ustawiały kolejkę zadań blokujących                                                                   | **naprawione** — drugie żądanie w czasie otwierania dostaje odpowiedź „kamera właśnie się otwiera”                                     |
| K-07 | drobne | Rozmiar klatki od sterownika nie był ograniczony przed alokacją                                                                          | **naprawione** — limit `MAX_IMAGE_PIXELS` przed odczytem; długość bufora w ścieżce `Lock2D` nadal niesprawdzana                        |
| K-08 | drobne | Gdy sterownik nie poda kierunku wierszy, obraz RGB32 może wyjść odwrócony w pionie                                                       | **otwarte** — do sprawdzenia na prawdziwej kamerze                                                                                     |
| K-09 | drobne | Błąd zapisu przy wyłączaniu kamer zostawia w ustawieniach „włączone” na następny start                                                   | **otwarte** — błąd jest zgłaszany; do rozstrzygnięcia: ponawianie zapisu                                                               |

## Sprawdzone i poprawne (wg przeglądu)

- Równowaga inicjalizacji i zwalniania COM / Media Foundation na wątku kamery, także na ścieżkach błędów.
- Żądanie w toku w chwili wyłączenia kamer nigdy nie otwiera strumienia (epoka gniazda sprawdzana przed i po
  otwarciu oraz po odczycie).
- Wyłącznik główny, wyłącznik kamery, `stop_all`, odświeżenie listy i zmiana nazwy — bez drogi do otwarcia
  wyłączonej kamery.
- Usypianie naprawdę zamyka strumień; brak blokad trzymanych przez `.await`; awaria jednej kamery nie dotyka innych.
- Brak `unwrap` / `expect` / `panic` w ścieżkach produkcyjnych; komunikaty bez danych obrazu; klatka trafia na dysk
  tylko przez `attachments_add_camera`.

## Czego nie dało się ocenić

Zachowania prawdziwych kamer: wybór formatu (MJPG → RGB32), kierunek wierszy, kody błędów przy odłączeniu, pierwsza
ciemna klatka po otwarciu (automatyczna ekspozycja). Do sprawdzenia w zadaniu C3.2 na sprzęcie właściciela.
