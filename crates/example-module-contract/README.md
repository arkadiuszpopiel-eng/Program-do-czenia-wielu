# example-module-contract

Minimalny wzorzec modułu „echo” — pierwsza z trójki crate'ów (docs/PLAN.md §3.2, §4.5a pkt 2).
Kontrakt zawiera: trait `Echo` (`echo(input) -> EchoReply`), typy (`EchoReply`, `EchoError`),
stałą rodzaju zdarzenia `EVENT_ECHO_CALLED` (`example.echo.called`) publikowanego na magistralę
oraz — pod feature `contract-tests` — funkcję `contract_tests::run_all`, którą testy `-impl` i `-fake`
uruchamiają na swojej instancji. Rozjazd zachowań między implementacją a atrapą jest błędem testu.
Inne moduły zależą wyłącznie od tego crate'a (skrypt `scripts/check-deps.sh` to egzekwuje).
