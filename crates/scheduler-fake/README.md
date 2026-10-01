# scheduler-fake

Atrapa pełnego schedulera: ten sam `SchedCore` co `-impl`, **wirtualny zegar** (`advance`, `wait` sam
przesuwa czas), skryptowane wykonawczynie (`script(task, Script)`), nagrane zdarzenia, stan w pamięci
(`restart()` = nowy rdzeń z zapisanego stanu), ustawialna decyzja budżetu tła, `spawn` (delegacja).
Tylko jako dev-dependency.

Testy akceptacyjne F5 (zestawy w `evals/F5/`): `tests/props.rs` — F5-03, 0 zakleszczeń w 1000 losowych
scenariuszy; `tests/parallel.rs` — F5-01, 0 konfliktów zasobów w 100 scenariuszach równoległych agentek;
`tests/steering.rs` — F5-02, steering ≤ 1 krok atomowy 20/20; `tests/contract.rs` — wspólny kontrakt.
