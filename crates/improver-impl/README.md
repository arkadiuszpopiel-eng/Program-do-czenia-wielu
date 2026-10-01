# improver-impl

Usługa Ulepszacza (`docs/modules/improver/SPEC.md`): `ImproverService` = rdzeń `ImproverCore` (z kontraktu) +
`ServiceHost` (zegar, zdarzenia `improver.*` wysyłane na magistralę przez kanał po `start`, trwała kolejka propozycji
w `proposals.json` z zapisem atomowym i odtworzeniem po restarcie) + `cycle()` dla harmonogramu bezczynności
(obserwacja → ocena propozycji → nadzór wdrożeń) + moduł rejestru (`module.toml`, brak zdolności — jedyny zapis to
`core-config` z `Origin::Improver`). Testy: kontraktowe, F8-02 (≥ 100 prób ataku, 0 zapisów), F8-04 (R0 zawężające,
cofalne), integracja z prawdziwą bramką holdoutu (`evals-fake`).
