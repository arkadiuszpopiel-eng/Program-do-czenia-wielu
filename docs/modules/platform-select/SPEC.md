# platform-select — SPEC (szkic v0, planowany — plan v6)

## Cel
Jedno miejsce, w którym korzeń kompozycji dostaje implementacje portów systemowych dla bieżącego systemu
(`platform() -> PlatformPorts`) oraz mapę zdolności hosta (`HostCapabilities`), którą dostają klienty.
Moduł **nie** zawiera kodu systemowego, nie definiuje nowych portów i nie zmienia kontraktów Jądra — tylko
wybiera i składa istniejące implementacje. Stan: propozycja, crate nie istnieje.

## Fala i priorytet
Plan v6, **etap B** (zadanie B.4), po podpisie ADR 0018. Priorytet: do ustalenia przez właściciela.
v0: wybór dla Windows (stan obecny przeniesiony z korzenia kompozycji) + mapa zdolności.
v1: gałęzie Linux (etap G) i Android (etap F), gdy powstaną ich implementacje.

## Kontrakt (szkic Rust)
```rust
// szkic — typy portów pozostają w platform-contract; źródłem prawdy będzie kod
pub struct PlatformPorts {
    pub fs: Arc<dyn FsPort>,
    pub exec: Arc<dyn ExecPort>,
    pub secrets: Arc<dyn SecretStore>,
    pub audio: Option<Arc<dyn AudioIo>>,   // brak = głos niedostępny na tej platformie
    pub uia: Option<Arc<dyn UiaPort>>,     // brak = sterowanie komputerem niedostępne
    /* … pozostałe porty … */
    pub capabilities: HostCapabilities,    // co ten host potrafi — trafia do klientów
}
pub struct HostCapabilities { /* chat_cloud, chat_local, voice, tools_fs, tools_net, tools_shell,
                                 computer_use, office, registry, memory … */ }
pub fn platform() -> PlatformPorts;        // wybór przez cfg(target_os)
```
Zdarzenia na magistrali: brak (mapa zdolności jest stanem odczytywanym poleceniem z `app-api`).

## Zależności
`platform-contract` oraz kontrakty, w których żyją pozostałe porty (`AudioIo`, `SecretStore`, `KeyVault`,
`ShellPort`). Jako biblioteka kompozycji — jedyne miejsce poza `app-*`, które zależy od crate'ów
`platform-<system>-*-impl` (wyjątek od reguły „nigdy od cudzego `-impl`” do zapisania w `check-deps.sh`).

## Niezmienniki
- Poza tym modułem korzeń kompozycji nie zawiera nazw `platform_windows_*` (kryterium wyjścia etapu B).
- Port, którego system nie ma, zwraca `Unsupported` — nigdy pusty sukces.
- `HostCapabilities` wynika z faktycznie dostępnych portów; zdolność nie może być zgłoszona bez portu.
- Zachowanie na Windows po wprowadzeniu fabryki jest takie samo jak przed (te same testy zielone).
- Żadnego `cfg(target_os)` wybierającego implementację portu poza tym modułem.

## Zdolności / uprawnienia
Brak — moduł nie wykonuje działań; porty nadal są używane wyłącznie przez moduły pod kontrolą Brokera.

## Izolacja
`inproc`, wywołanie jednorazowe przy starcie hosta (`always`); bez wątków własnych.

## Budżet zasobów
Pomijalny (jednorazowe złożenie struktur przy starcie). Wpływ na czas startu — do pomiaru; próg ustala właściciel.

## Konfiguracja (klucze TOML)
Brak kluczy własnych w v0. Do ustalenia w SPEC v1: czy zdolność można wyłączyć konfiguracją maszyny
(np. host bez głosu mimo dostępnego portu).

## Wkład do UI
Pośredni: klienty ukrywają funkcje, których nie ma w `HostCapabilities` (ukryte, nie zepsute). Miejsce
pokazania mapy zdolności w Ustawieniach — do ustalenia.

## Testy akceptacyjne
- `ACC-V6-platform-select-01` — korzeń kompozycji (`app-*`) nie zawiera odwołań do `platform_windows_*` (test statyczny).
- `ACC-V6-platform-select-02` — `cargo test --workspace` zielone na Windows i Ubuntu po wprowadzeniu fabryki.
- `ACC-V6-platform-select-03` — dla każdego portu `None` odpowiadająca zdolność w `HostCapabilities` jest wyłączona
  (test własności na atrapach).
- `ACC-V6-platform-select-04` — interfejs z atrapą hosta bez głosu i bez UIA nie pokazuje zależnych funkcji.
- `ACC-V6-platform-select-05` — czas startu hosta nie rośnie ponad próg (próg ustala właściciel).

## Fake
`platform-fake` już istnieje; dochodzi konstruktor `PlatformPorts` z atrap z dowolnie wyłączonymi portami
(do testów mapy zdolności i interfejsu). Osobny crate `platform-select-fake` — do ustalenia, czy potrzebny.

## Otwarte pytania
- ADR 0018 — podpis właściciela; część o kontraktach Brokera (ścieżki, powłoki, tożsamość) jest osobnym
  zadaniem B.7 i nie należy do tego modułu.
- ADR 0017 — mapa zdolności: lista pól i ich nazwy do uzgodnienia z `app-api` i trzema klientami.
- Plan określa moduł jako „bibliotekę kompozycji”, a zasada 2 planu v6 wymaga trójki crate'ów bez wyjątków —
  do rozstrzygnięcia, czy powstają `-contract` / `-impl` / `-fake`, czy jeden crate `lib-*` / `app-*`.
- Gdzie żyje typ `HostCapabilities`: `platform-contract` czy `app-api` (musi mieć generowany typ TS — ADR 0019).
