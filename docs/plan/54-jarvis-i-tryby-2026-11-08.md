# 54 — Jarvis i dalszy rozwój trybów, 8–9.11.2026

**Blokada aktualnegoCI167:** [RUSTSEC-2026-0459 / SQLite–SQLCipher](../reviews/2026-11-09-ci167-sqlite.md).
Kandydat ma naprawiony SQLCipher4.20/SQLite3.54.1,33/0/1 testów bazy,73/0/3 modułów i15/0 Core.
Właściciel zatwierdził wyłącznie wąską łatkę licencji51e27c10; po zastosowaniu lokalny audyt4/4.
Nadal wymagane nowe pełneCI oraz niezależny przegląd.
Nie wyłączono szyfrowania ani nie ignorowano CVE. Bielik #41/#48 także pozostaje otwarty.

Karta [#22](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/22).
Pierwotne okno kończyło się9.11.2026 o08:00 Europe/Warsaw (07:00UTC).
**Właściciel zmienił zlecenie9.11 o06:53UTC: praca trwa do usunięcia obecnie wykrytych usterek.**
Nie dodajemy nowych funkcji ani kolejnych ulepszeń. Po rozliczeniu napraw: pełny raport, zapis
wyników i zakończenie prac do odwołania. Powtarzany opis pierwotnego okna nie unieważnia tej
konkretnej korekty. Zakaz commitowania chronionych modułów bez przeglądu pozostaje w mocy.

## Zamknięta lista obecnych usterek

| Karta | Usterka | Stan |
| --- | --- | --- |
| #53 | surowe klucze tłumaczeń system/vision i koloru w Kreatorze | naprawiona:905/905 Chromium,63/0/5 Windows/WebView2; obraz potwierdzony |
| #41/#48 | Bielik: Jarvis ignoruje jedno zdanie; Standard myli imiona Bety/Gamy | nierozwiązana; kandydat odrzucony po14/28 innych pytań i6 fikcyjnych potwierdzeniach wykonania |
| #37 | obsada rozmowy znika po restarcie | naprawiona;76 testów modułów i17 integracji, pełneCI166 zielone |
| #51 | publiczny LinuxClippy blokują importy chronionego testu | naprawiona po zgodzie właściciela; pełnyClippy0 i4/4, pełneCI166 zielone |
| #52 | pobieranie silników okresowo kończy HTTP504 | naprawiona obsługa ponowień;82/0/1 iClippy0;CI166 oraz Windows87 zielone |

Backlog nowych funkcji, w tym indywidualne przełączniki własnych person, jest zamrożony.
Nowe usterki będące skutkiem napraw podlegają regresji; nie rozszerzamy listy o nowe ulepszenia.

## Baza testowa przed naprawami,9.11 o07:14UTC

| Obszar | Ostatni dowód | Co pozostaje |
| --- | --- | --- |
| Pięć trybów | jawny zakres rozmowa/globalnie, Plan bez rozmowy, regresje zgód i kłódki | niezależny przegląd i odbiór sprzętu |
| Jarvis wbudowanych i własnych person | wspólny profil; cztery osobne przełączniki; Luna po restarcie katalogu10/10 tur Bielika | trwała obsada rozmowy #37; osobne przełączniki własnych person w zamrożonym backlogu |
| Kreator #44 iGłos #49 | CI163 na35df604b:903/903 i573/573, typy0/0 | tylko naprawy z listy powyżej |
| Rust Linux | [pełne3495/0/25 --all-features](../reviews/2026-11-09-validation-12d44af4.md) na12d44af4,77 integracji | chroniony LinuxClippy i przegląd |
| Rust Windows | CI163:3521/0/46, Clippy0 | przegląd i sprzęt |
| Windows/WebView2 | rehearsal84:63/0/5, bezpośredni Jarvis zaliczony | sprzęt i mikrofon |
| Zwięzłość Bielika | [#48:50 ciał i50 surowych odpowiedzi](../reviews/2026-11-09-jarvis-wire.md); Jarvis25/25 imion | jedno zdanie0/25 także przed dekoderem; Standard Bety/Gamy błędne imiona |
| CI i przegląd | Tauri, cargo-deny, dokumentacja zielone; #51 prywatna łatka ma pełnyClippy0 i4/4 | publicznyLinuxClippy nadal czerwony; #52 dwa HTTP504 silników83; świeży przegląd |

Kolejność pracy: naprawy z powyższej listy w osobnych worktree, celowane regresje, odpowiednie
pełne testy/CI, publikacja sprawdzonych przyrostów i aktualizacja raportu. Nie przypisywać starych
wyników nowym zmianom. Pomiary modelu i czasowe UI uruchamiać kolejno bez konkurującego obciążenia.
Po naprawach pełny raport z dokładnymi wynikami i ograniczeniami, potem STOP do odwołania.
Niezależny przegląd, zieloneCI i odbiór sprzętu pozostają warunkami scalenia/akceptacji.

## Zachowanie Jarvisa

Opis: [kontrakt profilu](../modules/personas/INTERACTION.md). Panel **Agentki** (`Alt+1`)
pozwala wybrać Standardowy / Jarvis / Jak ustawienie wspólne. Ustawienie wspólne znajduje się
w **Ustawienia → Agentki** oraz przy bibliotece **Kreatora**. Własne persony dziedziczą ten profil;
osobne przełączniki dotyczą obecnie czterech wbudowanych. Profil obowiązuje od nowej odpowiedzi lub nowego przebiegu.

Jarvis dodaje krótkie meldunki, instrukcję wykonania zlecenia i sprawdzenia wyniku oraz odpowiedzi
nadające się do odsłuchu. Pozostają imię, role, głos i pamięć persony. Właściciel osobno włącza
mikrofon. Tryb zgód, dostępne narzędzia i limity zadania nadal wyznaczają zakres działania.
Werdykt wewnętrznej Krytyczki zachowuje dotychczasowy format.

## Rozwój każdego trybu

Dalsza regresja: [Plan bez aktywnej rozmowy, karta #25](../reviews/2026-11-08-plan-without-session.md).
Interfejs wymaga otwarcia rozmowy przed Planem; skrót bez rozmowy pomija Plan. Wyniki szerokich
prób i ograniczenie dysku opisano w tym raporcie.

| Tryb | Nowa możliwość w tej serii | Sprawdzenie |
| --- | --- | --- |
| Plan | jawnie pokazany zakres rozmowy; wyjście nadal dotyczy tej rozmowy | test zakresu i dotychczasowe testy menu |
| Ręcznie | wybór tej rozmowy lub wszystkich rozmów | test wyboru zakresu i E2E |
| Akceptuj edycje | ten sam jawny wybór zakresu | test jednostkowy ścieżki prośby |
| Automatycznie | ten sam wybór zakresu; regresje modelu-sędziego w PR #20 | test zakresu i integracja Z.8 |
| Pełny dostęp | ten sam wybór zakresu z dotychczasową zgodą Brokera | test zakresu i kłódki rodzica |

## Zasady dalszej sesji

- Czytać `AGENTS.md`, ten plan i [raport bieżący](../reviews/2026-11-08-nightly-modes-jarvis.md).
- Pracować na `feat/nightly-modes-jarvis`; nie nakładać równoległych zapisów na tę samą gałąź.
- Przy każdym problemie zapisać odtwarzalny przypadek; nie powiększać limitów ani nie wyłączać bramek,
  żeby uzyskać zielony wynik. Pomiary modelu i testy czasowe UI uruchamiać bez konkurujących obciążeń.
- Wyniki Linux/headless, fake audio, Chromium oraz Windows/WebView2 i prawdziwy mikrofon opisywać osobno.
- Zmiany chronionych modułów przygotować jako łatkę do przeglądu człowieka; nie commitować ich automatycznie.
- Publikować sprawdzone przyrosty jako draft PR. Niezależny przegląd i zielone CI pozostają warunkiem scalenia.
- Nie uruchamiać mostów CLI z harmonogramu. Nie pobierać kolejnych wielkich modeli bez potrzeby.
- Pierwotny termin07:00UTC został zastąpiony korektą właściciela. Po rozliczeniu obecnych usterek
  zapisać pełny raport i zakończyć prace do odwołania. EtapJ i wersja0.0.85 pozostają bez wydania.

## Dowody szczegółowe

[Pełne12d44af4](../reviews/2026-11-09-validation-12d44af4.md) ·
[Kreator #44](../reviews/2026-11-09-builder-jarvis.md) ·
[Fixture rdzenia #45](../reviews/2026-11-09-windows-core-fixtures.md) ·
[Kodowanie #46](../reviews/2026-11-09-windows-encoding.md) ·
[Modal #47](../reviews/2026-11-09-composer-modal.md) ·
[Windows82](../reviews/2026-11-09-webview-green.md) ·
[Przewód #48](../reviews/2026-11-09-jarvis-wire.md) ·
[Kreator → Głos #49](../reviews/2026-11-09-builder-voice.md).

Nieskuteczne warianty promptu pozostają cofnięte; zamrożone evals, limity i polityki dotychczasowe.

[Bezpośredni WebView Jarvisa #50](../reviews/2026-11-09-jarvis-webview.md) ·
[CI162, prywatna łatka #51 iHTTP504 #52](../reviews/2026-11-09-ci162-and-review.md).

[Etykiety Kreatora #53 i bazaCI163/Windows84](../reviews/2026-11-09-builder-labels.md).

[Trwała obsada #37](../reviews/2026-11-09-cast-restart.md).

[Zatwierdzona naprawa #51](../reviews/2026-11-09-broker-imports-approved.md).

[Przerwy po chwilowych HTTP504 #52](../reviews/2026-11-09-http-outage.md).

## Aktualizacja napraw9.11

[Raport napraw i rzeczywistych prób Bielika](../reviews/2026-11-09-jarvis-model-repair.md).
Baza cc6f0ac9: CI166 Linux3504/0/25, Windows3530/0/46, Chromium905/905 iVitest573/573;
Windows87 WebView2 63/0/5, syntetyczne silniki1/0/2 filtrowane. Wynik modelu jest oddzielną
bramką jakości; sama poprawna propagacja instrukcji nie zamyka #41/#48. Nowe funkcje pozostają zamrożone.
