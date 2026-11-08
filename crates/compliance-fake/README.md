# compliance-fake

Atrapa modułu zgodności do testów innych modułów (`router`, `accounts-hub`, `agent-backends`).
`FakeCompliance` implementuje `Compliance` na rejestrze w pamięci, z datą „dziś” sterowaną
(`set_today`, `advance_days`), statusami tras sterowanymi (`set_status`) i rejestrem zapytań
`route_allowed` (`queries`). Decyzje, degradację i deny-listy liczy ta sama czysta logika
z `compliance-contract` (`RouteTable`, `DenyChecker`), więc atrapa przechodzi identyczny zestaw
testów kontraktowych co `compliance-impl`. Bez magistrali i bez plików. Profil użytkownika
w deny-listach: `C:\Users\Test`. Tylko jako `dev-dependency` innych modułów.
