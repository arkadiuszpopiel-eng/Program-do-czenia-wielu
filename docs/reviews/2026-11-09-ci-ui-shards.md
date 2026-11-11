# CI — podział E2E bez podnoszenia limitów, 9.11.2026

Karta[#55](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/55).
Baza`3054e9e9` zawiera naprawę Routera[#54](2026-11-09-enterprise-router.md).
To osobny przyrost infrastruktury w`/workspace/wt-enterprise-ci`.

## Rzeczywista porażka CI169

[CI169](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42686483119)
na`e3f70a8b` ma niezaliczone zadanieUI. Log129394564293 zawiera905 passed(30.1m), po czym
GitHub zgłasza timeout kroku E2E po30 minutach. Wszystkie scenariusze zaliczone nie oznaczają
zielonej bramki CI. Nie ukrywamy tego przez samo ponowienie. Pozostałe zadania CI169 są zielone;
Windows91 ma wszystkie4 zadania zielone. Nie przypisujemy tego nowej naprawie Routera.

## Zmiana

- Dotychczasowe kontrole UI (lint,typy,Vitest,bundle,Storybook) pozostają w`ui-build`.
- Jeden zbudowany`apps/desktop/ui/dist` jest przekazywany artefaktem tego samego przebiegu.
  Brak pobierania artefaktu z innej gałęzi, innego commitu lub „ostatniego zielonego” zadania.
- Trzy zadania na oddzielnych runnerach uruchamiają`--shard=1/3`,`2/3`,`3/3` na tym samym UI.
  `fail-fast:false` pozwala dokończyć pozostałe części po porażce jednej.
- Krok E2E nadal ma30 minut; timeout60s pojedynczego testu i wszystkie budżety działań pozostają
  niezmienione. Nie dodano retries,grep ani pomijania testów. Instalacja Chromium zachowuje
  dotychczasowy limit15 minut i ograniczone próby pobierania.
- Zachowano nazwę bramki`UI (Svelte 5)`: uruchamia się także po błędzie zależności i jest zielona
  **wyłącznie**, gdy kontrole/build i wszystkie trzy części E2E zakończą się sukcesem.
  Awaria,anulowanie lub pominięcie zależności nie daje zielonego UI po samym buildzie.

## Sprawdzenie lokalne

Playwright z przypiętego workspace zwrócił listę905 testów. Podział to **302+302+301**;
zbiory identyfikatorów są parami rozłączne, ich suma to dokładnie pełny zbiór905.
Weryfikacja YAML potwierdziła trzy części,`fail-fast:false`,zależności bramki oraz30 minut.
Zbudowano UI (18,07s) i sprawdzono transfer144 plików: identyczne SHA256 przed i po.
Na odtworzonym artefakcie wykonano **9/9 przypadków Jarvisa** w trzech częściach3+3+3
(19,0s/15,4s/17,6s), headless Linux/Chromium. Wcześniejszy mały zakres etykiet miał dwa testy
i pustą trzecią część; dlatego kontrola przekazania używała następnie dziewięciu przypadków,
aby każda część rzeczywiście wykonała testy. Wszystkie logi zachowane. Nie jest to WebView2,
mikrofon ani pełne905 na nowym workflow. Lint i generatory102 stron:0 naruszeń.

Wykonano też16 kombinacji stanów build/E2E: jedynie success/success zalicza rzeczywiste polecenie
bramki. Nie jest to wykonanie całych905 scenariuszy na nowym workflow.

Pierwsze lokalne polecenie listowania wybrało Pythonowy Playwright z PATH, bo w nowym worktree
brakowało dowiązania zależności UI: kod1,unknown command test. Po podpięciu istniejących
przypiętych zależności JS wszystkie cztery listy poprawne. To błąd procedury lokalnej, nie testUI;
zachowany w`/workspace/scratch/enterprise-ci/list-initial-error.txt`.

```sh
pnpm --filter @alfa/desktop-ui exec playwright test --list --reporter=json
pnpm --filter @alfa/desktop-ui exec playwright test --list --reporter=json --shard=1/3
pnpm --filter @alfa/desktop-ui exec playwright test --list --reporter=json --shard=2/3
pnpm --filter @alfa/desktop-ui exec playwright test --list --reporter=json --shard=3/3
```

[Wyniki i odciski](artifacts/ci-ui-shards-validation.json) ·
[Pełny zbiór i trzy listy identyfikatorów](artifacts/ci-ui-shards-membership.jsonl).
Log poprzedniej porażki:`/workspace/scratch/enterprise-ci/ci169-ui.log`.
Wymagane rzeczywiste CI nowego commitu i niezależny przegląd przed scaleniem.
