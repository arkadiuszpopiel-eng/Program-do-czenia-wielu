# broker-ui-fake

Atrapa `BrokerUi` (docs/modules/broker-ui/SPEC.md, sekcja „Fake”) — **wyłącznie testy**. `ScriptedBrokerUi`
przyjmuje wyzwania jak prawdziwe okno (kolejka, duplikaty, wycofanie, wygasłe odrzucane) i rozstrzyga
kartę na czele kolejki według skryptu: `Allow`, `AllowInScope { hours }` (≤ 24 h), `Deny`, `Injected`
(symulowana próba SendInput — zdarzenie `broker_ui.injection_rejected`, brak decyzji) albo `Ignore`.
Dowód jest syntetyczny (`broker_ui_only`) z nonce wyzwania, więc prawdziwy Broker go przyjmie — w produkcji
dowód powstaje wyłącznie w `broker-ui-impl` po regułach fizycznego wejścia.
