# Domykanie usterek — 10.11.2026

Właściciel zlecił usunięcie pozostałych problemów i sprawdzenie działającej wersji.
Zamykamy zgłoszenia napraw potwierdzonych kodem i testami; nie uznajemy braku odbioru za sukces.
Baza `574b3391`, trzy niezależne worktree i karty #64, #73, #75. Bez zmian polityk,
progów evals, uprawnień Brokera ani chronionego Core. PR24 pozostaje draft, bez merge.

## Poprawki

| Karta | Odtworzenie i zmiana | Sprawdzenie lokalne |
| --- | --- | --- |
| [#64](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/64) | Gotowa odpowiedź HTTP/SearchPort mogła ominąć istniejący deadline. Kontrola przed i po odpowiedzi/chunk oraz przed zwróceniem wyniku; Stop zachowuje pierwszeństwo. | RED0/6 → GREEN68/0/0; Clippy wszystkich celów/funkcji kod0. |
| [#73](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/73) | Po odmowie `Policy` model mógł pominąć ją w końcowym tekście. Runtime dopisuje informację z trwałego stanu faktycznego wykonania, także po wznowieniu, bez ujawniania argumentów. | Właściwy RED1/2 → GREEN135/0/0; Clippy0. Twarda odmowa nadal przerywa przed kolejną odpowiedzią modelu. |
| [#75](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/75) | Pomiar zakończenia procesu czytał `/proc` ponownie po wyjściu z pętli. Przejściowy błąd odczytu był uznawany za zakończenie. Teraz czeka na brak PID lub zombie, zachowuje jeden końcowy odczyt i dołącza diagnostykę do porażki. | RED1/3 → GREEN59/0/1; Clippy0; 20/20 rzeczywistych anulowań ×3 mosty-atrapy przy limicie2s. |

Liczby oznaczają zaliczone/niezaliczone/ignorowane, a RED zaliczone/niezaliczone.
Pierwszy fixture #73 miał błędne `text` zamiast `content`: wynik0/3 zachowany jako błąd
przygotowania próby. Poprawiony fixture potwierdził rzeczywiste `StepStatus::Denied` i dał RED1/2.
Nie przedstawiamy trzech porażek pierwszego fixture jako trzech błędów produktu.
Ignorowany test modułu mostów wymaga prawdziwego CLI; nie uruchamiano go ani nie zaliczono.

#75 z CI182 nastąpił po4,63s przy budżecie20s. To dowodzi sprzecznych odczytów w starym
oracle, ale log nie zawiera surowego `/proc`, więc nie ustalamy dokładnego errno ani stanu
tego konkretnego procesu. Deterministycznie odtworzono błędną ocenę odczytu i nazwę ze spacjami.
Kod zabijania procesów nie został zmieniony. Rzeczywisty test drzewa i nowe przypadki nadal
odrzucają żywy proces; nie zmieniono budżetów ani nie dodano retries do CI.

#64 nie cofa wysłanego żądania ani rozpoczętego zapisu zewnętrznego. #73 nie dowodzi, że model
zawsze opisze prawdę: dodaje deterministyczny fakt odmowy do `Completed.summary`; projekcja
aplikacji używa tego pola. Poprzednia bezpośrednia sonda odmowy Qwen poza runtime pozostaje
częściowo niezaliczona, a twarde odmowy już wcześniej poprawnie kończyły przebieg.

## Zachowany wynik poprzedniej wersji

`574b3391c605729aff55006c4c9f8a97e50330d9`:
[CI182](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42854928717)
**9/10**, zakończony, nie ponawiany: Linux3580/1/26 (#75), Windows3607/0/47,
po1138 wyników celów. Vitest573, Chromium315+315+315=945, Tauri i cargo-deny zielone.
[Windows105](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42854928523)
**4/4**: rzeczywisty WebView2 **63/0/5**, pięć pominięć nie jest zaliczeniem.
Qwen8B CPU: pierwszy token15346ms,96 tokenów,28368ms,7,37tok/s; propozycja narzędzia18518ms
nie jest potwierdzeniem wykonania fizycznej czynności. Bielik11B: `NOT_RUN_CPU_BUDGET`,
0tokenów przy RAM8192MB/VRAM0MB; nie jest udaną generacją ani odbiorem RTX2060.

Pełny CI nowych poprawek wymaga osobnego przebiegu. Nie przerwano poprzedniego CI kolejną
publikacją. Poprzednik `f6422855` miał CI18110/10 i Windows1044/4, co pozostaje dowodem
poprzedniej wersji, a nie zielonym wynikiem nowej gałęzi.

## Pozostały odbiór i backlog

- #41/#48/#58: fakty i jakość rzeczywistych odpowiedzi Core. Mały Bielik1.5B pozostaje
  pomocniczy; wcześniejsze niezaliczone50/50 fikcyjnych akcji nie znikają z historii.
  Aktualny Core jest budowany do próby głównego Qwen8B z limitem96tokenów i bez zmiany czasu.
- #70: konkretna chroniona łatka testu prywatności nadal wymaga odpowiedzi właściciela.
  SHA256 `636b5802fba693d6ffe951abab4ad538b094dc539bfcd15a80b08a92636e1e82`.
  Ogólna zgoda na maksymalne uprawnienia nie jest przeglądem tej łatki.
- #69: panel telefonu jest podglądem; brama/parowanie nie są zaimplementowane. Brak odbioru
  rzeczywistego telefonu, RTX2060 12GB, mikrofonu, UAC i sterowników.
- Produkcyjny dostawca WWW wymaga potwierdzonego wyboru usługi; SearchPort i atrapa nie są
  wyszukiwarką. Nie użyto przypadkowego API ani kluczy z plików.
- Świeży niezależny przegląd wymagany przed merge; nie uruchamiano subagentów i mostów CLI.
  Cel Enterprise nie jest zakończonym odbiorem.

[Komendy, wyniki i SHA256](artifacts/defect-closure-validation.json).
Surowe logi i checkpoint: `/workspace/scratch/defect-closure/`,
`/workspace/scratch/nightly-status.json`. Wcześniejsze logi/modelowe sondy zachowane;
zwolniono wyłącznie odtwarzalny cache kompilacji starszy niż8h z manifestem usuniętych artefaktów.
