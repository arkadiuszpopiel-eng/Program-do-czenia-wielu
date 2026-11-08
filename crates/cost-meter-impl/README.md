# cost-meter-impl

Licznik kosztów. `CostMeterService` implementuje `CostMeter` i `Module`:

- **Trwałość: NDJSON** (`NdjsonLedger`, jeden rekord na linię, tylko dopisywanie). Wybór zamiast
  SQLite: koszty nie są tajne (bez SQLCipher), format append-only zgodny ze strumieniem ModelCalls,
  czytelny, bez budowania SQLite/OpenSSL; agregaty i tak są w pamięci i odtwarzane z rekordów przy
  starcie. Urwana ostatnia linia (awaria) jest pomijana i domykana przed kolejnym zapisem.
- Kurs: `NbpFxSource<H: HttpGet>` (port HTTP dostarcza kompozycja; egress przez Brokera), limit 10 s;
  `refresh_fx` raz dziennie (gospodarz wywołuje po starcie i codziennie — start nie dotyka sieci);
  brak sieci → poprzedni kurs albo zapasowy + `cost.fx.stale`, nigdy blokada. Kurs odtwarzany
  z ostatniego rekordu po restarcie.
- Limity i alerty: `check_budget` (+ `cost.limit.blocked`), progi przy `record` (`cost.limit.warning`,
  każdy raz w miesiącu i zakresie), `set_budget` tylko użytkownik/Broker.
- `SystemClock`: UTC + data lokalna (granice dni/miesięcy lokalnie).
