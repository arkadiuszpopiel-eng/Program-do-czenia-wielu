# 54 — Jarvis i dalszy rozwój trybów, 8–9.11.2026

Karta [#22](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/22).
Zlecenie właściciela: rozwijać i testować program w Cloud do rana, do 8:00; Jarvis dostępny
dla każdej agentki. Właściciel potwierdził termin: **9.11.2026 o 08:00**; zachowano interpretację
czasu polskiego: **Europe/Warsaw = 07:00 UTC**. Szczegóły Jarvisa pozostają według roboczego
kontraktu; dalsze wskazówki właściciela mają pierwszeństwo.

## Aktualny stan,9.11 o06:26UTC

| Obszar | Ostatni dowód | Co pozostaje |
| --- | --- | --- |
| Pięć trybów | jawny zakres rozmowa/globalnie, Plan bez rozmowy, regresje zgód i kłódki | niezależny przegląd i odbiór sprzętu |
| Jarvis wbudowanych i własnych person | wspólny profil; cztery osobne przełączniki; Luna po restarcie katalogu10/10 tur Bielika | trwała obsada rozmowy #37; osobne przełączniki własnych person |
| Kreator #44 iGłos #49 | CI162 na d18d9433:901/901 i573/573, typy0/0 | #50 dwa wspólne przepływy zaliczone; pełne903 i native68 do wykonania |
| Rust Linux | [pełne3495/0/25 --all-features](../reviews/2026-11-09-validation-12d44af4.md) na12d44af4,77 integracji | chroniony LinuxClippy i przegląd |
| Rust Windows | [CI162:3521/0/46](../reviews/2026-11-09-ci162-and-review.md), Clippy0 | przegląd i sprzęt |
| Windows/WebView2 | rehearsal82 i83:62/0/5 | #50 native-wrapper w nowym CI; sprzęt i mikrofon |
| Zwięzłość Bielika | [#48:50 ciał i50 surowych odpowiedzi](../reviews/2026-11-09-jarvis-wire.md); Jarvis25/25 imion | jedno zdanie0/25 także przed dekoderem; Standard Bety/Gamy błędne imiona |
| CI i przegląd | Tauri, cargo-deny, dokumentacja zielone; #51 prywatna łatka ma pełnyClippy0 i4/4 | publicznyLinuxClippy nadal czerwony; #52 dwa HTTP504 silników83; świeży przegląd |

Kolejność do07:00UTC:

1. Zebrać nowy native-wrapper Jarvisa #50 i pełny nowy Chromium903. Kod produkcyjny pozostaje
   identyczny z zielonymi Windows3521/0/46 i Chromium901/901 wCI162. Linux --all-features3495/0/25 zakończony.
   Nie przepisywać wcześniejszych porażek jako zaliczeń.
2. Jeśli nowe Windows wykaże porażki, odtworzyć i poprawiać małe zakresy bez zmiany limitów;
   chronione poprawki pozostają łatkami do przeglądu człowieka.
3. Opublikować raporty sprawdzonego przyrostu, aktualny plan/roadmapę i wygenerowane HTML;
   wstrzymać aktualizację ref podczas bieżącego CI, aby go nie anulować.
4. Zebrać końcowy raport z commitami, dokładnymi wynikami i otwartymi punktami. Po07:00UTC
   nie zaczynać zmian. Termin9.11 o08:00 Europe/Warsaw pozostaje bez korekty.

Dalszy backlog: obsada rozmowy #37; indywidualny profil własnych person; różnorodne zadania
jakościowe i porównanie odpowiedniejszego modelu bez dostrajania do jednego pytania; rzeczywisty
mikrofon, odsłuch i widoczny pulpit Windows. [Raport nocny](../reviews/2026-11-08-nightly-modes-jarvis.md)
zachowuje poprzednie wyniki, porażki i odrzucone warianty. [Karta #22](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/22)
obejmuje całość; [draft PR24](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/24)
pozostaje bez scalenia.

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
- O 07:00 UTC zakończyć nowe zmiany i przedstawić wynik. Etap J pozostaje otwarty do przeglądu i próby sprzętowej;
  obecna seria robocza nie stanowi wydania, wersja pozostaje 0.0.85.

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
