# ADR 0019 — Protokół klient–host niezależny od transportu

| Pole | Wartość |
|---|---|
| Status | Projekt zatwierdzony przez właściciela12.11.2026 z doprecyzowaniem poniżej |
| Data | 2026-10-09 |
| Powiązane | plan v6: `docs/plan/02-architektura-docelowa.md` (szew 3), `09-dostep-zdalny-i-bezpieczenstwo.md` (uprawnienia klientów, R10), `03-roadmapa.md` (B.1–B.3, B.5, D.1, D.6); ADR 0013, 0020, 0021 |
| Zmienia | wykonuje ADR 0013 (typy TS generowane z Rust zamiast pisanych ręcznie); nie zmienia listy poleceń ani zdarzeń |

## Podpis właściciela12.11.2026

Właściciel zatwierdził konkretny [pakiet projektu](../reviews/2026-11-11-remote-design-review.md)
jako podpis ADR0019–0023. [Zapis decyzji i zakres](../reviews/2026-11-12-owner-decisions.md).
To zatwierdzenie architektury, bez odbioru implementacji; chronione poprawki Brokera
i polityk nadal wymagają konkretnego przeglądu.

## Kontekst

Dziś jest 210 poleceń w jednej liście (`app-api/src/commands.rs`, makro `with_commands!`), ok. 38 zdarzeń
(`AlfaEvent`) i DTO w `app-api/src/dto/`. Interfejs zna tylko `AlfaClient` z dwiema implementacjami:
`TauriAlfaClient` i `FakeAlfaClient`. To dobry punkt wyjścia, ale plan v6 zakłada trzy klienty (okno Tauri,
panel WWW, Android) i trzy transporty (Tauri IPC, kanał szyfrowany, atrapa w testach). Brakuje pięciu rzeczy:
numerów sekwencji zdarzeń (klient, który nie nadąża, po cichu gubi paczki w `pump.rs`), błędów strukturalnych
(powłoka spłaszcza `AppError` do tekstu), tożsamości klienta i uprawnień (jedyne ACL to uprawnienia okien Tauri),
oznaczenia poleceń zakładających lokalną maszynę oraz generowanych typów TS.

## Decyzja

| Element | Propozycja |
|---|---|
| Ramka | wersjonowana, pole `v` w każdej ramce; polecenie `{ v, id, cmd, args }`, odpowiedź `{ v, id, ok }` albo `{ v, id, err }`, paczka zdarzeń `{ v, seq, events: [...] }` |
| Wersja protokołu | negocjowana przy połączeniu; host obsługuje bieżącą i poprzednią, żeby telefon z nieaktualną aplikacją nadal działał; host może wymusić minimalną (R10) |
| Numery sekwencji | każde zdarzenie dostaje `seq`; host trzyma bufor ostatnich N zdarzeń (N — do ustalenia po pomiarze) |
| Wznowienie | klient po przerwie wysyła `events_resume { after_seq }`; gdy bufor nie sięga tak daleko, host odpowiada `resync` i klient czyta stan od nowa |
| Błędy | przez łącze idzie `{ code, message, details }`; tekst dla użytkownika tłumaczy klient (i18n PL/EN) |
| Tożsamość klienta | każda sesja klienta ma `ClientId`, rodzaj (okno lokalne / urządzenie sparowane) i jawny zestaw dozwolonych poleceń |
| ACL poleceń | sprawdzana po stronie hosta przed wykonaniem; klient nie wywoła polecenia, którego mu nie przyznano, nawet po modyfikacji interfejsu; domyślnie urządzenie sparowane ma wszystkie (decyzja właściciela 3) |
| Polecenia `local_only` | oznaczenie w liście poleceń (okna dialogowe systemu, podgląd `asset:`, ścieżki bezwzględne w DTO); dla klientów zdalnych odpowiedniki: wybór pliku z listy hosta, podgląd przesyłany strumieniem |
| Typy TS | generowane z Rust (ADR 0013); CI sprawdza `git diff --exit-code`; ręczne kopie w `types*.ts` usunięte w B.1 etap 2 (2026-10-29) |
| Transporty | te same ramki przez Tauri IPC, przez kanał szyfrowany (ADR 0021) i przez atrapę w testach; klient nie wie, którego używa |

