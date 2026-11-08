# scheduler-lite-fake

Atrapa `scheduler-lite` do testów innych modułów (tylko dev-dependency): ten sam rdzeń decyzyjny co
`-impl`, ale **wirtualny zegar** (`advance(ms)` obsługuje po kolei każdy termin po drodze — timeouty
i wygasanie rezerwacji), nagrane zdarzenia (`events()`), zapis żądań (`requests()`), jednorazowy błąd
(`fail_next`), polityki (`set_policy`) i migawka tablicy (`snapshot()`). Przyznania wolnych zasobów są
natychmiastowe. Przechodzi ten sam test kontraktowy co `-impl`; wywłaszczenie przez mowę użytkownika
zapada w 0 ms wirtualnego czasu (ACC-F2-scheduler-lite-02).
