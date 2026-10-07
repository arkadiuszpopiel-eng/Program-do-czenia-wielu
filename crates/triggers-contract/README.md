# triggers-contract

Kontrakt wyzwalaczy F5 (docs/modules/triggers/SPEC.md): `TriggerSpec` (właściciel, rodzaj, strefa,
akcja → `TaskSpec`, sufit uprawnień `scope`, limit częstości, okno ciszy/DND, zaległe), `CronExpr`
(5 pól, nazwy EN/PL, makra, reguła Vixie), `Tz` (UTC, stałe przesunięcie, strefy UE z regułą DST —
bez tzdata), `TriggerEngine` (deterministyczny), `TriggersCore<H: TriggerHost>` + trait `Triggers`.
Reguła zgodności: zadania z pochodzeniem `Trigger` (most odmawia); wyjątek — harmonogram czasowy
użytkownika z `allow_bridges` i limitem dziennym (`Schedule`). Treść wyzwalająca: taint + osobne pole
`untrusted`. Feature `contract-tests`: wspólny zestaw (DST wiosna/jesień, interwał, zdarzenia, limity,
cisza, właściciel, mosty, dziennik).
