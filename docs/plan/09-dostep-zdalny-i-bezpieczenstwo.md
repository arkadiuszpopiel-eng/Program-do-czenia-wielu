# 09 — Dostęp zdalny i bezpieczeństwo

Ten dokument opisuje projekt. Kod powstanie dopiero po podpisaniu ADR 0021–0023 przez właściciela, bo zmienia
ustalenia modelu zagrożeń ([`../THREAT_MODEL.md`](../THREAT_MODEL.md): S13, S14) i Jądra bezpieczeństwa.

## Założenia (decyzje właściciela z 2026-10-09)

1. Połączenie przez **przekaźnik** z szyfrowaniem end-to-end. Komputer nie otwiera portu.
2. **Każdą** zgodę, także wysokiego ryzyka, można wydać zdalnie.
3. Klient zdalny ma **100 % funkcji**; bez komputera rozmawia przez model chmurowy.

## Jak działa połączenie

```
telefon / przeglądarka                przekaźnik                      komputer (host)
        │   ── połączenie wychodzące ──▶  │  ◀── połączenie wychodzące ──   │
        │                                 │                                 │
        │◀═══════════ kanał szyfrowany end-to-end (klucze tylko na końcach) ═══════════▶│
```

- Obie strony **wychodzą** do przekaźnika (WebSocket po TLS). Zapora i router zostają bez zmian; reguła
  „żadnego nasłuchu” z testu F4-07 pozostaje prawdziwa **dla komputera** i zostaje rozszerzona na nowe moduły hosta.
  Jedynym elementem, który nasłuchuje, jest przekaźnik — osobna usługa na serwerze, poza komputerem właściciela.
- Wewnątrz połączenia strony zestawiają kanał według wzorca Noise (biblioteka `snow`; konkretny wariant wzorca
  i sposób użycia kluczy dostępu WebAuthn w przeglądarce są do ustalenia w ADR 0021 i 0022): wzajemne
  uwierzytelnienie kluczami urządzeń, utajnienie z wyprzedzeniem. Przekaźnik przenosi nieczytelne ramki.
- Przekaźnik zna tylko: identyfikator hosta (losowy), adresy IP stron, rozmiary i czas ramek. Nie zna treści,
  nazw, kluczy ani kont. Jest jednym plikiem wykonywalnym / obrazem kontenera — właściciel może postawić własny.
- Klienty natywne (Android, drugi komputer) mogą w przyszłości łączyć się bezpośrednio przez iroh (QUIC
  z przebijaniem NAT i własnym przekaźnikiem); przeglądarka tego nie potrafi, więc WebSocket zostaje wspólną
  bazą. Warstwa szyfrowania jest ta sama niezależnie od drogi.

## Parowanie urządzenia

1. Na komputerze: Ustawienia → Urządzenia → „Dodaj urządzenie”. Okno pokazuje kod QR ważny 2 minuty: adres
   przekaźnika, identyfikator hosta, klucz publiczny hosta, jednorazowy sekret.
2. Telefon skanuje kod (przeglądarka: wpisanie krótkiego kodu albo skan z telefonu). Urządzenie tworzy własną
   parę kluczy w bezpiecznym magazynie (Android Keystore; w przeglądarce klucz dostępu WebAuthn).
3. Obie strony pokazują ten sam **krótki kod kontrolny** (np. 6 cyfr). Właściciel potwierdza zgodność
   **na komputerze** — fizycznym kliknięciem w oknie Brokera. To jedyny krok, którego nie da się zrobić zdalnie:
   pierwsze urządzenie musi zostać wpuszczone lokalnie.
4. Host zapisuje urządzenie: nazwa, klucz publiczny, data, zakres uprawnień. Sekret jednorazowy wygasa.

Kolejne urządzenie można dodać z już sparowanego (decyzja 2: wszystko zdalnie), ale wymaga to świeżej
biometrii i trafia do dziennika jako zdarzenie wysokiego ryzyka.

## Lista urządzeń

Ustawienia → Urządzenia pokazują dla każdego: nazwę, rodzaj, ostatnie połączenie, przybliżoną lokalizację
z adresu IP (liczoną lokalnie, bez usług zewnętrznych — do ustalenia), zakres uprawnień. Działania:
**Odwołaj** (natychmiast zrywa kanał i usuwa klucz), **Odwołaj wszystkie**, zmiana zakresu.
Odwołanie działa też z każdego innego sparowanego urządzenia i z panelu WWW — na wypadek kradzieży telefonu.

## Zgody zdalne

