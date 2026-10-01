# evals/F5 — zestawy akceptacyjne fali 5 (scheduler, triggers, Marszałek)

Kryteria z `docs/ACCEPTANCE.md` §8. Zestawy są danymi (JSON); testy wczytują je przez `include_str!`,
więc zmiana zestawu zmienia test. **Status: propozycja autora modułów — do zamrożenia hashem
(`MANIFEST.json`) po akceptacji właściciela/modelu-recenzenta** (ACCEPTANCE §1: autor nie zatwierdza
własnego zestawu).

| Plik | Kryterium | Próg | Test |
|---|---|---|---|
| `scheduler-scenarios.json` | F5-03 — brak zakleszczeń | 0 w 1000 losowych scenariuszy (ziarno + generator) | `crates/scheduler-fake/tests/props.rs` |
| `parallel-scenarios.json` | F5-01 — agentki równolegle z blokadą ekranu/głośnika | 0 konfliktów w 100 scenariuszach | `crates/scheduler-fake/tests/parallel.rs` |
| `steering-cases.json` | F5-02 — steering w ≤ 1 kroku atomowym | 20/20 (tekst i głos; w toku, w kolejce, po pauzie, po wywłaszczeniu mową) | `crates/scheduler-fake/tests/steering.rs` |
| `bridge-trigger-cases.json` | F5-04 — „most nie startuje z wyzwalacza” | 0/100 (z jawną zgodą tras na harmonogram) | `crates/triggers-impl/tests/compliance.rs` |
| `marshal-rules.json` | F5-09 — reguły Marszałka tylko zawężają | 0 rozszerzających z 22 (50 reguł; 21 zawężających musi przejść) | `crates/marshal-fake/tests/narrowing.rs` |

Uruchomienie (deterministyczne, wirtualny zegar, bez sprzętu):

```
cargo test -p scheduler-fake -p triggers-impl -p marshal-fake -- --nocapture
```

Wyniki z 2026-10-01: F5-03 0/1000 (7079 zadań, 195 sterowań, 1149 przyznań mowy), F5-01 0 konfliktów
(95/100 scenariuszy z ≥ 2 agentkami naraz), F5-02 20/20, F5-04 0/100 (71 odrzuconych podzadań-mostów,
71 odmów mostu), F5-09 0/22 rozszerzających przyjętych, 0/7 niepoprawnych, 0/21 zawężających odrzuconych.
