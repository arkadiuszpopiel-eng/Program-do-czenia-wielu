# personas-fake

Atrapa `personas` do testów innych modułów (tylko dev-dependency): katalog z fixture'u
(`with_catalog`), `preset_cast` (bez zdarzeń), nagrane zdarzenia (`events()` — dokładnie te, które
`-impl` opublikowałby), zapis wywołań zmiany obsady (`set_cast_calls()` z informacją, czy przyjęta),
jednorazowy błąd (`fail_next`). Ta sama logika co `-impl` (`PersonasState`), więc przechodzi ten sam
test kontraktowy. Zależy wyłącznie od `personas-contract` i `core-bus-contract`.
