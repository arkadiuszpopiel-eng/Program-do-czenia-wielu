# safety-broker-fake

Atrapa Brokera dla testów `agent-runtime`, `tools-*`, UI. Pod spodem prawdziwy `BrokerEngine`
(zależność od `-impl` tego samego modułu — dozwolona) z kluczami z jawnego ziarna (bez sekretu, tylko
testy), Audytem w pamięci i portem procesów, który niczego nie zabija. `script(tool, Allow|NeedsApproval|Deny)`
zmienia decyzje, ale twarde blokady Jądra działają zawsze. `auto_approve` udaje Broker-UI (syntetyczny
dowód z nonce wyzwania, jak `broker-ui-fake` ze SPEC). Rejestratory: `audit_names`, `killed_jobs`,
`silences` (cisza audio jako zdarzenie). Przechodzi współdzielony test kontraktowy.
