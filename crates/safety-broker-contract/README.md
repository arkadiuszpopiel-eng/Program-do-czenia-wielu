# safety-broker-contract

Kontrakt Brokera — Safety Kernel (docs/modules/safety-broker/SPEC.md, PLAN §8, ADR 3, ADR 15).
- **Zdolności** `fs.read/fs.write/shell.exec(PathScope)`, `gui.control(AppSelector)`, `net.egress(HostPattern)`,
  `secrets.read(SecretId)`, `system.admin(AdminOp)`; atenuacja „potomek ⊆ rodzic” tylko w obrębie rodziny,
  ścieżki normalizowane jak w `compliance-contract` (zmienne, `\\?\`, `..`, ADS, wielkość liter, udziały `c$`).
- **Token** `CapToken` z kanonicznym formatem przewodowym (`ALFT` v1, MAC 32 B na końcu, ścisły parser
  z ponownym kodowaniem) — zmiana dowolnego bajtu = błąd parsowania albo niezgodny MAC.
- **Poziomy autonomii** `AutonomyTable` (SessionAgent → min(Session, Agent) → Global → L3, z terminem).
- **`PhysicalInputProof`**: prywatne pola, brak `Clone/Default/Deserialize`, konstruktor ukryty
  w `broker_ui_only`; doctesty `compile_fail` (E0451, E0616, E0277, E0308).
- **Strażnik Jądra** `KernelGuard` + leksykalny `check_command` (format, bootloader, audyt, usługi Jądra,
  poświadczenia, ścieżki Jądra, `%SystemRoot%`, polecenia zakodowane) i `derive_facts` dla klasyfikatora.
- **Traity** `Broker` (agentki/jądro), `ApprovalChannel` (wyłącznie Broker-UI), `AnchorStore`, zdarzenia Audytu,
  **protokół IPC** (`ipc`: role, poświadczenia, ramki z limitem 1 MiB, uprawnienia żądań per rola).
Testy: zakresy i guard (jednostkowe/negatywne, 43 polecenia blokowane + 15 dozwolonych), 6 własności
po 3000 przypadków, współdzielone testy kontraktowe (`contract-tests`).
`ipc_blocking`: klient blokujący (`BlockingClient` nad `Read + Write`, ramki jak w `ipc`) dla Broker-UI
i watchdoga oraz bilet startowy Broker-UI `UiLaunchTicket` (poświadczenie, potok, SID konta usługi).
