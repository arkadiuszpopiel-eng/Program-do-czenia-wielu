# #85 — test ponawiania po faktycznym zakończeniu zadania

[Windows120](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/43171040447),
źródło a2b47cf284bbfb5454d4e807e5da5717e4d5f60e: WebView2 **62 passed /1 failed
/5 skipped**. Porażka z-lifecycle: tasks_retry zwróciło invalid_input
„Ponowić można tylko zakończone zadanie”. Pozostały job silnika tego przebiegu
był jeszcze w toku podczas diagnozy; nie deklarujemy ukończenia całego workflow.

Przyczyna w teście: tasks_cancel dla aktywnego zadania zgłasza Stop, a wykonawca
kończy w punkcie atomowym (scheduler-contract/engine/control.rs). Ten kontrakt
nie obiecuje stanu terminalnego przy samym zwrocie listy objętych zadań.
Test natychmiast ponawiał, podczas gdy testRust prawidłowo czeka na zdarzenie końca.

Poprawka tylko w e2e-app/z-lifecycle.spec.ts: przez istniejące call/schema sprawdza
tasks_list aż stan tego samego zadania będzie done, potem **jedno** tasks_retry
i dotychczasowe sprawdzenie nowegoID. Odczyt w poll nie wykonuje akcji. Brak sleep,
ponawiania całego scenariusza, zmian API, kryterium lub budżetów. Obowiązuje istniejący
expect15s, test120s, retries0; hash playwright.app.config.ts pozostał identyczny.

LokalnieLinux: istniejące kontroleCore tasks **3/0/0**, pełny pnpm lint0,
Prettier zmienionego pliku0, kolekcjaPlaywright **68 testów/23 pliki**.
Kolekcja nie jest wykonaniem testów. Brak lokalnegoWindows/WebView2; oryginalnyRED
pochodzi z prawdziwej aplikacjiWindows, jej nowyGREEN i niezależny przegląd nadal
wymagane przed zamknięciem [#85](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/85).
[Logi, dokładne komendy i hashe](artifacts/2026-11-12-webview-task-wait/README.md).

Osobno Windows118 na fb1bd5f0 ponownie nie zaliczyło Core8B: pierwszy przypadek
Alfa/Standard, input_ok=true,0tekstu/FirstToken60s,78243ms,29NOT_RUN.
SemantykaNOT_EVALUATED, #58 nadal otwarte. Naprawa czekania w teście zadań nie
zmienia dostawcy, tekstu modelu lub limitów i nie jest naprawą tego timeoutu.