Dziś zgodę przyjmuje wyłącznie okno Brokera i wymaga `PhysicalInputProof`: dowodu, że kliknięcie pochodzi
od człowieka przy komputerze (albo Windows Hello). Interfejs w WebView nigdy nie zatwierdza.

Projekt (ADR 0022) dodaje drugi rodzaj dowodu, nie osłabiając pierwszego:

```rust
// szkic — kontrakt należy do Jądra; zmiana tylko po przeglądzie właściciela
pub enum ApprovalProof {
    PhysicalInput(PhysicalInputProof),   // jak dziś
    PairedDevice(DeviceApprovalProof),   // nowe
}
pub struct DeviceApprovalProof {
    pub device_id: DeviceId,
    pub request_hash: [u8; 32],   // skrót dokładnie tego żądania: co, na czym, poziom ryzyka
    pub user_verified: bool,      // urządzenie potwierdziło biometrię / PIN przy TYM podpisie
    pub signed_at: Timestamp,
    pub signature: Signature,     // kluczem z bezpiecznego magazynu urządzenia
}
```

Reguły:

- Podpis obejmuje **skrót konkretnego żądania**. Podpisu nie da się przenieść na inne działanie ani powtórzyć.
- Broker sprawdza podpis kluczem z listy urządzeń — sam, nie ufając bramie ani interfejsowi. Brama klientów
  tylko przenosi dowód; nie może go wytworzyć.
- `user_verified` musi być prawdziwe: klucz w magazynie Androida jest skonfigurowany tak, że każde użycie
  wymaga biometrii; w przeglądarce WebAuthn z `userVerification: required`.
- Żądanie pokazane na telefonie zawiera to samo co okno Brokera: kto prosi, co, na czym, ryzyko, czy da się cofnąć.
- Zgoda zdalna jest w dzienniku oznaczona urządzeniem, z którego przyszła.

### Zabezpieczenia przy „wszystko zdalnie”

Decyzja właściciela dopuszcza zdalne zatwierdzanie wysokiego ryzyka. To wygodne, ale przejęty odblokowany
telefon staje się wtedy kluczem do komputera. Plan ogranicza to tak, żeby nie odbierać wygody:

| Zabezpieczenie | Jak działa | Można wyłączyć? |
| --- | --- | --- |
| Biometria przy każdej zgodzie | sam odblokowany ekran nie wystarcza | nie |
| Wysokie ryzyko: pełny opis i przytrzymanie przycisku | chroni przed przypadkowym dotknięciem | nie |
| Powiadomienie na komputerze i pozostałych urządzeniach o zgodzie wysokiego ryzyka | właściciel widzi, że coś zatwierdzono | tak |
| Limit zgód wysokiego ryzyka w czasie (np. 5 na 10 minut) | hamuje serię działań po przejęciu | tak (ustawienie) |
| Zakres per urządzenie | urządzeniu można odebrać prawo do wysokiego ryzyka | tak — domyślnie pełny, zgodnie z decyzją |
| Odwołanie urządzenia z dowolnego klienta | natychmiastowe | nie |
| Twarde blokady z modelu zagrożeń | to, czego Broker nie pozwala nikomu, pozostaje zabronione | nie |

Rekomendacja sesji (do decyzji przy ADR 0022): zostawić domyślnie pełny zakres, zgodnie z decyzją, ale
pokazać przy parowaniu przełącznik „to urządzenie może zatwierdzać wysokie ryzyko”.

## Praca bez komputera

Gdy klient nie może połączyć się z hostem (komputer wyłączony, brak sieci po jego stronie):

- Interfejs pokazuje wyraźny stan „Bez komputera — rozmowa przez model w chmurze”. Narzędzia, pamięć, pliki
  i model lokalny są niedostępne (mapa zdolności).
- Rozmowa idzie bezpośrednio z urządzenia do dostawcy API — tego samego i z tym samym modelem, który jest
  ustawiony na komputerze. Ustawienia dostawcy i modelu klient zapamiętuje przy każdym połączeniu z hostem.
- **Klucz API** trafia na telefon tylko po osobnej zgodzie na komputerze („Pozwól temu urządzeniu rozmawiać bez
  komputera”), kanałem E2E, do magazynu kluczy Androida. Odwołanie urządzenia wysyła polecenie skasowania;
  ponieważ skradziony telefon może go nie odebrać, właściciel dostaje przy odwołaniu przypomnienie
  o wymianie klucza u dostawcy.
