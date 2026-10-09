# 08 — Android 15 i nowsze

Aplikacja na telefon jest pilotem do Alfy na komputerze ze 100 % funkcji, a gdy komputer jest niedostępny —
pozwala rozmawiać przez ten sam model chmurowy (decyzja właściciela nr 3). Model lokalny w telefonie nie jest
planowany. Etap F w [`03-roadmapa.md`](03-roadmapa.md); wymaga etapów D i E.1.

Minimalna wersja: **Android 15 (API 35)**. Nowsze wersje bez zmian w rdzeniu — kod systemowy jest tylko
w `platform-android` i we wtyczkach Kotlin.

## Budowa

| Element | Rozwiązanie | Pewność / uwagi |
| --- | --- | --- |
| Powłoka | Tauri 2 mobile (Android stabilny od 2.0; bieżąca 2.12) | P; `crate-type` w `src-tauri/Cargo.toml` już to dopuszcza |
| Interfejs | ten sam kod Svelte co okno i panel WWW; układ z E.1 | |
| Rdzeń w telefonie | **nie cały `AppCore`** — tylko klient protokołu, `offline-client`, magazyn lokalny | host zostaje na komputerze |
| Transport | WebSocket do przekaźnika + kanał szyfrowany w Rust (`snow`); później opcjonalnie iroh | |
| Klucze | Android Keystore (sprzętowy, klucz wymaga biometrii przy każdym użyciu) | wtyczka Kotlin |
| Biometria | BiometricPrompt, klasa 3 (silna) | wtyczka Kotlin |
| Powiadomienia | FCM bez treści → pobranie przez kanał E2E; kanały powiadomień: zgody / zakończone zadania / błędy | alternatywa bez usług Google: stałe połączenie w usłudze pierwszoplanowej |
| Praca w tle | usługa pierwszoplanowa dla rozmowy głosowej i stałego połączenia | U — brak gotowej wtyczki Tauri; własna |
| Skan QR | aparat przez wtyczkę skanera | |
| Budowa i testy | APK/AAB w CI; emulator API 35 i 36; testy interfejsu na WebView Androida | |

## Zakres — pilot

| Funkcja | W telefonie |
| --- | --- |
| Rozmowa, sesje, projekty, wyszukiwanie | pełne |
| Agentki, zadania, oś czasu | pełne; postęp na żywo, powiadomienie po zakończeniu |
| Zgody | karta zgody + biometria; także z powiadomienia (akcja „Pokaż”) — nigdy samo „Zatwierdź” z belki bez biometrii |
| STOP WSZYSTKIEGO | zawsze widoczny; także jako akcja w stałym powiadomieniu podczas pracy agentek |
| Pliki | przeglądanie katalogu roboczego hosta, pobieranie na telefon, wysyłanie z telefonu (zdjęcie, dokument, udostępnianie z innych aplikacji) |
| Głos | mikrofon i głośnik telefonu; rozpoznawanie, model i synteza na hoście; „naciśnij i mów” jako tryb domyślny |
| Pamięć, ustawienia, modele, koszty | pełne (ustawienia maszyny dotyczą hosta) |
| Sterowanie komputerem | podgląd ekranu hosta i Przejmij / Pauza / Stop |
| Terminal | dostępny; klawiatura ekranowa z paskiem klawiszy specjalnych |
| Lista urządzeń | podgląd i odwoływanie innych urządzeń |

Funkcje specyficzne dla telefonu, które nie istnieją na komputerze: udostępnianie do Alfy z innych aplikacji,
zdjęcie z aparatu jako załącznik, kafelek szybkich ustawień „Zapytaj Alfę”, widżet ostatniej rozmowy
(dwie ostatnie — po pierwszym wydaniu).

## Tryb bez komputera (F.6)

Szczegóły i zabezpieczenia: [`09`](09-dostep-zdalny-i-bezpieczenstwo.md), „Praca bez komputera”. W skrócie:

1. Na komputerze właściciel włącza dla urządzenia „rozmowa bez komputera”. Klucz API wybranego dostawcy trafia
   kanałem E2E do Android Keystore.
2. Gdy host nie odpowiada, aplikacja pokazuje baner „Bez komputera” i rozmawia bezpośrednio z API — tym samym
   dostawcą i modelem, które są ustawione na komputerze.
3. Niedostępne: narzędzia, pamięć, pliki hosta, model lokalny, głos przetwarzany na hoście (zostaje dyktowanie
   klawiatury systemowej).
4. Tury zapisują się lokalnie (zaszyfrowane). Po powrocie połączenia host dopisuje je do historii jako gałąź.

`offline-client` używa tych samych adapterów co komputer: crate `providers-api-impl` i `lib-openai-compat`
są czystym Rust bez zależności systemowych, więc kompilują się na Androida. Do potwierdzenia przy F.6: TLS
i magazyn certyfikatów na Androidzie.

## Ograniczenia Androida, które wpływają na projekt

| Ograniczenie | Skutek | Rozwiązanie |
| --- | --- | --- |
| System zabija procesy w tle | połączenie z hostem zrywa się po zminimalizowaniu | powiadomienia push budzą aplikację; wznowienie „od `seq`” (B.2) |
| Uprawnienia czasu działania (mikrofon, powiadomienia, aparat) | prośba w chwili pierwszego użycia | wyjaśnienie przed systemowym oknem zgody |
| Optymalizacja baterii producentów | opóźnione powiadomienia | instrukcja wyłączenia optymalizacji; wskaźnik „ostatnie połączenie” |
| WebView systemowy w różnych wersjach | różnice w CSS (m.in. `light-dark()`, `:has()`) | minimalna wersja WebView sprawdzana przy starcie; testy na WebView z API 35 |
| Brak fontów Segoe / Cascadia | inny wygląd | `system-ui` (Roboto / font producenta) — reguła „fonty systemowe” zachowana |
| Dane w kopii zapasowej systemu | wyciek rozmów do chmury producenta | wyłączenie kopii dla danych aplikacji; klucze i tak nie są eksportowalne |

## Publikacja

Pierwsze wydania: plik APK podpisany kluczem właściciela, do pobrania i instalacji ręcznej (zgodne
z charakterem programu dla jednego właściciela). Sklep Google Play — decyzja właściciela (F.8): wymaga konta
dewelopera, deklaracji o danych i zgodności z regulaminem dla aplikacji AI.

## Kryteria akceptacji (szkic; progi ustala właściciel)

- `ACC-V6-android-01`: parowanie od skanu QR do pierwszej wiadomości w czasie poniżej progu.
- `ACC-V6-android-02`: scenariusz MVP wykonany w całości z telefonu.
- `ACC-V6-android-03`: zgoda z powiadomienia wymaga biometrii; bez niej Broker odrzuca (test z podmienionym dowodem).
- `ACC-V6-android-04`: aplikacja w tle 10 minut, potem powrót — brak utraconych zdarzeń.
- `ACC-V6-android-05`: host wyłączony → rozmowa przez API działa; host włączony → tury w historii na komputerze.
- `ACC-V6-android-06`: odwołanie urządzenia kasuje klucz API i dane rozmów przy najbliższym połączeniu.
- `ACC-V6-android-07`: czas od wciśnięcia STOP w telefonie do zatrzymania na hoście poniżej progu.
- `ACC-V6-android-08`: zużycie baterii w stanie bezczynności poniżej progu na dobę.
