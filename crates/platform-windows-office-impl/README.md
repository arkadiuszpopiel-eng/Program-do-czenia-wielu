# platform-windows-office-impl

Porty aplikacji F6 (`platform-apps-contract`) dla Windows; SPEC: `docs/modules/platform-apps/SPEC.md`.

| Port | Implementacja |
|---|---|
| `OfficePort` (`office/`) | COM late binding `IDispatch` na wątku STA z `recv_timeout` (porzucanie wiszących, limit), kopia robocza w `work_dir`, `AutomationSecurity = 3` ustawione i odczytane przed otwarciem, Protected View (`ProtectedViewWindows.Open`) dla plików z MOTW, hasła-atrapy przy otwarciu, Excel: tryb obliczeń ręczny, bez zdarzeń i aktualizacji łączy; instancja Excela użytkownika odrzucana. |
| `BrowserPort` (`browser/`) | Edge/Chrome z osobnym profilem (`Default/Preferences` bez haseł/autouzupełniania), CDP przez potok (CRT fd 3/4 przez `lpReserved2` + `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`), Job Object, `Fetch.requestPaused` → filtr egressu, WebSockety zablokowane, cele potomne wstrzymane do `Fetch.enable` (błąd → zamknięcie celu), pobrania `allowAndName` do kwarantanny. |
| `RegistryPort` (`registry.rs`) | `RegOpenKeyExW(KEY_READ)`, wyliczanie z limitami; deny-lista kontraktu przed otwarciem, redakcja wartości. |

Testy przenośne: dekodowanie rejestru, kopia robocza, pełna sesja CDP na skryptowanej przeglądarce
(`browser/tests.rs`). Office/Edge na żywo — wyłącznie self-hosted Windows (testy `#[ignore]` w `tests/`).
