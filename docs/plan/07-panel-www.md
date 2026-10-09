# 07 — Panel WWW (sterowanie przez Internet)

Panel WWW to Alfa w przeglądarce: ta sama rozmowa, agentki, pliki i zgody co w oknie na komputerze, dostępne
z dowolnego miejsca. Komputer pozostaje „domem” — panel jest pilotem (decyzja właściciela nr 3).
Projekt bezpieczeństwa: [`09-dostep-zdalny-i-bezpieczenstwo.md`](09-dostep-zdalny-i-bezpieczenstwo.md).
Etapy: D (fundament) i E (panel) w [`03-roadmapa.md`](03-roadmapa.md).

## Dlaczego to jest wykonalne małym kosztem

Interfejs Alfy już dziś działa w zwykłej przeglądarce — tak powstają zrzuty ekranu i testy E2E. Kod interfejsu
zna tylko interfejs `AlfaClient`; biblioteki Tauri są importowane w dwóch miejscach. Panel WWW to:

1. trzecia implementacja `AlfaClient` — przez kanał szyfrowany zamiast Tauri IPC (D.6),
2. układ na wąski ekran (E.1),
3. parowanie i logowanie (E.3),
4. opakowanie PWA i hosting (E.2, E.7).

## Jak to wygląda dla użytkownika

1. Na komputerze: Ustawienia → Urządzenia → „Dodaj przeglądarkę”. Pojawia się kod QR i krótki kod.
2. W przeglądarce: adres panelu → skan kodu telefonem albo wpisanie krótkiego kodu → przeglądarka tworzy klucz
   dostępu (odcisk palca, PIN urządzenia albo klucz sprzętowy).
3. Potwierdzenie kodu kontrolnego na komputerze. Od tej chwili ta przeglądarka jest na liście urządzeń.
4. Kolejne wejścia: adres panelu → klucz dostępu → Alfa. Bez haseł.

Na cudzym komputerze: tryb „sesja jednorazowa” — parowanie ważne do zamknięcia karty, nic nie zostaje
w przeglądarce (do rozpisania w SPEC `web-panel`).

## Zakres funkcji

Wszystko, co host ogłasza w mapie zdolności, z odpowiednikami dla działań lokalnych:

| W oknie na komputerze | W panelu |
| --- | --- |
| Okno wyboru pliku systemu | przeglądarka plików hosta (w zakresie katalogu roboczego) albo wysłanie pliku z urządzenia |
| „Otwórz”, „Pokaż w Eksploratorze” | „Pobierz na to urządzenie”, podgląd przesyłany strumieniem |
| Okno Brokera | karta zgody w panelu + potwierdzenie kluczem dostępu (E.4) |
| Skróty globalne | brak; skróty w obrębie karty przeglądarki działają |
| Głos | mikrofon i głośnik przeglądarki; rozpoznawanie i synteza na hoście — po F.5, jeśli opóźnienie pozwoli |
| Terminal | dostępny; dane terminala tylko kanałem E2E, nigdy w logach (reguła już obowiązuje) |
| Kill-switch | przycisk STOP zawsze widoczny; polecenie o najwyższym priorytecie |

## Technika

| Element | Rozwiązanie | Uwagi |
| --- | --- | --- |
| Kod | ten sam pakiet interfejsu (`apps/desktop/ui`), nowy punkt wejścia i cel budowy | bez rozgałęziania komponentów; różnice przez mapę zdolności |
| Transport | WebSocket do przekaźnika + kanał szyfrowany w przeglądarce (WebCrypto / WASM) | przeglądarka nie obsłuży iroh — stąd WebSocket jako wspólna baza |
| Klucze | klucz dostępu WebAuthn związany z domeną panelu; klucz kanału w IndexedDB jako nieeksportowalny `CryptoKey` | |
| PWA | manifest, instalacja na ekranie głównym, pamięć podręczna powłoki aplikacji | treść rozmów nie trafia do pamięci podręcznej service workera |
| Powiadomienia | Web Push bez treści: „Alfa czeka na Twoją zgodę” → po otwarciu pobranie przez kanał E2E | iOS poza zakresem planu |
| Typy | generowane z Rust (B.1) | wspólne z oknem Tauri i Androidem |
| Markdown | host renderuje do HTML w Rust (reguła twarda) — panel tylko wstawia wynik | bez zmian |
| Fonty | systemowe; bez fontów webowych | reguła twarda obowiązuje też w panelu |

## Hosting panelu

Panel to pliki statyczne. Dwie opcje (do decyzji właściciela przy E.7):

| Opcja | Zalety | Wady |
| --- | --- | --- |
| Własna domena właściciela + przekaźnik na tym samym serwerze | pełna kontrola; zagrożenie R7 ograniczone do własnego serwera | utrzymanie serwera i certyfikatu |
| Hosting statyczny u dostawcy + osobny przekaźnik | najprostsze | zaufanie do dostawcy co do integralności plików |

W obu przypadkach: ścisła polityka CSP (bez skryptów z zewnątrz, bez `eval`), integralność zasobów,
wersjonowane wydania, nagłówki izolacji. Kod panelu jest publiczny i nie zawiera sekretów — bezpieczeństwo
opiera się na kluczach urządzeń, nie na tajności strony.

## Układ na wąski ekran (E.1)

Dziś minimalne okno to 400 × 500 px, a poniżej 720 px panele stają się arkuszami. Do zrobienia: szerokość od
360 px, obsługa dotyku (cele co najmniej 44 px), klawiatura ekranowa nieprzesłaniająca pola pisania, gesty
cofania, bezpieczne marginesy. Ta praca służy też Androidowi i oknu Tauri w małym rozmiarze.

## Tryb bez komputera

W panelu **wyłączony** do rozstrzygnięcia ADR 0023 (przeglądarka nie ma magazynu kluczy klasy telefonu).
Bez połączenia panel pokazuje stan hosta i ostatnio widziane rozmowy tylko wtedy, gdy użytkownik włączył
„pamiętaj na tym urządzeniu” — domyślnie nic nie zostaje.

## Kryteria akceptacji (szkic; progi ustala właściciel)

- `ACC-V6-web-01`: parowanie przeglądarki od kodu QR do pierwszej wiadomości w czasie poniżej progu.
- `ACC-V6-web-02`: scenariusz MVP wykonany w całości z panelu (z odpowiednikami działań lokalnych).
- `ACC-V6-web-03`: przerwa w sieci 30 s w trakcie odpowiedzi — po wznowieniu brak utraconych zdarzeń.
- `ACC-V6-web-04`: axe — zero naruszeń krytycznych i poważnych na 360 px i 1280 px, w trzech stylach.
- `ACC-V6-web-05`: zapis ruchu przekaźnika nie zawiera znanego tekstu wiadomości.
- `ACC-V6-web-06`: po odwołaniu przeglądarki jej karta traci dostęp w czasie poniżej progu.
- `ACC-V6-web-07`: polecenie spoza uprawnień klienta — odmowa hosta (test z podmienionym klientem).
