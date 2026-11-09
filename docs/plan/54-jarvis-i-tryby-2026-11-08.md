# 54 — Jarvis i dalszy rozwój trybów, 8–9.11.2026

Karta [#22](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/22).
Zlecenie właściciela: rozwijać i testować program w Cloud do rana, do 8:00; Jarvis dostępny
dla każdej agentki. Właściciel potwierdził termin: **9.11.2026 o 08:00**; zachowano interpretację
czasu polskiego: **Europe/Warsaw = 07:00 UTC**. Szczegóły Jarvisa pozostają według roboczego
kontraktu; dalsze wskazówki właściciela mają pierwszeństwo.

## Aktualny stan,9.11 o05:12UTC

| Obszar | Ostatni dowód | Co pozostaje |
| --- | --- | --- |
| Pięć trybów | jawny zakres rozmowa/globalnie, Plan bez rozmowy, regresje zgód i kłódki | niezależny przegląd i odbiór sprzętu |
| Jarvis wbudowanych i własnych person | wspólny profil; cztery osobne przełączniki; Luna po restarcie katalogu10/10 tur Bielika | trwała obsada rozmowy #37; osobne przełączniki własnych person |
| Kreator #44 |9 nowych E2E, pełny899/899 i573/573 jednostkowych, typy0/0 | nowy pełny Chromium CI161 obejmuje901, trwa |
| Rust Linux | [pełne3492/0/24](../reviews/2026-11-09-full-validation.md) na169a6004, domyślne cechy | ponowienie --all-features na zamrożonym12d44af4 trwa |
| Rust Windows | CI160:3516/5/46; #45 ma7/7 Linux, #46 MSVC crosscheck+Clippy0 | CI161 na12d44af4 wykonuje pełne testy |
| Windows/WebView2 | [rehearsal82:62/0/5](../reviews/2026-11-09-webview-green.md), kłódka3,0s i dokowanie38,4s | pięć jawnych pominięć, sprzęt i rzeczywisty mikrofon |
| Zwięzłość Bielika | [#48:50 ciał i50 surowych odpowiedzi](../reviews/2026-11-09-jarvis-wire.md); Jarvis25/25 imion | jedno zdanie0/25 także przed dekoderem; Standard Bety/Gamy błędne imiona |
| CI i przegląd | Tauri, cargo-deny, dokumentacja zielone | LinuxClippy: chronione importy Brokera, łatka dla człowieka; świeży przegląd |

Kolejność do07:00UTC:

1. Zebrać pełne wyniki Rust --all-features Linux, Windows CI161 i Chromium901 bez nakładania
   ciężkich lokalnych prób. Nie przepisywać wcześniejszych porażek jako zaliczeń.
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

## Prewarunek rzeczywistego menu, #47

[Regresja modalu Głosu](../reviews/2026-11-09-composer-modal.md):Windows81 wskazał blokadę kliku,
Chromium1028px odtworzył przechwycenie przez scrim. Helper zamyka modal zwykłym przyciskiem;
2/2 po zmianie, typy0/0,120s i polityka dotychczasowe. Rzeczywisty Windows do wykonania.

## Ponowienie Windows i poprawki fixture,04:53UTC

CI160 na974ae45e: Windows **3516 zaliczonych,5 nieudanych,46 pominiętych**; te same dwa
przypadki conductor_handoff, załącznik files oraz dwa kodowania. UI CI160 zaliczone;
WebView81 **60/1/6**, PIN ustawiany i włączany poprawnie, blokada przy kliku menu za modalem Głosu.

[Fixture rdzenia #45](../reviews/2026-11-09-windows-core-fixtures.md):7/7 na Linux i Clippy bez
ostrzeżeń. Komendy delegowania mają deterministyczny FakeExec; załącznik zachowuje rzeczywisty
Broker, lecz kolejne fazy działają w odrębnych, całkowicie zamykanych runtime.
[Fixture kodowania #46](../reviews/2026-11-09-windows-encoding.md): jawny UTF8 pliku PowerShell,
przypadek CMD wybierany niezależnym, ścisłym enkoderem OEM. MSVC crosscheck i Clippy zaliczone;
**nie jest to wykonanie na Windows**. Żadne asercje polityki,300ms,120s ani progi evals nie zmienione.
Nowy rzeczywisty Windows/WebView2 pozostaje kolejnym krokiem przed terminem07:00UTC.

[Diagnoza przewodu #48](../reviews/2026-11-09-jarvis-wire.md):50 ciał HTTP z atrapy i50 bezpośrednich
odtworzeń rzeczywistego Bielika; Jarvis nadal0/25 jednego zdania przed dekoderem, imiona25/25.
45/50 tekstów identycznych,5 różnic Standard Luny zachowano; nie deklarujemy pełnej zgodności.
