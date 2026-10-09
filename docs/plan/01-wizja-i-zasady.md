# 01 — Wizja i zasady planu v6

## Po co nowy plan

Plan v5 opisuje Alfę jako program na jeden komputer z Windows 11. Właściciel chce pójść dalej: ten sam asystent
ma być dostępny z kilku miejsc — z komputera z Windows, później z Linuksa, z przeglądarki przez Internet
i z telefonu z Androidem — i ma dostać funkcje, które w 2026 roku są już standardem w asystentach AI.

Plan v6 odpowiada na trzy pytania:

1. **Co już mamy?** — [`00-stan-wykonania.md`](00-stan-wykonania.md).
2. **Jak zbudować resztę, żeby nie trzeba było przepisywać tego, co jest?** —
   [`02-architektura-docelowa.md`](02-architektura-docelowa.md).
3. **W jakiej kolejności?** — [`03-roadmapa.md`](03-roadmapa.md).

## Cele produktu

| Cel | Co to znaczy w praktyce |
| --- | --- |
| Asystent po polsku, który działa bez Internetu | rozmowa, głos i narzędzia na modelach lokalnych; chmura jest opcją, nie warunkiem |
| Agentki, którym można zaufać | każde działanie przechodzi przez Brokera, ma poziom ryzyka, zgodę, wpis w dzienniku i „Cofnij” tam, gdzie to możliwe |
| Jeden asystent, wiele ekranów | komputer jest „domem” Alfy; telefon i przeglądarka to pełnoprawne piloty do niego |
| Rozbudowa bez przebudowy | nowa platforma to nowa implementacja portów, nowa funkcja to nowy moduł; rdzeń się nie zmienia |
| Zgodność z prawem UE od początku | oznaczanie treści AI, zgłaszanie podatności, brak telemetrii domyślnie |

## Kolejność platform

| Kolejność | Platforma | Rola | Dokument |
| --- | --- | --- | --- |
| 1 | **Windows 11** (x64) | pierwsza wersja, host i interfejs | [`05-windows.md`](05-windows.md) |
| 2 | **Panel WWW** | pilot przez Internet (przeglądarka) | [`07-panel-www.md`](07-panel-www.md) |
| 3 | **Android 15+** | pilot w telefonie; bez komputera — rozmowa przez model chmurowy | [`08-android.md`](08-android.md) |
| 4 | **Linux** | drugi system dla hosta i interfejsu | [`06-linux.md`](06-linux.md) |

Panel WWW i Android są przed Linuksem, bo korzystają z tego samego fundamentu (host bez okna i protokół zdalny)
i nie wymagają przepisywania warstwy systemowej. Linux to najwięcej pracy „pod spodem” — ok. 30 portów
systemowych — i najmniej nowego dla użytkownika. Kolejność może zmienić właściciel; etapy D–G są tak
rozpisane, żeby G (Linux) dało się zacząć zaraz po B.

Dokumentacja wszystkich czterech powstaje teraz, żeby decyzje podejmowane przy Windows nie zamknęły drogi
pozostałym.

## Decyzje właściciela z 2026-10-09

| # | Pytanie | Decyzja | Gdzie rozpisana |
| --- | --- | --- | --- |
| 1 | Jak telefon i panel WWW łączą się z komputerem? | Przez **przekaźnik z szyfrowaniem end-to-end**; komputer nie otwiera żadnego portu; parowanie kodem QR | ADR 0021, [`09`](09-dostep-zdalny-i-bezpieczenstwo.md) |
| 2 | Czy zdalnie wolno zatwierdzać ryzykowne działania? | **Tak, wszystkie**, także wysokiego ryzyka | ADR 0022, [`09`](09-dostep-zdalny-i-bezpieczenstwo.md) |
| 3 | Czym są telefon i panel WWW? | **Pilotem ze 100 % funkcji**. Gdy nie ma połączenia z komputerem, klient rozmawia przez **ten sam model chmurowy (API)**, który jest ustawiony na komputerze. Model lokalny zostaje tylko na komputerze | ADR 0023, [`08`](08-android.md) |
| 4 | Jak nowy plan ma się do v5? | Plan v6 **obok**, v5 zostaje bazą; do `AGENTS.md` jedna reguła o prowadzeniu planu | ten katalog |

