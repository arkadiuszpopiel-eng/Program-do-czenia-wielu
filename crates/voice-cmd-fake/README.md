# voice-cmd-fake

Atrapa `voice-cmd` do testów `voice-dialog` i UI: `FakeRecognizer` zwraca decyzje z adnotacji
(`script(tekst, decyzja)`), a bez adnotacji dopasowuje wyłącznie dokładne frazy gramatyki (rozwinięte
alternatywy, słowa opcjonalne i formy imion; bez tolerancji literówek), z tą samą regułą „nie”,
adresowaniem i progiem `settle_ms` co `voice-cmd-impl` (wspólne funkcje z kontraktu). Rejestruje wejścia
(`calls()`), przechodzi ten sam test kontraktowy. Zależy wyłącznie od `voice-cmd-contract`.
