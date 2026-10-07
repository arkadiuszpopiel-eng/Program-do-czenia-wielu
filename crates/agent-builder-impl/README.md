# agent-builder-impl

Moduł Kreatora agentów: `AgentBuilderModule` = `AgentBuilder` + `Module` (`module.toml`: `on-demand`,
`inproc`, RAM ≤ 2 MB). Rdzeń `BuilderCore` z kontraktu; ten crate dodaje:

- katalog person i ról odświeżany z usługi `personas` przed każdą operacją (kolizje imion, form, glifów);
- sufit autonomii z sesji (`CeilingSource`; w aplikacji: `Broker::autonomy(sesja, None)`; zawsze ≤ L3);
- zapis: warunki rdzenia → `Personas::add_role` → `Personas::add_persona` → biblioteka manifestów
  (`DirManifestStore`: `<persona>.json`, zapis atomowy) → rdzeń → zdarzenia `agent_builder.*`;
- test na sucho i zapis wymagają uruchomionego modułu (zdarzenia w dzienniku).

Zależności produkcyjne: tylko `*-contract`. Testy: kontrakt (ścieżka szczęśliwa, 41 ataków = 0 sukcesów)
na `personas-fake`, sufit z sesji, trwałość, błąd usługi person = brak zapisu.
