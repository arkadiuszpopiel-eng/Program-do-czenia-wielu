# voice-turn-impl

Polityka końca tury (`PatienceTurnDetector`) i model tekstowy (`HeuristicTurnModel`).
Wymagana cisza: bazowa (`base_ms`), skracana do minimalnej przy pewnym modelu (≥ 0,85) lub — bez modelu —
przy interpunkcji końcowej; wydłużana przy niepewnym modelu (< 0,5, `low_prob_bonus_ms`) i po hezytacji
(„yyy”, „eee”, „znaczy”, „hmm”, niedokończone „i”, „że”, przyimek, przecinek, wielokropek —
`hesitation_bonus_ms`); zawsze w granicach `[min_silence_ms, max_ms]` (twardy limit — brak zawieszenia).
Wynik modelu jest liczony raz na (koniec mowy, wersja transkryptu); błąd modelu → sama polityka tekstowa.
`HeuristicTurnModel` zastępuje Smart Turn v3.2 do czasu impl ONNX (model ~8 MB, hash, CPU).

Testy: kontrakt współdzielony (z modelem heurystycznym i skryptowym), scenariusze z wirtualnym zegarem
(pytanie → 200 ms, hezytacja → 1200 ms, twardy limit, poziomy cierpliwości, błąd modelu), property-based
(nigdy koniec w trakcie mowy, zawsze w granicach, najwyżej raz na turę).
