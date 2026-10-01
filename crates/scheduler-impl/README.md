# scheduler-impl

Moduł `scheduler` (module.toml, `lifecycle = "always"`): `SchedulerModule` implementuje `Scheduler`,
`SchedulerLite` (w kompozycji **zastępuje `scheduler-lite-impl`** — jedna tablica blokad) i `Module`.
Zegar = epoka + tokio `Instant`; sterownik terminów; wykonawczynie jako przerywalne zadania tokio za
portem `TaskExecutor` (panika = błąd nieponawialny); zdarzenia `scheduler.*` na magistrali; stan
w `FileSnapshotStore` (zapis atomowy, wersje `revision`, zatrzymanie łagodne → wznowienie po starcie);
budżet tła `CostMeterBudget` (ta sama reguła `evaluate` co `cost-meter`).
Testy: kontrakt na zatrzymanym zegarze tokio, plik stanu i restart, budżet z `cost-meter-fake`, awaria
wykonawczyni, zdarzenia, kill-switch.
