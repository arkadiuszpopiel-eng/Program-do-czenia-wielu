# Diagnostyka browser_search — naprawa integracji

[CI193](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/43159904243)
na8b66e957 zakończyło się8/10: oba zadaniaRust zatrzymał test diagnostyki narzędzi.
Odczyt surowych podsumowań: Linux3655/1/28,Windows3677/1/49, po1154podsumowania.
Nie podwajano liczb z numerowanego przedruku porażek. Tauri i wszystkie częściUI
zaliczone; Windows117 ma4/4 zadań, bez przypisywania temu odbioru semantyki modelu.

Przyczyna była w fixture: narzędzia browser_* dzieliły stronę formularza. Dodane
wyszukiwanie nawigowało na inną stronę i select/type dostawały NotFound. Osobna
sesja browser_search przywraca oba wcześniejsze wyniki. Po tej naprawie test nadal
nie przechodził (1/1), wskazując już tylko brak nowego narzędzia we wzorcu.

Uruchomiono istniejący generator ALFA_TOOL_DIAG_UPDATE=1. Porównanie strukturalne
potwierdza **81/81 dawnych wpisów bez zmian, zero usuniętych, jeden dodany:
browser_search:ok**. Zwykłe cargo test --locked -p app-core --test tool_diag -j2
bez generatora: **2/0/0**. Clippy tego celu z-Dwarnings0, fmt0.
Nie zmieniono progów evals, budżetów, polityk lub produkcyjnego Brokera.

[Logi i hashe](artifacts/2026-11-12-browser-diag/README.md) obejmują również
pierwotne czerwoneCI. Lokalne próby używają atrap i nie dowodzą odczytu realnych
wynikówWWW, jakości rankingu, osadzeniaWebView2 lub działania telefonu.
Nowy head nadal wymaga własnegoCI i niezależnego przeglądu przed merge.

Po integracji opublikowanej bazyPR81 (fb1bd5f0, poprawki lockfile i #83) ponowiono
wspólnie tylko dotknięte kontrole: tool_diag/finish_order/spy/spy_work/spy_modes/errors
**12/0/0**. Dokumentację regenerowano z kodu po scaleniu; bez ręcznej edycjiHTML.
Windows117, kod8b66: WebView2 **63/0/5**, Runtime156.0.4314.58; macierzCore30/30
transportu i wejścia,30odpowiedzi„Nie”,0NOT_RUN, pierwszytekst4475–32594ms.
Ponownie dodatni fixture to jedynieTurnMeta, więc semantykaNOT_ACCEPTED.
[Pełne dowodyCI193/Windows117 i integracji](artifacts/2026-11-12-ci193/README.md).
