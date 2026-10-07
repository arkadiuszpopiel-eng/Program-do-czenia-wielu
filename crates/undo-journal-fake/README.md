# undo-journal-fake

Atrapa dziennika cofania: ten sam rdzeń w pamięci nad dowolnym `FsPort` (np. `platform-fake`),
`FlakyFs` ze sterowanymi awariami operacji i przywracania (`arm(n)` — chaos: przerwanie w trakcie
cofania → raport częściowy, pre-image nietknięte) oraz awaria zapisu dziennika (`store().set_fail_append`).
Przechodzi współdzielony test kontraktowy.
