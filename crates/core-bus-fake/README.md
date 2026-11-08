# core-bus-fake

Deterministyczna atrapa magistrali do testów innych modułów (docs/PLAN.md §4.5).
`FakeBus` rejestruje każde opublikowane zdarzenie (`recorded()`), nadaje deterministyczne
identyfikatory (`Uuid` z licznika) i znaczniki czasu z wirtualnego zegara (`VirtualClock`:
`now()`, `advance()`, `set()`), dostarcza zdarzenia subskrybentom synchronicznie w kolejności
publikacji, a `replay(order)` odtwarza nagrane zdarzenia w zadanej kolejności.
Subskrypcje mają nieograniczony bufor — fake nigdy nie gubi zdarzeń (`dropped == 0`).
Przechodzi ten sam zestaw testów kontraktowych co `core-bus-impl`.
