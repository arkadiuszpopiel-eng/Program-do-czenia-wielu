# Testy Planu i harness WebView2

Karta [#38](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/38).
Zmiana dotyczy testów i ich uruchamiania, bez zmiany kodu produkcyjnego UI, Brokera, polityk czy progów.

## Plan: zakończenie strumienia a karta decyzji

Pełny lokalny UI na `bd389207`, UI równoważne `7752ac70`: **877 zaliczonych / 2 nieudane /
0 pominiętych**,37,8 minuty. Glass light/dark nie zobaczyły karty końca Planu w5 sekundach.
[Zachowany wynik](artifacts/ui-full-rerun-2026-11-09.json). Oddzielny CI156 miał879/879.

Przed zmianą testu powtórzono wszystkie8 wariantów dwukrotnie: **16/16**,2,1 minuty.
Nie odtworzono w nich pierwotnej porażki pełnego zestawu. Następnie dodano wariant istniejącej
atrapy `slow-stream` (5 tokenów/s). Pojedynczy Glass light **nie przeszedł na tej samej asercji5 sekund**:
odpowiedź wciąż trwała. Test funkcjonalny zakładał więc krótki czas modelu, zamiast jawnie
sterować zakończeniem odpowiedzi. Nie stanowi to dowodu, że każda pierwotna porażka ma identyczną przyczynę.

Test Planu używa teraz zegara Playwright do timerów atrapy. Po wysłaniu wiadomości prowadzi
symulowany czas strumienia, a potem sprawdza kartę. Wariant wolny sprawdza dodatkowo, że podczas
generowania przycisk Stop jest widoczny, a karty jeszcze nie ma. Akceptacja nadal wymaga zgody
i zmienia poziom rozmowy; dopiero po zgodzie pojawia się polecenie wykonania.

Domyślne5 sekund asercji,60 sekund całego testu oraz wszystkie testy i progi wydajności pozostają
dotychczasowe.20 sekund przesuwanych w zegarze jest czasem symulacji atrapy. Zestaw funkcjonalny
nie służy do pomiaru szybkości LLM ani opóźnień rzeczywistego interfejsu.

Po zmianie: **16/16** (8 dotychczasowych +8 wolnych),1,5 minuty. Następnie pięć powtórzeń pary
Glass light/dark z wolnym strumieniem: **10/10**,1,5 minuty. Łącznie26 wykonań po zmianie.
Nowy pełny zestaw liczy887 przypadków i pozostaje do wykonania; wcześniejszego877/2 nie uznano za zielony.

## WebView2: warunki początkowe

[Rehearsal77](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42537977174)
na `7752ac70` miał **60 zaliczonych / 2 nieudane / 5 pominiętych**. Artefakt13641026194
potwierdził dwa błędy harnessu:

- `run.ps1` ustawiał `ALFA_E2E_INSTANCE` wyłącznie dla portu innego niż9222. CI używa domyślnego9222,
  a test plakietki wymaga tej flagi. Harness ustawia teraz flagę i port przy każdym swoim uruchomieniu.
  Dotychczasowa ochrona świeżego profilu pozostaje aktywna. Produkcyjna powłoka nadal nie interpretuje
  flag instancji testowej — jest to ścieżka buildu `e2e`.
- `z-night` próbował schować Sesje, choć snapshot miał przycisk „Pokaż panel Sesje” i nie miał nawigacji
  Sesji. Test najpierw otwiera panel kontrolką właściciela i potwierdza jego widoczność.

Skrypt pozostaje UTF-8 z BOM; ma213 zakończeń CRLF i0 samotnych LF. Komunikat zamkniętej
strony z pierwotnej próby pojawił się po120 sekundach czekania i sprzątaniu testu; sam nie dowodzi crashu.
**Poprawki WebView2 nie były uruchomione lokalnie na Linux.** Potrzebny jest rzeczywisty ponowny
przebieg Windows/WebView2 w CI; powyższe26 prób Chromium go nie zastępuje.

## Sprawdzenia i logi

Lint zmienionych TypeScriptów: zaliczony. Końcowe `pnpm check` projektu:0 błędów,0 ostrzeżeń.
Samodzielna próba `tsc` z ręcznymi opcjami nie obejmowała deklaracji globalnych i różniła się
od konfiguracji projektu; jej błędów nie uznano za zaliczony check.

```text
pnpm --dir apps/desktop/ui exec playwright test e2e/plan-mode.spec.ts --repeat-each=2
pnpm --dir apps/desktop/ui exec playwright test e2e/plan-mode.spec.ts --grep 'glass light — wolny strumień'
pnpm --dir apps/desktop/ui exec playwright test e2e/plan-mode.spec.ts
pnpm --dir apps/desktop/ui exec playwright test e2e/plan-mode.spec.ts --grep 'glass.*wolny strumień' --repeat-each=5
```

Próby uruchamiano kolejno, bez kompilacji Rust i bez modelu CPU. Bundle UI i źródła produkcyjne były
identyczne z `bd389207`; zależności i `dist` podłączono z worktree zakończonego pełnego UI.
Logi: `/workspace/scratch/ui-plan-determinism-before.log`, `ui-plan-slow-red.log`,
`ui-plan-clock-tests.log`, `ui-plan-clock-stress.log`, `ui-harness-check.log`.

Wersja0.0.85, etap J otwarty; wymagany niezależny przegląd i zielone CI przed scaleniem.
