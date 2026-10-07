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

**Część 2** (`service`): `BrokerService` — named pipe z ACL (`SecurePipePort`), wątek na połączenie (`BlockingIo`
nad `BrokerServer::serve_with`), rola klienta wiązana z tożsamością procesu (`RoleBindings`: konto, integralność,
obraz, podpis; jądro/watchdog bez MAC po tożsamości obrazu, Broker-UI zawsze z biletem i wysoką integralnością;
odrzucenia → `broker.ipc.rejected`), `open_audit` w katalogu prywatnym (`PrivateDirPort`), `UiSupervisor`
(bilet `UiLaunchTicket` przez stdin, restart z przerwą). Binarka `alfa-broker` — crate `app-safety`. Testy:
`tests/service.rs` na `platform-fake`.
