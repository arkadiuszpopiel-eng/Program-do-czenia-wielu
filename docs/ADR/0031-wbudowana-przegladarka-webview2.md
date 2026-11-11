# ADR 0031 — Lekka przeglądarka wbudowana i wyszukiwanie WWW

| Pole | Wartość |
|---|---|
| Status | Propozycja do przeglądu, bez wdrożenia |
| Data | 2026-11-11 |
| Powiązane | #78, plan55, ADR0007/0009/0012, tools-browser, platform-apps |

## Problem i rekomendacja

Właściciel wskazał wbudowaną małą przeglądarkę jako kierunek dla WWW. Rekomendacja:
osadzony WebView2 korzystający z już dostarczanego runtime, uruchamiany dopiero po
otwarciu panelu. Nie dodajemy drugiego pakietu Chromium lub Electron. Osobny profil
Alfy, bez profilu użytkownika i bez tokenów CLI. To decyzja projektowa, nie pomiar RAM.

Przeglądarka wyświetla stronę i umożliwia odczyt/akcje przez istniejący BrowserPort.
Wyszukiwarka tworzy ranking wyników. Sama przeglądarka nie implementuje SearchPort:
`app-agents/src/sysnet.rs` nadal ustawia `search: None`. Nie zamykamy #78 samym panelem.

## Co już istnieje

- `tools-browser-contract/impl/fake`: otwarcie, odczyt drzewa dostępności i tekstu,
  klikanie, wpisywanie, zrzut, przewijanie, klawisze, wybór, nawigacja, czekanie,
  konsola i zamknięcie. Narzędzia obsługują Broker, taint Web, limity i kwarantannę.
- `platform-apps-contract::BrowserPort`: wspólny punkt integracji. Dzisiejszy Windows
  uruchamia osobny Edge/Chrome przez CDP po potoku, w Job Object. To nie jest jeszcze
  osadzony WebView2 i nie należy ogłaszać osadzenia jako wykonanej funkcji.
- Filtr domen nie jest kompletną zaporą dla WebRTC/UDP/WebTransport/DNS-prefetch;
  otwarta kwestia w SPEC platform-apps musi zostać zweryfikowana dla nowego backendu.

## Projekt powierzchni

1. Panel obok rozmowy: adres, tytuł, wstecz/dalej, odśwież/Stop, otwórz/zamknij kartę,
   odczytaj dla agentki, źródła i pobrania. Adres i tytuł to tekst, nie HTML.
2. Początkowo jedna aktywna karta na sesję. Nieaktywna zamrażana, gdy runtime pozwala;
   zamknięcie sesji i kill-switch zamykają jej przeglądarkę i unieważniają zgody hostów.
   Liczbę kart i próg pamięci ustali pomiar, bez zmiany obecnych budżetów.
3. Zewnętrzna strona w osobnym WebView bez Tauri IPC, mostka JS do poleceń aplikacji,
   sejfu, lokalnych zasobów aplikacji i dziedziczonych uprawnień okna głównego.
   Sam iframe w uprzywilejowanym oknie nie jest tym backendem.
4. Niezmienniki BrowserPort pozostają: filtr przed każdym żądaniem, osobny profil,
   zablokowane hasła/autouzupełnianie, pobrania do kwarantanny, odczyt niezaufany.
   Obsługa nowych okien, przekierowań, uprawnień strony i Service Workerów wymaga testu.
5. Na telefonie panel odczytu źródeł jest dostosowany do małego ekranu; sterowanie
   przeglądarką hosta wymaga działającego transportu #69. Sam lokalny panel tego nie daje.

## Dodatki

Pierwszy zestaw to odczyt artykułu, cytowania ze źródłem i datą odczytu, tekst/drzewo
dostępności, konsola dla kodowania i pobrania. Wykorzystujemy istniejące narzędzia
oraz kontrakt wtyczek Wasm/WIT; nie otwieramy ogólnego API rozszerzeń Chrome.
Każdy dodatek ma manifest, deklarowane zdolności, zakres sesji, budżet, anulowanie
i wynik niezaufany. Zewnętrzny skrypt strony nie dostaje terminala ani IPC Alfy.

## Dwie drogi wyszukiwania

- Interaktywna: użytkownik wybiera znany silnik wyszukiwania w panelu; zapytanie trafia
  do niego dopiero przy jawnym wyszukaniu. Strona wyników może być odczytana jak inne
  niezaufane strony. Błąd CAPTCHA/blokada/brak wyników jest jawny; nie ma obietnicy
  stabilnego scrapera lub rankingu „światowej klasy”.
- Automatyczna: adapter prawdziwego dostawcy SearchPort, z jawną konfiguracją konta
  i budżetu. Rekomendacja do decyzji: oficjalne Brave Search API albo własny SearXNG;
  nie korzystamy z przypadkowej publicznej instancji. Klucz przez sejf, nigdy w URL.
  Wybór konta nie został potwierdzony i żadne prywatne zapytanie nie zostało wysłane.

## Kolejne przyrosty i odbiór

| Przyrost | Dowód przed uznaniem za gotowy |
|---|---|
| Backend WebView2 pod BrowserPort | kontrakt + fake + Windows: nawigacja, DOM, zrzut, Stop, zamknięcie, restart |
| Izolacja zewnętrznej strony | brak IPC/sejf/plików Alfy; popup/redirect/iframe/Service Worker/egress oraz awaria procesu |
| Panel desktop i dotyk | klawiatura, focus, powiększenie tekstu, 320px/pion/poziom, PL/EN, axe, reduced-motion |
| Pamięć | aplikacja bez panelu, pierwsza strona, długa sesja, zamrożenie i zamknięcie; working set i CPU na Windows |
| Wyszukiwanie | realne konto albo wybrany silnik, zachowane źródła, błędy401/429/5xx/timeout, jakość PL/EN plan55 |
| Dodatki | brak samoczynnej instalacji/rozszerzenia uprawnień, test kontraktu, anulowanie i brak wycieku danych |

Zmiany chronionych polityk/ACL wymagają osobnej konkretnej łatki i przeglądu człowieka.
Ta propozycja nie zmienia polityk, zależności, progów, istniejącego portu ani zgód.
Pełny odbiór wymaga Windows/WebView2, niezależnego przeglądu i zielonego CI.