Decyzje 1–3 zmieniają ustalenia planu v5 i modelu zagrożeń. Każda ma projekt ADR, który wymaga podpisu
właściciela, zanim powstanie kod — to obszar Jądra bezpieczeństwa (bramka ludzka #7).

## Co się zmienia względem planu v5

| Ustalenie v5 | Po zmianie | Dlaczego to bezpieczne | ADR |
| --- | --- | --- | --- |
| Cel: tylko Windows 11 | Windows pierwszy; Linux i Android przez nowe implementacje portów | kod systemowy nadal tylko za portami; reguła „żadnego `windows::` poza platformą” zostaje | 0017, 0018 |
| Żadnego nasłuchu sieciowego na komputerze (S14 — test F4-07; S13 — port CDP, test F3-13) | Nadal **żadnego nasłuchu na komputerze**: host sam wychodzi do przekaźnika; testy zostają i obejmują nowe moduły hosta. Nasłuchuje tylko przekaźnik — osobna usługa poza komputerem | brak otwartego portu = brak powierzchni do skanowania; przekaźnik widzi tylko szyfrogram | 0021 |
| Zgoda tylko w oknie Brokera, z dowodem fizycznego kliknięcia | Dochodzi drugi rodzaj dowodu: podpis sparowanego urządzenia po biometrii | podpis kluczem z bezpiecznego magazynu urządzenia, wiązany z konkretnym żądaniem; urządzenie można odwołać | 0022 |
| Sekrety nigdy nie opuszczają komputera | Klucz API dostawcy chmurowego może trafić na sparowany telefon (do magazynu kluczy Androida) | tylko kanałem E2E, tylko po osobnej zgodzie, z możliwością zdalnego skasowania | 0023 |
| Bez synchronizacji — tylko import/eksport `.alfa` | Rozmowy prowadzone bez komputera **dołączają się** do historii po powrocie połączenia | historia jest tylko-do-dopisywania, więc scalanie to dopisanie gałęzi, nie nadpisanie | 0023 |
| Typy TS pisane ręcznie | Generowane z Rust (ADR 0013 — w końcu wykonany) | trzy klienty jednego protokołu nie mogą się rozjechać | 0019 |

Reguły twarde z `AGENTS.md` („cel: Windows 11”, „sekrety tylko w Windows Credential Manager”) obowiązują bez zmian,
dopóki właściciel nie podpisze ADR 0017 i 0023 i nie zatwierdzi odpowiedniej zmiany `AGENTS.md`.

Czego plan v6 **nie** zmienia: Broker jako jedyny wykonawca działań, poziomy L0–L4, historia tylko-do-dopisywania,
markdown renderowany w Rust, zakaz fontów webowych i `backdrop-filter`, reguła skrótów AltGr, kill-switch,
brak telemetrii, przegląd człowieka nad Jądrem, zasada „kryterium spełnione dopiero na sprzęcie”.

## Zasady projektowe

1. **Rdzeń nie wie, na jakim działa systemie ani jaki klient z nim rozmawia.** Wszystko, co systemowe, jest portem
   w `platform-contract`; wszystko, co widzi klient, jest poleceniem albo zdarzeniem w `app-api`.
2. **Moduł to trójka crate'ów** (`-contract`, `-impl`, `-fake`) z manifestem i jednostronicowym SPEC. Nowe moduły
   planu v6, które są modułami hosta (`remote-gateway`, `device-pairing`, `offline-client`, platformy), trzymają się
   tej reguły. Wyjątki są jawne i takie jak dotąd: biblioteka kompozycji (`platform-select`), binarki
   (`host-headless`, `remote-relay`) i klient w TypeScript (`web-panel`).
3. **Jeden protokół, trzy transporty.** Te same polecenia i zdarzenia idą przez Tauri IPC (okno na komputerze),
   przez kanał szyfrowany (telefon, przeglądarka) i w testach przez atrapę.
4. **Bezpieczeństwo nie zależy od klienta.** Klient zdalny nie dostaje skrótu do narzędzi — rozmawia z tym samym
   hostem, a ten z tym samym Brokerem. Zmienia się tylko to, skąd przychodzi zgoda.
5. **Uczciwość co do możliwości sprzętu.** Baza to laptop z kartą 6 GB. Funkcje, które na niej nie działają,
   są w planie nazwane po imieniu (patrz „Ograniczenia” w [`04-nowe-funkcje.md`](04-nowe-funkcje.md)).
6. **Najpierw to, co już zbudowane.** Etap A (zamknięcie MVP na Windows) ma pierwszeństwo przed każdą nową
   funkcją — bez tego nowe platformy powielałyby niesprawdzony kod.

## Kto co robi

Podział z [`../KONTYNUACJA.md`](../KONTYNUACJA.md) §4 obowiązuje dalej. Nowe pozycje:

| Obszar | Sesja AI | Właściciel |
| --- | --- | --- |
| ADR 0017–0024 | projekt, opcje, rekomendacja | **decyzja i podpis** |
| Przekaźnik w Internecie | kod, obraz kontenera, instrukcja | konto u dostawcy hostingu, domena, opłaty |
| Aplikacja Android | kod, budowa, testy w emulatorze | telefon do testów, konto dewelopera przy publikacji |
| Zgodność prawna | lista zadań, dokumenty techniczne (SBOM, polityka zgłoszeń) | konsultacja z prawnikiem przed wydaniem komercyjnym |
