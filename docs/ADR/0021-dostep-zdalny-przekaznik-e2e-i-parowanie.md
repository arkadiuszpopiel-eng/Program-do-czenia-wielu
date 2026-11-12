# ADR 0021 — Dostęp zdalny: przekaźnik, kanał end-to-end i parowanie urządzeń

| Pole | Wartość |
|---|---|
| Status | Projekt zatwierdzony przez właściciela12.11.2026 z doprecyzowaniem poniżej |
| Data | 2026-10-09 |
| Powiązane | decyzja właściciela nr 1 z 2026-10-09; plan v6: `docs/plan/09-dostep-zdalny-i-bezpieczenstwo.md` (połączenie, parowanie, lista urządzeń, R1–R11), `03-roadmapa.md` (D.2–D.4, D.8, D.9); `docs/THREAT_MODEL.md` (S13, S14); ADR 0019, 0020, 0022 |
| Zmienia | model zagrożeń: dopisuje scenariusze R1–R11; reguła „żadnego nasłuchu” (S14, test F4-07) **zostaje i jest rozszerzona** na nowe moduły |

## Podpis właściciela12.11.2026

Właściciel zatwierdził konkretny [pakiet projektu](../reviews/2026-11-11-remote-design-review.md)
jako podpis ADR0019–0023. [Zapis decyzji i zakres](../reviews/2026-11-12-owner-decisions.md).
To zatwierdzenie architektury, bez odbioru implementacji; chronione poprawki Brokera
i polityk nadal wymagają konkretnego przeglądu.

## Kontekst

Właściciel zdecydował (decyzja 1), że telefon i panel WWW łączą się z komputerem przez przekaźnik
z szyfrowaniem end-to-end, komputer nie otwiera żadnego portu, a urządzenia paruje się kodem QR. Plan v5
i model zagrożeń zabraniają nasłuchu sieciowego: S13 (otwarty port CDP WebView2) i S14 (DNS rebinding,
przejęcie lokalnych usług) pozostają zagrożeniami. Dostęp zdalny musi więc być nową, osobną drogą z własnym
uwierzytelnieniem, a nie złagodzeniem starych reguł.

## Decyzja

| Element | Propozycja |
|---|---|
| Kierunek połączeń | **obie strony wychodzą** do przekaźnika (WebSocket po TLS); host nie nasłuchuje; zapora i router bez zmian |
| Reguła „żadnego nasłuchu” | pozostaje prawdziwa dla hosta; statyczny test obejmuje dodatkowo `remote-gateway`, `device-pairing`, `host-headless` |
| Kanał E2E | wewnątrz połączenia strony zestawiają kanał według wzorca Noise (biblioteka `snow`): wzajemne uwierzytelnienie kluczami urządzeń, utajnienie z wyprzedzeniem; ramki z licznikiem — powtórzenie odrzucane |
| Dobór wzorca Noise | Zatwierdzony kierunek dla klienta natywnego: `Noise_XX_25519_ChaChaPoly_SHA256`; biblioteka, klient WWW i implementacja wymagają przeglądu zgodnie z pakietem12.11 |
| Co widzi przekaźnik | losowy identyfikator hosta, adresy IP stron, rozmiary i czas ramek; nie zna treści, nazw, kluczy ani kont |
| Przekaźnik | moduł `remote-relay`: jeden plik wykonywalny / obraz kontenera; **właściciel może postawić własny**; limit połączeń na identyfikator hosta (R9) |
| Parowanie | kod QR ważny 2 minuty: adres przekaźnika, identyfikator hosta, klucz publiczny hosta, jednorazowy sekret; urządzenie tworzy własną parę kluczy w bezpiecznym magazynie (Android Keystore; w przeglądarce klucz dostępu WebAuthn) |
| Potwierdzenie | obie strony pokazują ten sam **krótki kod kontrolny** (np. 6 cyfr); **pierwsze urządzenie** właściciel potwierdza fizycznym kliknięciem w oknie Brokera na komputerze — tego kroku nie da się zrobić zdalnie |
| Kolejne urządzenia | można dodać z już sparowanego; wymaga świeżej biometrii i trafia do dziennika jako zdarzenie wysokiego ryzyka |
| Lista urządzeń | Ustawienia → Urządzenia: nazwa, rodzaj, ostatnie połączenie, zakres uprawnień; **Odwołaj** (natychmiast zrywa kanał i usuwa klucz), **Odwołaj wszystkie**, zmiana zakresu; odwołanie działa z każdego sparowanego urządzenia i z panelu WWW |
| Klienty natywne później | opcjonalnie połączenie bezpośrednie przez iroh 1.x (QUIC z przebijaniem NAT) dla Androida i drugiego komputera; przeglądarka tego nie potrafi, więc WebSocket zostaje wspólną bazą; warstwa szyfrowania ta sama niezależnie od drogi |

