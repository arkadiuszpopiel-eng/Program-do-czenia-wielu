# ADR 0018 — Fabryka platform (`platform-select`) i kontrakty Brokera niezależne od systemu

| Pole | Wartość |
|---|---|
| Status | Proponowany — czeka na podpis właściciela |
| Data | 2026-10-09 |
| Powiązane | plan v6: `docs/plan/02-architektura-docelowa.md` (szew 1, szew 2, mapa zdolności), `03-roadmapa.md` (B.4, B.7, B.8, B.9, G.2, G.3); ADR 0002, 0003, 0015, 0017; `docs/reviews/2026-10-08-tools-shell-clipboard-net-review.md` (S-01…S-04, S-06) |
| Zmienia | kontrakty Jądra bezpieczeństwa (`safety-broker-contract`, `compliance-contract`, `tools-shell-contract`, część `platform-contract`) — **wymaga przeglądu i akceptacji człowieka** (bramka #7) |

## Kontekst

Porty systemowe są zdefiniowane czysto (ok. 30 w `platform-contract`, poza nim m.in. `AudioIo`, `SecretStore`,
`KeyVault`, `ShellPort`), a `AppOptions` przyjmuje każdy z nich jako `Option<Arc<dyn …>>`. Mimo to korzeń
kompozycji wpisuje typy Windows wprost w ok. 10 miejscach (`app-core/src/parts/*`, `app-agents/src/sysnet.rs`,
`app-gui`, `app-terminal`, `app-broker`, `app-safety`). Drugi kłopot jest głębszy: w kontraktach Brokera
semantyka Windows jest wpisana w typy, a nie odgrodzona `cfg` — ścieżki, listy zakazów, analizator powłoki
i tożsamość procesu. To najtrudniejszy szew planu v6, bo dotyka Jądra.

## Decyzja

Część 1 — fabryka platform (zadanie B.4):

| Element | Propozycja |
|---|---|
| Moduł | `platform-select` — biblioteka kompozycji z funkcją `platform() -> PlatformPorts`; wybór implementacji przez `cfg(target_os)` w tym jednym miejscu |
| `PlatformPorts` | struktura z portami; porty opcjonalne jako `Option<Arc<dyn …>>` (brak `audio` = głos niedostępny, brak `uia` = sterowanie komputerem niedostępne) |
| `HostCapabilities` | mapa zdolności hosta wyliczana z dostępnych portów; trafia do klientów, które pokazują tylko to, co host potrafi |
| Korzeń kompozycji | zna tylko `PlatformPorts`; kryterium wyjścia etapu B: brak nazw `platform_windows_*` w korzeniu kompozycji |
| Port nieobecny | zwraca `Unsupported` — nigdy pusty sukces |

Część 2 — kontrakty Brokera niezależne od systemu (zadanie B.7):

| Miejsce | Dziś | Propozycja |
|---|---|---|
| `safety-broker-contract/src/scope.rs` | ścieżki sprowadzane do małych liter z `\` | `PathScope` parametryzowany `PathFlavor` (Windows / POSIX): wielkość liter, separator, korzenie |
| `compliance-contract/src/deny/path.rs` | listy zakazów rozwijają `%ZMIENNE%` profilu Windows | `PathEnv` per system; listy zakazów jako dane per system |
| `safety-broker-contract/src/shell_guard.rs`, `tools-shell-contract` | reguły tylko dla `cmd` i PowerShell; `ShellKind = Pwsh / Powershell / Cmd` | analizator jako port z implementacją per powłoka; dochodzi `Posix` (bash/sh/zsh) |
| `safety-broker-contract/src/ipc.rs` (`Hello`) | tożsamość procesu = SID + ścieżka obrazu | tożsamość abstrakcyjna: Windows — SID; Linux — UID + `SO_PEERCRED` |
| `platform-contract` (`PROTECTED_IMAGES`, klasy UWP) | stałe Windows eksportowane z kontraktu | dane platformy dostarczane przez implementację |

Warunki wejścia dla części 2:

1. **Najpierw rozstrzygnięcie otwartych ustaleń analizatora powłoki S-01…S-04 i S-06** (zadanie A.7, decyzja
   właściciela) — żeby nie przenosić znanych dziur do nowej struktury.
2. Zmiana przechodzi **przegląd człowieka** jak każda zmiana Jądra; sesja AI przygotowuje pakiet do przeglądu,
   nie zatwierdza (ADR 0016).
3. Zachowanie na Windows nie może się zmienić: te same testy kontraktowe, property-based i zestawy `evals/`
   muszą przejść przed przebudową i po niej.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| `cfg(target_os)` bezpośrednio w korzeniu kompozycji i w modułach | rozsiewa wiedzę o systemie; nie da się testować kompozycji na atrapach |
| Wybór platformy w czasie działania (rejestr wtyczek) | system jest znany przy kompilacji; dodatkowa pośredniość bez korzyści |
| Osobne kontrakty Brokera dla każdego systemu | dwie kopie Jądra do przeglądu przez jedną osobę; reguły rozjechałyby się |
| Semantyka POSIX „doklejona” warunkami w obecnych typach | niejawne założenia (małe litery, `\`) dalej siedziałyby w kontrakcie; błąd normalizacji ścieżki to błąd bezpieczeństwa |
| Przebudowa kontraktów przed rozstrzygnięciem S-01…S-04, S-06 | znane dziury analizatora przeszłyby do nowej struktury i na nową powłokę |
| Jeden wspólny analizator dla wszystkich powłok | składnie cytowania i ucieczki są różne; wspólny parser byłby najsłabszym wspólnym mianownikiem |

## Konsekwencje

- Część 1 nie dodaje funkcji i nie dotyka Jądra; może iść równolegle z etapem A. Część 2 czeka na A.7.
- Analizator POSIX (G.2) i tożsamość przez gniazdo Unix z kontrolą UID (G.3) to osobne zadania Jądra z własnym
  przeglądem właściciela; ten ADR przygotowuje dla nich miejsce w kontrakcie, nie przesądza implementacji.
- Okno zgód i dowód fizycznego wejścia na Linuksie są w roadmapie oznaczone jako badawcze (G.4) — ten ADR ich
  nie rozstrzyga.
- Listy zakazów jako dane per system oznaczają nowy materiał do przeglądu bezpieczeństwa dla każdej platformy.
- Drobna poprawka przy okazji: `crates/app-bridges/src/app.rs:227` wybiera `cmd` dla systemów innych niż
  Windows (B.9).
- Kryterium wyjścia etapu B: `cargo test --workspace` zielone na Windows i Ubuntu.

## Jak cofnąć

- Część 1: `platform-select` to cienka warstwa — cofnięcie oznacza powrót do typów wpisanych w korzeń
  kompozycji, bez wpływu na moduły.
- Część 2: parametr `PathFlavor::Windows` i analizatory `cmd`/PowerShell zachowują dzisiejsze zachowanie; jeśli
  przegląd odrzuci przebudowę, kontrakty zostają w obecnej postaci, a Linux nie dostaje powłoki ani Jądra poza
  procesem (zadania G.2, G.3 wstrzymane).
- Do podpisu tego ADR kontrakty Jądra pozostają bez zmian.
