# voice-turn-fake

Atrapy `voice-turn` do deterministycznych testów potoku i `voice-dialog`: `ScriptedTurnModel`
(prawdopodobieństwa lub błędy z kolejki, potem wartość stała; rejestruje transkrypty wywołań) oraz
`ScriptedTurnDetector` (koniec tury po adnotowanej ciszy `push_delay`, przyciętej do granic cierpliwości;
bez adnotacji — `base_ms`). Przechodzi ten sam test kontraktowy co `voice-turn-impl`.
Zależy wyłącznie od `voice-turn-contract`.