- **Panel WWW**: przeglądarka nie ma magazynu równie bezpiecznego jak telefon. Projekt zakłada, że tryb bez
  komputera w przeglądarce jest wyłączony, dopóki ADR 0023 nie rozstrzygnie przechowywania klucza
  (opcja: szyfrowanie kluczem pochodnym z klucza dostępu WebAuthn).
- **Scalanie**: rozmowa prowadzona bez komputera jest zapisywana lokalnie na urządzeniu (zaszyfrowana) jako ciąg
  tur z identyfikatorem sesji. Po powrocie połączenia host dopisuje je do historii jako nową gałąź od ostatniej
  wspólnej tury. Historia jest tylko-do-dopisywania, więc nic nie jest nadpisywane i nie ma konfliktów
  do rozstrzygania. Tury z urządzenia są oznaczone „prowadzone bez komputera”.
- Koszty rozmów bez komputera są doliczane do licznika kosztów po scaleniu.

## Uprawnienia klientów

Brama nadaje każdej sesji klienta zestaw dozwolonych poleceń. Domyślnie urządzenie sparowane ma wszystkie
(decyzja 3), ale lista jest jawna i sprawdzana po stronie hosta — klient nie może wywołać polecenia, którego
mu nie przyznano, nawet jeśli interfejs został zmodyfikowany. Polecenia `local_only` (np. otwarcie okna
dialogowego systemu) mają zdalne odpowiedniki.

## Nowe scenariusze zagrożeń (do dopisania w `THREAT_MODEL.md`)

| ID | Scenariusz | Obrona |
| --- | --- | --- |
| R1 | Przekaźnik przejęty albo złośliwy | E2E: widzi tylko szyfrogram; nie może podszyć się pod host (klucz hosta znany klientowi z parowania) |
| R2 | Podsłuch lub modyfikacja po drodze | TLS + Noise; ramki z licznikiem — powtórzenie odrzucane |
| R3 | Kradzież odblokowanego telefonu | biometria przy każdej zgodzie; limit; odwołanie z innego klienta |
| R4 | Kradzież telefonu z kluczem API | klucz w magazynie sprzętowym, nieeksportowalny w postaci jawnej dla innych aplikacji; odwołanie + przypomnienie o wymianie klucza |
| R5 | Podmiana kodu QR / atak przy parowaniu | krótki kod kontrolny potwierdzany fizycznie na komputerze |
| R6 | Złośliwa strona udająca panel | panel pod stałą domeną z CSP; klucz dostępu WebAuthn jest związany z domeną — na innej nie zadziała |
| R7 | Przejęcie hostingu panelu (podmiana skryptów) | integralność zasobów, wersjonowane wydania; opcja: panel serwowany wyłącznie z własnego serwera właściciela |
| R8 | Wstrzyknięcie poleceń w treści (strona, plik) wywołuje działanie zdalnie | bez zmian: znakowanie treści niezaufanych i reguła „trzech uprawnień” w Brokerze działają niezależnie od klienta |
| R9 | Zalanie hosta żądaniami przez przekaźnik | limit połączeń na identyfikator hosta w przekaźniku; host odrzuca nieuwierzytelnione ramki przed jakimkolwiek przetwarzaniem |
| R10 | Stare urządzenie z nieaktualną aplikacją | wersja protokołu negocjowana; host może wymusić minimalną |
| R11 | Dane rozmów w pamięci telefonu | szyfrowanie kluczem z magazynu; kasowanie przy odwołaniu i przy wylogowaniu |

Zmiana S13 / S14: scenariusze „otwarty port lokalny” i „zdalne sterowanie interfejsem” **pozostają
zagrożeniami** — plan ich nie dopuszcza. Dostęp zdalny jest nową, osobną drogą z własnym uwierzytelnieniem,
a nie złagodzeniem starych reguł.

## Testy, które muszą istnieć przed włączeniem

1. Test „przekaźnik nie widzi treści”: przekaźnik testowy zapisuje wszystko, co przenosi; znany tekst
   wiadomości nie występuje w zapisie.
2. Powtórzenie i przeniesienie dowodu zgody na inne żądanie — odrzucone.
3. Dowód od urządzenia odwołanego — odrzucony; kanał zerwany w czasie poniżej ustalonego progu.
4. Klient bez uprawnienia do polecenia — odmowa po stronie hosta.
5. Zerwanie i wznowienie połączenia przy trwającej odpowiedzi — żadne zdarzenie nie ginie (numery sekwencji).
6. Statyczna reguła „żadnego nasłuchu” obejmuje `remote-gateway`, `device-pairing`, `host-headless`.
7. Zestaw ataków na dostęp zdalny napisany przez sesję-recenzentkę, zamrożony w `evals/`.
