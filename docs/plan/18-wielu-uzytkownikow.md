# 18 — Wielu użytkowników: konta, logowanie, role (etap U)

Stan na **2026-10-11**, wersja 0.0.28. Zlecenie właściciela: obsługa wielu użytkowników po koncie i logowaniu,
na wszystkich celach (Windows, Linux, panel WWW, Android). Projekt decyzji:
[ADR 0027](../ADR/0027-wielu-uzytkownikow-konta-i-role.md) — **Proponowany**, czeka na podpis właściciela,
bo zmienia model uprawnień (obszar Jądra).

## Dlaczego to osobny etap

Dziś Alfa zakłada jedną osobę: jedna historia rozmów, jedna pamięć, jeden zestaw kluczy API, jeden poziom
autonomii agentek, a Broker pyta „właściciela”. Konta zmieniają to we wszystkich warstwach, więc trzeba je
zaprojektować raz, zanim powstaną klienci zdalni (etapy D–F) i baza osób (P6).

## Co dostaje użytkownik

| Element | Opis |
| --- | --- |
| Konta | każdy domownik albo współpracownik ma własne konto z nazwą, znakiem i kolorem |
| Role | **Właściciel** — pełnia praw, zarządza kontami i urządzeniami · **Dorosły** — własne rozmowy, pamięć, klucze i limity · **Dziecko** — bez działań w systemie, bez chmury (chyba że właściciel zezwoli), filtr treści · **Gość** — rozmowa bez pamięci, znika po wylogowaniu |
| Logowanie na komputerze | Windows Hello albo klucz dostępu; zapasowo PIN; szybkie przełączanie kont; blokada po bezczynności |
| Logowanie zdalne | panel WWW i telefon logują się na konkretne konto; urządzenie jest sparowane z kontem, nie z komputerem jako całością |
| Rozpoznanie po głosie i twarzy | podpowiada, kto mówi, i przełącza rozmowę na jego konto (z P6); do działań ryzykownych zawsze potrzebne jest logowanie — rozpoznanie nie jest autoryzacją (ADR 0025) |
| Własne i wspólne | własne: rozmowy, pamięć, pliki sesji, klucze API, głos i ustawienia wyglądu; wspólne (za zgodą właściciela): modele lokalne, kamery, wybrane zakresy pamięci, agentki z Kreatora |
| Limity | miesięczny limit kosztów i najwyższy poziom autonomii agentek na konto |
| Nadzór | dziennik działań z kontem przy każdej pozycji; właściciel widzi zgody i koszty wszystkich kont, ale nie treść cudzych rozmów; STOP działa dla każdego |

## Jak to wygląda w budowie

- **Tożsamość w rdzeniu**: każda sesja klienta (okno, przeglądarka, telefon) niesie identyfikator konta; każde
  polecenie i zdarzenie jest sprawdzane względem roli (rozszerzenie listy uprawnień klientów z ADR 0019).
- **Dane**: osobny katalog danych i osobne klucze szyfrujące na konto; klucze w Menedżerze poświadczeń Windows
  (na Linuksie: Secret Service, na Androidzie: magazyn kluczy systemu).
- **Broker**: prośba o zgodę wskazuje konto, które ją wywołało; zgodę wydaje to konto albo właściciel; rola
  ogranicza, o co w ogóle można prosić.
- **Głos**: profil głosu i wybrane głosy agentek na konto; słowo wywoławcze wspólne.
- **Eksport `.alfa`**: na konto; właściciel może wyeksportować całość.
- **Zgodność**: dane dzieci i dane biometryczne — wymagania RODO opisane w [`10`](10-zgodnosc-prawna.md).

## Zadania (roadmapa, etap U)

| ID | Zadanie | Zależy od |
| --- | --- | --- |
| U.0 | Podpis właściciela pod ADR 0027 | — |
| U.1 | Moduł kont: rejestr, role, logowanie lokalne (Windows Hello, klucz dostępu, PIN), blokada | U.0 |
| U.2 | Rozdzielenie danych na konta: rozmowy, pamięć, pliki, ustawienia; migracja dotychczasowych danych na konto właściciela | U.1 |
| U.3 | Klucze API, limity kosztów i poziom autonomii na konto | U.1 |
| U.4 | Broker: konto w prośbie o zgodę, ograniczenia roli, dziennik z kontem | U.0; przegląd właściciela |
| U.5 | Okno: ekran logowania, przełącznik kont w belce, strona Ustawień „Użytkownicy” | U.1 |
| U.6 | Rola Dziecko i Gość: filtr treści, brak pamięci gościa | U.2 |
| U.7 | Połączenie z bazą osób: rozpoznanie głosu i twarzy wybiera konto | U.2, P6 |
| U.8 | Konta w dostępie zdalnym: parowanie urządzenia z kontem, logowanie w panelu WWW i w telefonie | U.1, D.2, E.3, F.2 |
| U.9 | Testy ataków: przejęcie sesji, podniesienie roli, dostęp do cudzych danych; przegląd w świeżym kontekście | U.1–U.8 |

**Wyjście z etapu U:** dwie osoby korzystają z jednej Alfy na tym samym komputerze i z dwóch telefonów; żadna nie
widzi rozmów ani pamięci drugiej; dziecko nie może uruchomić działania w systemie; właściciel odbiera dostęp
jednym kliknięciem.

## Otwarte pytania do właściciela

1. Czy agentki mają mieć osobną „znajomość” każdej osoby (inna pamięć), ale wspólną osobowość? (Propozycja: tak.)
2. Czy Dorosły może sam dodawać klucze API, czy tylko korzystać z kluczy właściciela w ramach limitu?
3. Czy konto Gościa ma być dostępne bez logowania z ekranu startowego?