Nowe scenariusze zagrożeń R1–R11 (przekaźnik przejęty, podsłuch, kradzież telefonu, podmiana kodu QR, fałszywy
panel, przejęcie hostingu panelu, wstrzyknięcie poleceń, zalanie żądaniami, stara aplikacja, dane w telefonie)
i ich obrony są w dokumencie 09; po podpisie przechodzą do `THREAT_MODEL.md` (zadanie D.8).

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Otwarty port na komputerze (przekierowanie na routerze, UPnP) | powierzchnia do skanowania z Internetu; łamie S13/S14 i test „żadnego nasłuchu” |
| Sam TLS do przekaźnika, bez E2E | przekaźnik widziałby treść rozmów i zgody; przejęcie przekaźnika = przejęcie Alfy (R1) |
| Gotowa usługa tunelująca albo VPN strony trzeciej jako warunek | zależność od cudzego konta i klienta; przeglądarka na obcym komputerze nie zainstaluje VPN |
| Wyłącznie połączenia bezpośrednie (iroh) | przeglądarka nie potrafi; potrzebna wspólna baza dla trzech klientów |
| Parowanie hasłem lub kontem w chmurze | hasła da się wyłudzić; konto oznacza serwer, który zna użytkowników — sprzeczne z „przekaźnik nic nie wie” |
| Parowanie bez kodu kontrolnego | podmieniony kod QR pozwoliłby wpiąć obce urządzenie (R5) |
| Pierwsze urządzenie wpuszczane zdalnie | nie istnieje jeszcze żaden zaufany klient zdalny; początek zaufania musi być przy komputerze |

## Konsekwencje

- Kod powstaje dopiero po podpisie ADR 0019–0023 (zadanie D.0).
- Testy wymagane przed włączeniem: „przekaźnik nie widzi treści” (przekaźnik testowy zapisuje wszystko, znany
  tekst nie występuje w zapisie); dowód od urządzenia odwołanego odrzucony i kanał zerwany w czasie poniżej
  progu (próg ustala właściciel); zestaw ataków na dostęp zdalny napisany przez sesję-recenzentkę i zamrożony
  w `evals/` (D.9).
- Przekaźnik z natury **nasłuchuje** — ale jest osobną usługą poza komputerem właściciela i poza Jądrem; reguła
  „żadnego nasłuchu” dotyczy hosta. Kto prowadzi przekaźnik (własny serwer czy dostawca chmurowy), rozstrzyga
  właściciel; koszty hostingu i domeny są po jego stronie.
- Przekaźnik zna adresy IP i metadane ruchu — to ograniczenie, które plan nazywa wprost; własny przekaźnik
  zmniejsza je do minimum.
- Przybliżona lokalizacja urządzenia z adresu IP na liście urządzeń ma być liczona lokalnie, bez usług
  zewnętrznych — sposób jest do ustalenia.
- Nowe zależności (`snow`, klient WebSocket, później `iroh`) wymagają wpisu w `docs/vendor/` i `cargo deny`.

## Jak cofnąć

- Dostęp zdalny jest wyłączony, dopóki właściciel nie sparuje pierwszego urządzenia; „Odwołaj wszystkie”
  i wyłączenie bramy przywracają stan sprzed tego ADR bez zmian w danych.
- Przekaźnik można zmienić (adres w ustawieniach hosta); urządzenia wymagają wtedy ponownego parowania albo
  przekazania nowego adresu kanałem E2E — do ustalenia w SPEC `device-pairing`.
- Warstwa szyfrowania jest oddzielona od drogi: zamiana WebSocket na iroh albo odwrotnie nie zmienia kluczy
  ani listy urządzeń.
