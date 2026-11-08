# diagnostician-fake

Atrapa Diagnosty (tylko `dev-dependencies`):

- `FakeDiagnostician` = rdzeń `DiagnosticianCore` z `FakeDiagHost` (wirtualny zegar `ManualClock`, zdarzenia i dziennik
  w pamięci) — przechodzi te same testy kontraktowe co `diagnostician-impl`.
- `ChaosWorld` — symulowany system (konfiguracja z rewizjami, pliki na woluminach C:/D:, magazyny wpisów, porty, GPU,
  sieć, zegar, trasy dostawców, budżet) jako atrapa portów `RepairEnv` + `RepairContext`, oraz `ChaosBroker` (Broker
  wykonujący naprawy Jądra po „fizycznym potwierdzeniu”); kroki Diagnosty i Brokera rejestrowane osobno.
- `FAULTS` / `inject` / `healthy` — katalog 24 awarii (`evals/F8/chaos/catalog.json`): stan + sygnały modułów
  i sonda zdrowia.
- `run_chaos` — runner F8-01/F8-06: wykrycie z poprawną klasyfikacją, kompletna karta propozycji, naprawa
  zweryfikowana sondą, cofnięcie do stanu 1:1, obszar Jądra wyłącznie przez Brokera.
