# ADR 0023 — Tryb bez komputera: klucz API na urządzeniu i scalanie rozmów

| Pole | Wartość |
|---|---|
| Status | Proponowany — czeka na podpis właściciela |
| Data | 2026-10-09 |
| Powiązane | decyzja właściciela nr 3 z 2026-10-09; plan v6: `docs/plan/09-dostep-zdalny-i-bezpieczenstwo.md` (praca bez komputera, R4, R11), `04-nowe-funkcje.md` (F.6, sprawy do rozstrzygnięcia), `03-roadmapa.md` (E.6, F.6); ADR 0006, 0021, 0022 |
| Zmienia | ustalenia planu v5: „sekrety nigdy nie opuszczają komputera” (`AGENTS.md`: sekrety tylko w Windows Credential Manager) oraz „bez synchronizacji — tylko import/eksport `.alfa`” |

## Kontekst

Właściciel zdecydował (decyzja 3), że telefon i panel WWW są pilotem ze 100 % funkcji, a gdy nie ma połączenia
z komputerem (wyłączony, brak sieci po jego stronie), klient rozmawia przez **ten sam model chmurowy (API)**,
który jest ustawiony na komputerze. Model lokalny zostaje tylko na komputerze. Żeby urządzenie mogło rozmawiać
z dostawcą bez hosta, musi mieć klucz API — a plan v5 zabrania sekretom opuszczać komputer. Rozmowa prowadzona
bez komputera musi też wrócić do historii, choć plan v5 nie przewiduje synchronizacji.

## Decyzja

| Element | Propozycja |
|---|---|
| Stan w interfejsie | wyraźny komunikat „Bez komputera — rozmowa przez model w chmurze”; narzędzia, pamięć, pliki i model lokalny niedostępne (mapa zdolności) |
| Droga rozmowy | bezpośrednio z urządzenia do dostawcy API — tego samego i z tym samym modelem co na komputerze; ustawienia dostawcy i modelu klient zapamiętuje przy każdym połączeniu z hostem |
| Klucz API — Android | trafia na telefon **tylko po osobnej zgodzie na komputerze** („Pozwól temu urządzeniu rozmawiać bez komputera”), kanałem E2E, do magazynu kluczy Androida (Android Keystore) |
| Odwołanie | odwołanie urządzenia wysyła polecenie skasowania klucza; skradziony telefon może go nie odebrać, więc właściciel dostaje przy odwołaniu przypomnienie o wymianie klucza u dostawcy |
| Klucz API — panel WWW | tryb bez komputera w przeglądarce **wyłączony**, dopóki nie zostanie rozstrzygnięte pytanie otwarte niżej |
| Zapis bez komputera | rozmowa zapisywana lokalnie na urządzeniu, zaszyfrowana kluczem z magazynu, jako ciąg tur z identyfikatorem sesji; kasowana przy odwołaniu i przy wylogowaniu (R11) |
| Scalanie | po powrocie połączenia host **dopisuje** tury do historii jako nową gałąź od ostatniej wspólnej tury; historia jest tylko-do-dopisywania (ADR 0006), więc nic nie jest nadpisywane i nie ma konfliktów do rozstrzygania |
| Oznaczenie | tury z urządzenia oznaczone „prowadzone bez komputera” |
| Koszty | doliczane do licznika kosztów po scaleniu |
| Zakres | to nie jest synchronizacja: dane płyną w jedną stronę (urządzenie → host) i dotyczą tylko tur rozmowy; pełna synchronizacja (CRDT) pozostaje tematem badawczym H.7 |

## Otwarte pytanie do właściciela — klucz API w przeglądarce

Przeglądarka nie ma magazynu równie bezpiecznego jak telefon. Opcje z katalogu funkcji:

| Opcja | Co oznacza | Ryzyko |
|---|---|---|
| A — nie przechowywać | tryb bez komputera tylko w aplikacji Android; panel WWW bez hosta pokazuje „komputer niedostępny” | brak nowego ryzyka; panel nie spełnia w tym punkcie „100 % funkcji” |
| B — przechowywać zaszyfrowany | klucz API w pamięci przeglądarki, zaszyfrowany kluczem pochodnym z klucza dostępu WebAuthn; odszyfrowanie wymaga potwierdzenia kluczem dostępu | po odszyfrowaniu klucz jest w pamięci strony — przejęcie hostingu panelu (R7) lub złośliwe rozszerzenie mogą go odczytać |

Sesja nie rekomenduje opcji B bez osobnego przeglądu bezpieczeństwa panelu (CSP, integralność zasobów, hosting).
Do rozstrzygnięcia obowiązuje stan z wiersza „Klucz API — panel WWW”.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Bez komputera nic nie działa | sprzeczne z decyzją właściciela 3 |
| Mały model lokalny w telefonie | decyzja właściciela: model lokalny tylko na komputerze; temat odłożony jako badawczy (H.1) |
| Pośrednik API w chmurze trzymający klucz | serwer znający klucz i treść rozmów; sprzeczne z zasadą, że przekaźnik widzi tylko szyfrogram |
| Osobny klucz API wpisywany ręcznie w telefonie | gorsza wygoda i drugi sekret do pilnowania; możliwe jako uzupełnienie — do decyzji właściciela, plan tego nie rozstrzyga |
| Klucz wysyłany każdemu sparowanemu urządzeniu automatycznie | sekret opuszczałby komputer bez świadomej decyzji; projekt wymaga osobnej zgody per urządzenie |
| Scalanie przez nadpisanie albo łączenie tur w istniejącej gałęzi | łamie historię tylko-do-dopisywania; rodzi konflikty |
| Pełna synchronizacja dwukierunkowa (CRDT) | dużo większy zakres; plan odkłada ją jako H.7, „tylko jeśli tryb pilota okaże się niewystarczający” |

## Konsekwencje

- Zmienia się twarda reguła z `AGENTS.md` o miejscu przechowywania sekretów — zmianę `AGENTS.md` zatwierdza
  człowiek. Reguły „sekrety nigdy w plikach konfiguracji, logach ani eksporcie `.alfa`” pozostają.
- Nowy scenariusz R4 (kradzież telefonu z kluczem API): klucz w magazynie sprzętowym, odwołanie, przypomnienie
  o wymianie klucza. Uczciwie: polecenie skasowania może nie dotrzeć — jedyną pewną obroną jest wymiana klucza.
- Zgoda na przekazanie klucza jest działaniem na sekrecie — sposób jej wydania (okno Brokera na komputerze)
  wymaga przeglądu człowieka razem z ADR 0022.
- Rozmowa bez komputera omija Brokera, bo nie ma w niej narzędzi; klient offline nie może wykonywać żadnych
  działań — to niezmiennik modułu `offline-client`.
- ADR 0006 ma status „Tymczasowy”; scalanie opiera się na jego kierunku (gałęzie, brak modyfikacji tur).
  Gdyby ADR 0006 się zmienił, ten ADR wraca do przeglądu.
- Limit kosztów obowiązuje na hoście; jak ograniczyć wydatki urządzenia działającego bez hosta — do ustalenia
  w SPEC `offline-client`.

## Jak cofnąć

- Cofnięcie zgody „rozmowa bez komputera” dla urządzenia kasuje klucz na urządzeniu (gdy jest osiągalne)
  i wyłącza tryb; pewność daje wymiana klucza u dostawcy.
- Gałęzie dopisane ze scalenia są zwykłymi gałęziami historii — można je pominąć, niczego nie nadpisały.
- Do podpisu tego ADR obowiązują ustalenia planu v5: sekrety nie opuszczają komputera, brak scalania.
