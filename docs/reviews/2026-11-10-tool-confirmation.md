# Stan narzędzia wymagającego potwierdzenia — 10.11.2026

Karta[#57](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/57),
baza`aa2e75ffaa296b9b09293e8ba39362d4c955da64`,worktree`/workspace/wt-tool-confirmation`.
Komponent: projekcja zdarzeń narzędzi do rozmowy,jej DTO/wiązania oraz widok.
Obowiązuje termin właściciela: rozwój do05:30PL10.11,potem testy/raport i STOP06:00PL.

## Błąd i zachowanie po poprawce

`StepStatus::NeedsConfirmation` trafiał do Replay jako`NeedsConfirmation`, ale do wątku jako
`ToolStatus::Done`. To gubiło rozróżnienie między wykonaniem i propozycją czynności. Rzeczywiste
przykłady w narzędziach: `shell_terminal` tylko proponuje polecenie do skopiowania; trwałe
usunięcie może zwrócić kartę potwierdzenia bez usunięcia pliku.

Nowe DTO zachowuje`needs_confirmation`. Projektor,Replay i zdarzenie ToolCall mają zgodny stan.
Wątek pokazuje licznik „Do potwierdzenia” i etykietę „Wymaga potwierdzenia”, także po angielsku,
oraz nie oferuje „Cofnij” dla propozycji. Dotychczasowe zachowanie przycisku dla innych stanów
z tokenem cofania jest zachowane, w tym możliwość cofnięcia częściowego skutku kroku z błędem.
Otwarcie terminala samo nie oznacza wykonania polecenia i nie zamienia propozycji w Done.

Schemat i TypeScript wygenerowano z Rusta. Test round-trip zachowuje poprzednie wartości
`running/done/error` i nową`needs_confirmation`. Nie przepisano historycznie zapisanych Done
do innego stanu — bez dodatkowego dowodu nie można zgadywać ich znaczenia. Brak zmian decyzji
Brokera,polityk,uprawnień,progów evals,wywołań narzędzi lub promptów modeli.

## Odtworzenie i weryfikacja

| Sprawdzenie | Wynik |
| --- | --- |
| Rust RED | 0 zaliczonych/1 nieudany,13 filtrowanych,kod101:NeedsConfirmation było Done |
| UI RED,classic light | 0/1,kod1:brak jawnego stanu,pozostała tylko linia „3 kroki narzędzi” |
| Rust app-api/app-agents,all-features | **73 zaliczone/0 nieudanych/3 ignorowane**,20 wyników celów z doctestami |
| Generowanie schematu IPC | 4/4,to podzbiór powyższych73;wygenerowane wiązania zgodne |
| Clippy all-targets/all-features | kod0,bez ostrzeżeń,1m56s |
| Jednostkowe UI | **573/573**,103 pliki |
| Svelte obu pakietów | 0 błędów/0 ostrzeżeń |
| E2E,4 style×2 motywy+EN+360px | pierwszy GREEN10/10;końcowy po przeglądzie10/10,bez zwiększenia limitów |

E2E potwierdza zgodne stany dwóch wykonanych kroków i jednej propozycji,brak spinnera po
zakończeniu kroku,jawne PL/EN,brak przycisku cofania propozycji,niezmieniony stan po otwarciu
terminala,brak poziomego przepełnienia w360px oraz0 krytycznych/poważnych naruszeń axe.
Zrzuty classic i mobile obejrzane; lokalnie`/workspace/scratch/tool-confirmation/confirmation-*.png`.
To headlessLinux/Chromium i atrapa backendu,nie faktyczne przeniesienie14 plików z demonstracji,
otwarcie terminala właściciela,model chmurowy ani odbiór Windows/WebView2,GPU lub mikrofonu.

Po dodaniu10 przypadków jest **915 testów UI**,305 na każdą z3 części. Sprawdzone identyfikatory
potwierdzają zachowanie wszystkich dawnych905 i dokładnie10 nowych,bez duplikatów lub braków.
Weryfikacja list nie jest wykonaniem pełnych915; to wymaga nowegoCI po publikacji.
Pierwsza kontrola rozmiaru wykazała417 linii słownika pl-app przy limicie wyjątku416; nowe
klucze przeniesiono do słowników agentek. Wyjątek nie został zwiększony.

Komendy i odciski:[walidacja](artifacts/tool-confirmation-validation.json).
Logi,w tym obie porażki RED i wszystkie przebiegi UI:zapisane w`/workspace/scratch/tool-confirmation/`.

```sh
UPDATE_SCHEMAS=1 cargo +1.94.1 test --locked -p app-api --test ipc_schema -j2
node scripts/gen-ipc-types.mjs
cargo +1.94.1 test --locked -p app-api -p app-agents --all-features -j2
cargo +1.94.1 clippy --locked -p app-api -p app-agents --all-targets --all-features -j2 -- -D warnings
pnpm check
pnpm --filter @alfa/desktop-ui test
CI=1 PW_WORKERS=1 pnpm --filter @alfa/desktop-ui exec playwright test e2e/tool-confirmation.spec.ts
```

## Baza CI i dalsze kroki

CI171 naaa2e75ff zakończone sukcesem10/10:Linux3520/0/26,Windows3546/0/47,po1129 wyników
celów;UI905/905,Vitest573/573,Svelte0/0. WebView2Windows93 ma63/0/5;zbieranie wyników
przebiegu silników trwało przy przygotowaniu przyrostu. Poczekano do zielonego zakończenia
całegoWindows93(4/4success) przed publikacją; nie przerwano go nowym pushem.
Wyniki te dotyczą bazy,nie nowego DTO. Wymagane noweCI i niezależny przegląd przed scaleniem.

#41/#48 i jakość odpowiedzi modeli pozostają otwarte. To naprawa faktów projekcji,nie dowód,
że model przestał wymyślać wykonanie czynności. Produkcyjny dostawca wyszukiwania WWW nadal
nie jest skonfigurowany. Sprzęt docelowyRTX2060 12GB,prawdziwy mikrofon i API czekają na odbiór.
