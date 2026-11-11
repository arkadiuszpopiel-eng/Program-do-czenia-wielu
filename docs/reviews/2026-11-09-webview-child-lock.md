# WebView2: diagnostyka kłódki Dziecko, #42

9.11.2026. [Windows80](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42549736415),
źródło `169a6004`, zadanie128942395555: **60 zaliczonych / 1 nieudany / 6 pominiętych**,4,6 minuty.
Scenariusz `z-night` z poprawką #40 przeszedł w39,9s. Dokowanie korzystało z jawnie emulowanego
viewportu1600×900 tylko w tym fragmencie; to Windows/WebView2 na runnerze GitHub, osobno od
natywnego szerokiego okna, sprzętu właściciela i prawdziwego mikrofonu.

## Dowody porażki

`zy-klodka-dziecko.spec.ts` rozpoczął się02:30:21.167UTC; wynik `timedOut`,120003ms.
Stos dodatkowego błędu wskazuje `permissions_child_lock` przy końcowym wyłączaniu profilu
(dotychczasowa linia87). Zrzut52a nie powstał. Sprzątanie w `finally` mogło przesłonić wcześniejszą
porażkę; na tym zapisie nie można wskazać pierwotnej operacji. Brak śladu `trace.zip`.
Snapshot pokazuje działające okno, a backend rejestruje później kill-switch. Komunikat
`Target page, context or browser has been closed` po timeoutcie nie dowodzi awarii procesu.
Poprzedni Windows78 na74ce7f99 zaliczył ten sam test w3,1s.

Artefakt13645970772,46 637 759 bajtów, SHA256
`da0b55a2f04f44c3f63a076e5b0c409428abf517cc5db04d3218b7357e16a0d8`.
Lokalnie `/workspace/scratch/webview2-ci80-169a.zip`; zapisano osobno results.json,
snapshot i logi backendu. [Metadane](artifacts/webview80-169a-results.json).

## Przyrost diagnostyczny

Nazwane kroki Playwright i znaczniki `start` / `ok` / `error` obejmują odczyt, ustawienie testowej
kłódki, otwarcie i sprawdzenie menu, odmowę Max dostępu, rozmowy A/B oraz sprzątanie.
Nazwy są stałe; logowanie nie zawiera argumentów IPC ani PIN-u. Zwracany wynik i wyjątek przechodzą
bez zmiany. Zachowano wszystkie asercje, operacje, warunek pomijania już ustawionego PIN-u,
120s testu,15s oczekiwania i brak ponowień. Kod Brokera i polityki pozostają dotychczasowe.

Sprawdzenie lokalne: Svelte/TypeScript0 błędów i0 ostrzeżeń, ESLint i Prettier zielone,
rozmiary3487 plików/264 crate'ów/0 naruszeń. Próba rzeczywistego Windows z instrumentacją
pozostaje do wykonania. Przyczyna zawieszenia jest otwarta; kolejnym krokiem jest odczyt
pierwszego niedokończonego kroku i błędu poprzedzającego sprzątanie w nowym przebiegu.

Osobno pełny Chromium w CI159 na169a6004: **890/890 E2E**,28,2 minuty, **573/573 jednostkowych**,
103 pliki. Typy0/0, lint, build, bundle i Storybook zielone. Ten wynik zamyka bieżącą szeroką próbę
atrapowego UI; historyczny lokalny877/2 pozostaje zapisany. Pełny Linux Rust nadal działa,
a Windows Rust z tego samego CI159 jest otwarty przy przygotowaniu tej notatki.
