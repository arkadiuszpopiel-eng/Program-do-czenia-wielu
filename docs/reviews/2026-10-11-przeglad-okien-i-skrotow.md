# Przegląd okien, paneli i skrótów (2026-10-11)

Zgłoszenie właściciela: przy przeglądzie sprawdzać każde okno, każdy rozwijany panel i każdą funkcję
wywoływaną skrótem i myszką. Ten przegląd jest zautomatyzowany — `apps/desktop/ui/e2e/review-sweep.spec.ts`
uruchamia się z resztą testów interfejsu, więc powtarza się przy każdej zmianie.

## Co jest sprawdzane

W każdym z trzech stylów (Klasyczny, Szkło, Next) i obu motywów:

| Obszar                     | Zakres                                                                     | Co musi być prawdą                                                                                               |
| -------------------------- | -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Strony Ustawień            | wszystkie 32 strony (także cztery „wkrótce”); dodatkowo okno 900 px        | nic nie wystaje poza okno, brak przewijania w poziomie, żaden przycisk, pole ani nagłówek nie ma uciętego tekstu |
| Panele boczne              | `Alt+1` … `Alt+7`                                                          | jak wyżej                                                                                                        |
| Nakładki ze skrótów        | `Ctrl+K` (paleta), `Ctrl+/` (ściąga skrótów), `Ctrl+P` (przełącznik sesji) | otwiera się jako okno dialogowe, bez wad układu, zamyka się klawiszem Escape                                     |
| Skróty przenoszące do pola | `Ctrl+Shift+F` (szukanie w sesjach), `Ctrl+F` (szukanie w rozmowie)        | fokus trafia do właściwego pola; pasek szukania w rozmowie znika po Escape                                       |
| Elementy wyskakujące belki | objaśnienie stanu, szczegóły kosztów                                       | otwierają się myszką (`aria-expanded`), bez wad układu, zamykają się Escape                                      |
| Pole pisania               | podpowiedzi po „/” i „@”, menu pod prawym przyciskiem                      | lista i menu rysowane przez Alfę, pozycja „Wklej” jest, zamykają się Escape                                      |
| Konsola przeglądarki       | przez cały przebieg                                                        | zero błędów                                                                                                      |

Dostępność (kontrast, role, nazwy) tych samych ekranów sprawdzają `a11y.spec.ts` i `a11y-all.spec.ts`;
stany otwarte list wyboru — `skins.spec.ts` i `controls.spec.ts`; prawdziwą aplikację — `e2e-app`.

## Ustalenia

| ID   | Waga    | Ustalenie                                                                                                                                                                                         | Stan                             |
| ---- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------- |
| P-01 | poważne | `Ctrl+Shift+F` i przycisk „Szukaj” na pasku ikon sesji nic nie robiły, gdy lista sesji była zwinięta do paska ikon — czyli na oknie szerokości laptopa z otwartym prawym panelem (domyślny układ) | **naprawione (0.0.24)**          |
| P-02 | drobne  | podgląd w przeglądarce prosił o `favicon.ico`, którego nie było (błąd 404 w konsoli); okna programu nie miały ikony strony                                                                        | **naprawione (0.0.24)**          |
| P-03 | drobne  | w jasnym motywie kropka wagi „poważna” w historii zmian wychodziła brązowa zamiast pomarańczowej                                                                                                  | naprawione przed wydaniem 0.0.23 |
| P-04 | drobne  | w stylu Next (jasny) przycisk „Rozwiń wszystkie” w historii zmian miał za słaby kontrast                                                                                                          | naprawione przed wydaniem 0.0.23 |

## Druga część przeglądu (tego samego dnia)

Plik `e2e/review-sweep-flows.spec.ts` dokłada to, czego nie widać po samym wejściu na stronę — także w trzech
stylach i dwóch motywach:

- **pierwsze uruchomienie** — każdy z ośmiu kroków kreatora powitalnego;
- **kreator dostawcy** — lista dostawców i krok klucza; **okno potwierdzenia** usunięcia konta (Escape rezygnuje,
  konto zostaje);
- **każda lista wyboru na każdej stronie Ustawień w stanie otwartym** — lista się otwiera, ma opcje, mieści się
  w oknie i zamyka klawiszem Escape;
- **stany systemowe** — brak sieci, limit zapytań, brak kluczy, brak mikrofonu, mało miejsca, pusta lista sesji;
- **okna poboczne** — „Szybkie pytanie” i pigułka głosowa.

- **przebieg aktualizacji** — dostępna wersja, pobieranie z paskiem postępu, gotowa do ponownego uruchomienia,
  okno „Co nowego” (z przyciskiem „Historia zmian”), potwierdzenie przywrócenia poprzedniej wersji;
- **wiadomość w rozmowie** — prawy przycisk przypina i odpina pasek akcji;
- **Kreator agentek** z wpisanym opisem oraz strony z formularzami (zadania w tle, reguły Marszałka,
  umiejętności, wtyczki): każda zakładka i każda rozwijana sekcja w stanie otwartym.

Razem z pierwszą częścią: 55 testów, wszystkie zielone; w drugiej części nie wyszła żadna nowa wada.

**Ocena na oko:** obejrzałem zbiorcze zrzuty wszystkich 32 stron Ustawień w stylu Klasycznym (jasny) oraz po
ośmiu stron w stylach Szkło (ciemny) i Next (jasny) —
`scripts/shots-settings-all.mjs`. W tej skali (miniatury) nie widać rozjechanych układów, pustych kart ani
uciętych elementów. Uchwyt zmiany rozmiaru pól wielowierszowych jest rysowany przez Alfę (`base.css`), nie przez
system. Miniatury nie pozwalają ocenić drobnych odstępów ani typografii — to wymaga oglądania stron w pełnej skali.

## Czego ten przegląd nie obejmuje

- **Oceny wizualnej „na oko”** — test mierzy wady układu, nie estetykę. Zrzuty do oglądania robią skrypty
  `scripts/shots-*.mjs` (belka, nawigacja, konta, kamery, historia zmian, stany otwarte).
- **Skrótów globalnych** (`Ctrl+Alt+Space`, `Ctrl+Shift+F12`, dyktowanie, czytanie zaznaczenia) — działają poza
  oknem przeglądarki; sprawdza je częściowo `e2e-app`, reszta wymaga ręcznej próby na komputerze.
- **Okien Brokera** (zatwierdzenia) — osobny proces, obszar Jądra.
- **Stanów zależnych od sprzętu** — prawdziwa kamera, mikrofon, model lokalny.
- **Oceny na oko w pełnej skali** — obejrzane są miniatury: cały styl Klasyczny jasny i po osiem stron stylów
  Szkło ciemny i Next jasny; pozostałe kombinacje i ocena drobnych odstępów w pełnej skali są do zrobienia.
- **Dalszych kroków Kreatora agentek** (podgląd persony, test na sucho, zapis) i okien zgód — mają własne
  testy przebiegu, bez przeglądu układu w każdym stylu.

## Następne kroki przeglądu

1. Ręczna próba skrótów globalnych na laptopie właściciela (lista w Ustawienia → Skróty).
2. Ocena na oko w pełnej skali: po jednym stylu na sesję, wszystkie strony i panele.
3. Dopisać do przeglądu pozostałe okna dialogowe i kroki Kreatora agentek.
4. Okna Brokera i stany zależne od sprzętu — razem z właścicielem.