Szkic ramek (z planu; źródłem prawdy będzie `app-api`):

```
→ { "v": 1, "id": 4711, "cmd": "turn_send", "args": { … } }
← { "v": 1, "id": 4711, "ok": { … } }            albo  "err": { "code": "…", "message": "…" }
← { "v": 1, "seq": 90211, "events": [ { "type": "TurnDelta", … }, … ] }
→ { "v": 1, "id": 4712, "cmd": "events_resume", "args": { "after_seq": 90180 } }
```

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Osobne API dla klientów zdalnych (np. REST obok IPC) | dwie powierzchnie do utrzymania i do przeglądu bezpieczeństwa; zasada planu: jeden protokół, trzy transporty |
| Zdarzenia bez numerów, klient po przerwie zawsze czyta cały stan | drogie na łączu komórkowym; nie rozwiązuje cichego gubienia paczek w trakcie odpowiedzi |
| Błędy jako tekst | klient nie może ich tłumaczyć ani rozróżniać; trzy klienty powielałyby dopasowywanie napisów |
| ACL tylko w interfejsie (ukrywanie przycisków) | zmodyfikowany klient wywoła wszystko; uprawnienia muszą być po stronie hosta |
| Typy TS dalej pisane ręcznie | przy trzech klientach rozjazd jest pewny; ADR 0013 już to rozstrzygnął |
| Protobuf / gRPC | odrzucone już w ADR 0013 dla IPC; JSON jest czytelny i wystarczający, a przeglądarka i tak potrzebuje WebSocket |

## Konsekwencje

- Zadania B.1 (generator typów), B.2 (`seq` i wznowienie w `EventHub` / `pump.rs`), B.3 (błędy strukturalne)
  i B.5 (`local_only`) nie dodają funkcji i mogą iść równolegle z etapem A; są warunkiem etapu D.
- B.2 zamyka przy okazji otwarte ustalenie o deltach przychodzących w czasie wczytywania rozmowy.
- Okno Tauri korzysta na zmianie od razu: przestaje po cichu gubić zdarzenia i dostaje kody błędów.
- Lista poleceń `local_only` i ich zdalnych odpowiedników wymaga przeglądu wszystkich 210 poleceń; ścieżki
  bezwzględne w DTO to także kwestia prywatności (klient zdalny widzi układ dysku hosta) — do oceny przy B.5.
  **Zrobione 2026-10-29 (B.5):** przegląd 281 poleceń — 31 oznaczonych w `crates/app-api/data/local-only.json`
  (powód: okno systemu, podgląd `asset:`, ścieżka hosta, okna/schowek/terminal hosta; odpowiednik: istniejące
  polecenie, zadanie D.1/D.5/D.6 albo „tylko lokalnie” z powodem) i 16 pól DTO ze ścieżką hosta, które brama
  D.1 skróci albo zastąpi uchwytem. Tabela jest w schemacie łącza i w typach TS (`LOCAL_ONLY_COMMANDS`);
  testy `app-api/tests/local_only.rs` pilnują, że wpisy wskazują prawdziwe polecenia i pola, a polecenie
  z nazwą wyboru, eksportu, importu albo otwarcia nie ominie tabeli. Egzekucja (odmowa klientowi
  zdalnemu) przychodzi z bramą w D.1.
- Test wymagany przed włączeniem dostępu zdalnego: zerwanie i wznowienie połączenia przy trwającej odpowiedzi —
  żadne zdarzenie nie ginie; klient bez uprawnienia do polecenia dostaje odmowę po stronie hosta.
- Rozmiar bufora zdarzeń i limit rozmiaru ramki są budżetem zasobów modułu `remote-gateway` — progi ustala
  właściciel po pomiarach.

## Jak cofnąć

- Pola `v` i `seq` są dodatkiem: klient, który ich nie używa, działa jak dziś. Wyłączenie wznowienia sprowadza
  zachowanie do obecnego (pełne wczytanie stanu po przerwie).
- Generator typów można wymienić (`tauri-specta` ↔ `ts-rs`) bez zmiany kontraktów — jak w ADR 0013.
- ACL per klient przy jednym kliencie lokalnym z pełnym zestawem jest równoważna stanowi obecnemu.
