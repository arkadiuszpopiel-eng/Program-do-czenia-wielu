# #63 — Stop i deadline podczas oczekiwania na magazyn pobrania

Baza `cea4444a`, osobny worktree `fix/download-disk-wait` i [karta #63](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/63).
Nie zmieniono kontraktów, zależności, polityk lub dotychczasowego deadline pobrania (300000 ms).

## Problem i naprawa

HTTP obserwował Stop i termin, ale otwarcie magazynu i zapis bufora czekały bezpośrednio na
`spawn_blocking`. Zawieszony magazyn mógł przetrzymać zadanie mimo Stop lub upłynięcia terminu.
Teraz oczekiwanie na begin/write obserwuje token i ten sam deadline. Pracownik sprawdza je
również przed rozpoczęciem operacji; gotowy wynik jest sprawdzany ponownie.

Rozpoczętego synchronicznego I/O nie można przerwać przez porzucenie JoinHandle. Komunikat
przyznaje, że operacja dyskowa może jeszcze kończyć się w tle; część jest usuwana przez Drop
po powrocie pracownika. Nie obiecujemy natychmiastowego usunięcia zablokowanego pliku.
Rozpoczęty commit pozostaje obserwowany do wyniku: zatwierdzonego pliku nie ukrywamy jako
Cancelled/Timeout. Przed rozpoczęciem commit sprawdzany jest token i dotychczasowy termin.

## RED → GREEN i granice dowodu

- Nowe sześć scenariuszy przed naprawą: **2 zaliczone, 4 nieudane, 0 pominiętych**, kod101.
  Stop i deadline nie kończyły oczekiwania w begin/write; dwie kontrole commit już przechodziły.
- Po naprawie pełne tools-net-contract/impl/fake, all-features: **62/0/0**, 13 wyników celów,
  kod0. Clippy dla tych trzech crate'ów i wszystkich celów, `-D warnings`: kod0; format: kod0.
- Lint kod0; generator HTML102 strony, weryfikacja odnośników i limit rozmiaru kod0.
- Test blokuje pracownika na jawnej bramce, bez sztucznego obciążania CPU. Termin przesuwany
  wirtualnie po rozpoczęciu I/O. Wynik Cancelled/Timeout musi być gotowy przed zwolnieniem
  bramki, z pustymi danymi i jawną informacją o tle. Po zwolnieniu: brak pliku, zero otwartych
  sinków i jedno porzucenie. Przy rozpoczętym commit wynik pozostaje Pending, a następnie
  sukces z plikiem. Watchdog testu nie jest nowym progiem akceptacji opóźnień.

[Komendy, kody wyjścia, SHA256 logów i źródeł](artifacts/download-wait-validation.json).
To deterministyczne atrapy magazynu i HTTP na Linux; nie dowód sprzętowego dysku,
GPU, mikrofonu lub prawdziwej wyszukiwarki. Niezależny przegląd przed merge pozostaje otwarty.

## Zachowane pełne CI bazy

Nie przerwano poprzedniego przebiegu nowym pushem. Baza `cea4444a` ma **CI175 10/10** i
**Windows97 4/4** zielone. Rust Linux3544/0/26, Windows3570/0/47 (po1131 wyników celów),
Chromium915/915 (305 na część: 9,8/9,9/8,1 min), Vitest573/573, Svelte0 błędów/ostrzeżeń,
WebView2 63/0/5 w2,6 min. Clippy, cargo-deny, schematy i pozostałe bramki zielone.
[Surowe podsumowania zadań/logów](artifacts/ci175-disk-baseline.json).
Nowa zmiana wymaga własnego pełnego CI; nie dziedziczy wyniku bazy.
