# example-module-impl

Implementacja modułu-wzorca „echo”. `EchoModule` implementuje `Echo` (z `example-module-contract`)
oraz `Module` (z `core-registry-contract`): `manifest()` czyta `module.toml` z tego crate'a
(walidowany testem), `start(ctx)` zapamiętuje magistralę, `stop()` ją zwalnia, `health()` raportuje stan.
Każde udane `echo` publikuje zdarzenie `example.echo.called` na magistralę; przed `start` echo zwraca
`EchoError::NotStarted`. Zależności produkcyjne: wyłącznie crate'y `*-contract`; w testach używa
`core-bus-fake` (dozwolone tylko jako dev-dependency). Testy: współdzielony test kontraktowy,
cykl życia modułu i sprawdzenie, że zdarzenia trafiają na magistralę.
