# voice-cmd-contract

Kontrakt szybkiej ścieżki komend głosowych bez LLM (PLAN §6.2, §6.5; SPEC `docs/modules/voice-cmd`).
Zawiera: `VoiceCommand` (stop, czekaj, pauza, wznów, powtórz, anuluj, głośniej/ciszej, wycisz mikrofon,
przełącz na personę, nie przeszkadzać, stop wszystko = kill-switch, samodzielne „nie”), `AgentActivity`
(stan agentki z `voice-dialog`, bez cyklu zależności), wejście `CmdInput` (tokeny ze znacznikami czasu,
źródło, czas, adresowanie), wynik `CmdDecision` (Hit / Pending / Ignored / NoMatch), edytowalną gramatykę
`Grammar` (frazy PL/EN z alternatywami, słowami opcjonalnymi i slotem `{persona}`, wypełniacze, formy imion,
reguła „nie”, próg, `settle_ms`), wspólne reguły `fold`, `split_tokens`, `nie_verdict`, trait
`CommandRecognizer`, zdarzenia `voice.cmd.*` i — pod feature `contract-tests` — `contract_tests::run_all`.
