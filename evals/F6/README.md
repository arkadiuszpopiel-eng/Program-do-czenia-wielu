# evals/F6 — computer use (ACCEPTANCE F6)

**Status: szkic — wymaga akceptacji człowieka, potem zamrożenie hashem.** Zestawy przygotowała model-recenzentka
(fala 4); runnera F6 i maszyny wirtualnej jeszcze nie ma (bramki #1 „mózg”, #8 VM).

| Kryterium                                | Gdzie                                                          | Stan                                                                                 |
| ---------------------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| F6-01 zadania w ≥ 5 kategoriach, ≥ 85 %  | `tasks/` (60 zadań, 5 kategorii po 12, schemat, strony `web/`) | szkic; runner i VM do zbudowania (`tasks/README.md`)                                 |
| F6-02 benchmark zewnętrzny               | —                                                              | poza tym zestawem (OSWorld / Windows Agent Arena — dostępność do weryfikacji)        |
| F6-03 macierz aplikacja × trasa          | `app-matrix.md` (45 pozycji, arkusz pomiaru spike'u c)         | prognoza; pomiar w spike'u (c)                                                       |
| F6-04 weryfikacja po akcji GUI           | niezmiennik runnera F6 (`tasks/README.md`, „Przebieg”)         | ✅ na atrapach (testy `tools-*-impl`, `docs/STATUS.md`); w VM — przy przebiegu F6-01 |
| F6-05 deny-listy (100 scenariuszy)       | częściowo: zadania `guard` (`pl-12`, `pr-11`, `pr-12`)         | zestaw 100 scenariuszy nadal do przygotowania                                        |
| F6-06 `gui.control` wobec Alfy (50 prób) | częściowo: `ap-12`, `us-12`                                    | jw. (50 prób na desktopie)                                                           |
