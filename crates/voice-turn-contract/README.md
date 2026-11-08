# voice-turn-contract

Kontrakt polityki końca tury (PLAN §6.2/§6.4, SPEC `docs/modules/voice-turn`): zdarzenia `TurnEvent`
(VAD start/koniec mowy, transkrypt częściowy, reset), konfiguracja cierpliwości (`Patience`
Low/Normal/High/Custom, `TurnCfg` z walidacją), decyzja `TurnDecision` (`Idle` / `Wait { until_ms }` /
`EndOfTurn`), trait `TurnModel` (prawdopodobieństwo końca tury — Smart Turn v3.2 ONNX w przyszłości),
trait `TurnDetector` (stanowy, deterministyczny, czas podaje wywołujący), zdarzenia `voice.turn.*` i —
pod feature `contract-tests` — `contract_tests::run_all` (granice ciszy, jeden koniec na turę, wznowienie mowy).
