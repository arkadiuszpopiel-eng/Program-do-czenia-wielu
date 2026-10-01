# safety-broker-impl

Broker — część 1 (logika). `BrokerEngine` implementuje `Broker`, `ApprovalChannel`, `KillSwitch`,
`JobRegistry`: tokeny HMAC-SHA256 (klucz tylko w pamięci, `getrandom`, rotacja z oknem łaski,
kasowanie przy kill-switchu; nowy klucz i `boot` przy każdym starcie), decyzje L0–L4 z klasyfikatorem
i regułami Jądra (przy wydaniu i przy każdym użyciu), plan do zatwierdzenia, „zawsze zezwalaj”
(tylko reguły zależne od poziomu, z limitem czasu), prośby z jednorazowym nonce i dowodem fizycznego
wejścia, zmiany poziomu (agentka → `SelfEscalation`) i polityk. Każda decyzja w Audycie — fail-closed.
`audit`: `BrokerAuditWriter` (NDJSON, łańcuch SHA-256 po kanonicznym JSON, nowy łańcuch z głową
`pre-broker`, kotwica po każdym zapisie w `AnchorStore`: `FileAnchorStore`/`MemoryAnchorStore`, redakcja
sekretów). `ipc`: serwer/klient na dowolnym strumieniu (`tokio::io::duplex` w testach); brak gniazd
sieciowych (test skanuje źródła). Named pipe z ACL na SID, usługa Windows na osobnym koncie i Broker-UI —
część 2. Testy: kontrakt, Audyt 10 000 zdarzeń + manipulacje, 116 scenariuszy negatywnych (0 sukcesów),
IPC, kill-switch 50 prób (budżet ściśle przy `ALFA_PERF_BUDGETS=1`).
