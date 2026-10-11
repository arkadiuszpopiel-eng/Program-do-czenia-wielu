# ADR 0027 — Wielu użytkowników: konta, logowanie i role

| Pole | Wartość |
|---|---|
| Status | Proponowany — czeka na podpis właściciela |
| Data | 2026-10-11 |
| Powiązane | zlecenie właściciela z 2026-10-11 („obsługa wielu użytkowników po koncie i logowaniu”); `docs/plan/18-wielu-uzytkownikow.md`, `03-roadmapa.md` (etap U); ADR 0003, 0015 (Broker, uprawnienia), 0019 (lista uprawnień klientów), 0021–0023 (dostęp zdalny), 0025 (biometria) |
| Zmienia | założenie planu v5 „program dla jednej osoby”; model uprawnień L0–L4 dostaje wymiar konta; dopisuje zagrożenia do modelu zagrożeń — po akceptacji |

## Kontekst

Alfa była projektowana dla jednej osoby. Właściciel chce, żeby z jednej instalacji korzystało kilka osób — każda
ze swoim kontem — na komputerze i przez klientów zdalnych. Konta dotykają Brokera, audytu i danych, więc decyzję
podejmuje człowiek.

## Decyzja (propozycja)

1. **Konto jest wymiarem każdej sesji klienta.** Rdzeń nie wykonuje polecenia bez ustalonego konta; okno lokalne
   po starcie wymaga logowania (albo działa na koncie Gościa, jeśli właściciel to włączy).
2. **Cztery role**: Właściciel, Dorosły, Dziecko, Gość. Rola wyznacza najwyższy poziom autonomii agentek, dostęp
   do chmury i do działań w systemie. Dokładnie jeden Właściciel; może przekazać rolę.
3. **Logowanie**: Windows Hello albo klucz dostępu (FIDO2); zapasowo PIN przechowywany jako skrót z solą
   w Menedżerze poświadczeń. Bez haseł w plikach i bez własnej kryptografii haseł poza sprawdzoną biblioteką.
4. **Dane rozdzielone na konta**: osobny katalog i osobny klucz szyfrujący na konto; klucz w magazynie sekretów
   systemu. Właściciel nie czyta cudzych rozmów; widzi dziennik działań i koszty.
5. **Broker**: każda prośba o zgodę niesie konto; zgodę wydaje to konto w granicach swojej roli albo Właściciel.
   Dziecko i Gość nie mogą wywołać działań w systemie.
6. **Rozpoznanie głosu i twarzy nie jest logowaniem** (zgodnie z ADR 0025) — może przełączyć rozmowę na konto,
   ale działania ryzykowne wymagają logowania.
7. **Zdalnie**: urządzenie paruje się z kontem; odwołanie konta odwołuje jego urządzenia.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Osobna instalacja na każde konto Windows | nie rozwiązuje wspólnego komputera rodzinnego ani klientów zdalnych; modele i kamery powielone |
| „Profile” bez logowania | brak granicy bezpieczeństwa — każdy widzi dane każdego |
| Rozpoznanie twarzy lub głosu jako logowanie | łatwe do podrobienia zdjęciem lub nagraniem; sprzeczne z ADR 0025 |
| Konta w chmurze producenta | sprzeczne z zasadą „lokalnie najpierw”; nie ma serwera kont |

## Konsekwencje

- Nowy moduł kont (kontrakt, atrapa, implementacja) i migracja dotychczasowych danych na konto Właściciela.
- Zmiany w Brokerze i audycie (obszar Jądra) — przegląd człowieka; nowe scenariusze w modelu zagrożeń: przejęcie
  odblokowanej sesji, podniesienie roli, wyciek między kontami, dziecko obchodzące filtr.
- Każde polecenie rdzenia i każde zdarzenie dostaje kontrolę konta — łączy się z listą uprawnień klientów (ADR 0019).
- Dane dzieci i dane biometryczne: wymagania RODO do opisania w `docs/plan/10-zgodnosc-prawna.md` przed wydaniem.
- Koszt: etap U przed klientami zdalnymi, żeby parowanie od początku wiązało urządzenie z kontem.

## Jak cofnąć

Do czasu migracji danych (zadanie U.2) decyzję można wycofać bez skutków. Po migracji powrót do jednego
użytkownika to scalenie kont w konto Właściciela — narzędzie scalania jest częścią zadania U.2.
