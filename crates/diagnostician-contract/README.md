# diagnostician-contract

Kontrakt Diagnosty (`docs/modules/diagnostician/SPEC.md`, PLAN §12.2): katalog awarii (`FailureKind`, 24 rodzaje),
sygnały (`Signal` — stan modułów z `core-registry`, symptomy z dziennika Diagnostyka, akcje watchdoga, zasoby;
`Signal::from_event` dla zdarzeń magistrali), klasyfikator (`classify`: okno czasu, progi powtórzeń, przyczyna
szczegółowa wygrywa z ogólną), planista (`plan` → `Proposal`: diff, uzasadnienie, ryzyko, plan cofnięcia, kroki
`RepairStep` z operacją odwrotną, obszar Jądra → Broker), polityka autonomii, dziennik napraw append-only i rdzeń
`DiagnosticianCore` (wykonanie z pokwitowaniami, weryfikacja, cofnięcie przy porażce i na żądanie, odzysk po
restarcie, limity i wychładzanie) oraz raport `HealthReport` dla panelu „Zdrowie systemu”.
