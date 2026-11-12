# #83 — zdarzenie końca po zapisaniu faktów i zakończeniu generacji

CI192 na5f265ba3: Windows spy.rs:241 dostał1aktywną generację zamiast0 po trzechStop.
Źródło finish_generation emitowałoStop/Error przed synchronicznym zapisem osi czasu
oraz usunięciem uchwytu z rt.gens. Odbiorca mógł więc zobaczyć koniec przy aktywnym stanie.

Nowa regresja naLinux odtwarza kolejność niezależnie od szybkości dysku:
TimelineModelCall ma indeks10, aStop9; **RED0/1, kod101**. Źródło finish identyczne zCI192.
Poprawka w zwykłym app-chat: najpierw istniejący zapis/timeline i usunięcie uchwytu
z kontrolą identyfikatora tury, następnieStop/Error. Zapis łączności nadal przedError.
Nie zmieniono kill-switcha, Brokera, polityk, limitów ani oczekiwanego0 w spy.

Regresja plus istniejące spy/spy_work/spy_modes/errors: **10/0/0**.
Clippy app-chat+app-core/all-targets **0 bez ostrzeżeń**, fmt i diff0.
To testyLinux z atrapą modelu/portów, nie pełny odbiórWindows lub sprzętu.
[Surowe logi, kodRED i hashe](artifacts/2026-11-12-terminal-order/README.md).

[Karta83](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/83)
pozostaje otwarta do własnegoCIWindows i niezależnego przeglądu. Fix jest niezależny
od nieodebranej semantyki #58 oraz transportu telefonu. Nie zmienia tekstów modelu.
