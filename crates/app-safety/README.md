# app-safety

Korzeń kompozycji (`app-*`, check-deps) procesów Jądra bezpieczeństwa — F3 część 2 (ADR 3, PLAN §8.1–8.2, §8.6).
Logika usług żyje w `safety-broker-impl::service`, `broker-ui-impl`, `watchdog-impl::daemon` (testowalna
na `platform-fake`); tu tylko złożenie z `platform-windows-impl` i trzy binarki (binarki nie mogą być w
`-impl`, bo zależność `-impl` → cudzy `-impl` łamie zasadę trójki).

| Binarka | Uruchamia | Co robi |
|---|---|---|
| `alfa-broker` | menedżer usług (`AlfaBroker`, `--config <plik.json>`) albo `--console` (dev) | Audyt w katalogu z chronionym DACL (kotwica obok), silnik Brokera, serwer IPC na `\\.\pipe\alfa-broker` (DACL: konto usługi + konto użytkownika, `PIPE_REJECT_REMOTE_CLIENTS`, pierwsza instancja, etykieta ME), rola klienta wiązana z tożsamością procesu (SID, integralność, obraz, podpis — dev: „niezweryfikowane”), nadzór Broker-UI |
| `alfa-broker-ui` | usługa Brokera (bilet startowy na stdin) | sprawdza konto serwera potoku, natywne okno zatwierdzeń (`WinApprovalSurface`) z dowodem fizycznego wejścia |
| `alfa-watchdog` | launcher (`alfa-watchdog [--broker-pipe P] [--broker-user SID] [-- <jądro> …]`) | `Ctrl+Shift+F12` → zabija drzewa procesów (Job Objects; jądro w jego Job Object), Broker `KillAll` przez potok z limitem 100 ms |

Instalacja usługi (bramka ludzka #10, jednorazowy UAC):
`sc.exe create AlfaBroker binPath= "\"C:\Program Files\Alfa\alfa-broker.exe\" --config C:\ProgramData\Alfa\broker\broker.json" start= auto obj= LocalSystem`
(osobne konto z `SeTcbPrivilege` zamiast LocalSystem — decyzja przy instalacji). Plik `broker.json` = `ServiceConfig`
(SID konta usługi i użytkownika, katalog danych, wiązania ról z pełnymi ścieżkami obrazów w `Program Files`,
`broker_ui.integrity = "user_session_high"`). Testy: `tests/chain.rs` (pełny łańcuch na atrapach, każdy system),
`tests/windows_ports.rs` (porty Windows na CI; wysoka integralność — `#[ignore]`, self-hosted).
