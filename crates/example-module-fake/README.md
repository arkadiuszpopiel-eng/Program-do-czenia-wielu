# example-module-fake

Atrapa modułu-wzorca „echo” do testów innych modułów: `FakeEcho` implementuje `Echo` bez magistrali,
rejestruje wszystkie wywołania (`calls()`), pozwala wstrzyknąć błąd (`fail_next`) do testów ścieżek
błędów i jest w pełni deterministyczna (numery kolejne od 1). Używa tej samej walidacji wejścia
co `-impl` (`validate_input` z kontraktu), więc przechodzi identyczny test kontraktowy —
to jest dowód, że atrapa nie kłamie. Zależy wyłącznie od `example-module-contract`.
