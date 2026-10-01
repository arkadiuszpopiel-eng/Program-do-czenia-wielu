# evals-fake

Atrapa harnessu ewaluacji (tylko `dev-dependencies` innych modułów). `FakeCatalog` trzyma zestawy w pamięci
(manifest + bajty plików) i stosuje te same reguły co `evals-impl`: hashe SHA-256, zamrożony zestaw ze zmienionym
plikiem → błąd, holdout zapieczętowany. `FakeGate` ocenia warianty na holdoucie w pamięci przez `CandidateRunner`
(np. `QualityRunner` z testów kontraktowych), z budżetem zapytań i wirtualnym zegarem; rejestruje żądania.
Przechodzi współdzielone testy kontraktowe (`tests/contract.rs`).
