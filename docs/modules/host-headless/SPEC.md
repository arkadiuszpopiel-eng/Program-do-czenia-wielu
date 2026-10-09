# host-headless — SPEC (szkic v0, planowany — plan v6)

## Cel
Binarka `alfa-host`: uruchamia hosta (`AppCore`, moduły, połączenie z Brokerem) bez okna, tak żeby Alfa była
dostępna dla klientów zdalnych, gdy okno jest zamknięte. Okno Tauri staje się wtedy klientem lokalnym hosta.
Moduł **nie** zawiera logiki bramy (`remote-gateway`), nie zastępuje Brokera ani watchdoga i niczego nie
zatwierdza. Stan: propozycja, binarka nie istnieje.

## Fala i priorytet
Plan v6, **etap D** (zadanie D.7) — **drugi krok**: najpierw brama działa w procesie aplikacji (D.1), dopiero
potem wydzielenie hosta. Po podpisie ADR 0020. Priorytet: do ustalenia przez właściciela.
v0: `alfa-host` uruchamiany ręcznie / przy logowaniu, okno łączy się przez nazwany potok. v1: Linux (etap G).

## Kontrakt (szkic Rust)
```rust
// szkic — binarka; AppCore::build(paths, options) z shell: None (HeadlessShell) już istnieje
pub struct HostOptions { pub paths: AppPaths, pub autostart: bool, pub remote: bool }
pub enum HostState { Starting, Ready, Stopping, SafeMode }
pub trait HostControl {                     // kanał lokalny okno ↔ host
    fn state(&self) -> HostState;
    fn attach_local_client(&self, peer: PeerIdentity) -> Result<ClientSession, HostError>;
    fn request_stop(&self, reason: StopReason) -> Result<(), HostError>;
}
// fn main: platform() -> PlatformPorts → AppCore::build → brama klientów → pętla do sygnału stop
```
Zdarzenia: `host-headless.started`, `.ready`, `.stopping`, `.local_client.attached`.

## Zależności
`app-core` / `app-api` (kompozycja), `platform-select`, `remote-gateway-contract`, `platform-contract`
(nazwany potok z kontrolą tożsamości, zasobnik), `core-bus-contract`, `core-log-contract`.

## Niezmienniki
- **Żadnego nasłuchu sieciowego** — statyczna reguła obejmuje źródła `host-headless`; kanał lokalny to nazwany
  potok (na Linuksie gniazdo Unix), nigdy port TCP.
- Do potoku lokalnego łączy się tylko proces tego samego użytkownika o sprawdzonej tożsamości.
- Jednocześnie działa jeden host na profil użytkownika; drugi egzemplarz kończy się z czytelnym błędem.
- Kill-switch zatrzymuje hosta bez okna tak samo jak aplikację z oknem.
- Host bez okna nie wykonuje poleceń `local_only` i nie zatwierdza działań; zgody idą przez okno Brokera albo
  dowód urządzenia sparowanego (ADR 0022).
- Awaria okna Tauri nie kończy hosta; zamknięcie hosta zamyka sesje klientów w sposób jawny (zdarzenie).
- Ścieżki i zmienne środowiskowe wyłącznie przez `AppPaths` (zadanie B.8).

## Zdolności / uprawnienia
Takie same jak dzisiejszy proces aplikacji — host nie dostaje nic ponad to; dodatkowo `net.egress(<przekaźnik>)`
przez `remote-gateway`.

## Izolacja
`process` — osobny proces w sesji użytkownika (nie usługa w sesji 0), `always` po włączeniu autostartu,
inaczej `on-demand` (uruchamiany przez okno). Wątki RT audio — jak dziś w procesie hosta.

## Budżet zasobów
RAM hosta w tle bez okna i bez załadowanych modeli, czas startu do `Ready`, zużycie CPU w spoczynku — do
pomiaru na komputerze bazowym; progi ustala właściciel.

## Konfiguracja (klucze TOML)
Szkic: `[host] autostart = false`, `keep_running_after_window_close = false`, `tray = true`. Domyślnie
zachowanie jak dziś (host kończy się z oknem) — zmiana wymaga świadomego włączenia.

## Wkład do UI
Ustawienia: „Uruchamiaj Alfę w tle po zalogowaniu”, „Działaj po zamknięciu okna”. Zasobnik: ikona przez okno
Tauri uruchamiane na żądanie, pozycje „Otwórz”, „STOP”, „Zakończ”. Wskaźnik stanu hosta w oknie.

## Testy akceptacyjne
- `ACC-V6-host-headless-01` — `alfa-host` startuje bez okna i osiąga `Ready` na Windows; okno łączy się jako klient lokalny.
- `ACC-V6-host-headless-02` — zamknięcie i ponowne otwarcie okna przy trwającej odpowiedzi: żadne zdarzenie nie ginie.
- `ACC-V6-host-headless-03` — statyczna reguła „żadnego nasłuchu” obejmuje źródła modułu.
- `ACC-V6-host-headless-04` — proces innego użytkownika albo o niezgodnej tożsamości nie podłączy się do potoku.
- `ACC-V6-host-headless-05` — kill-switch zatrzymuje hosta bez okna w czasie poniżej progu (próg ustala właściciel).
- `ACC-V6-host-headless-06` — RAM hosta w spoczynku poniżej progu na komputerze bazowym (próg ustala właściciel).
- `ACC-V6-host-headless-07` — wyjście etapu D: drugi komputer łączy się przez przekaźnik, rozmawia i zatwierdza działanie.

## Fake
Host w procesie testu zbudowany z atrap (`PlatformPorts` z `platform-fake`, brama z `remote-gateway-fake`),
kanał lokalny w pamięci, sterowany cykl życia (start, awaria okna, stop). Osobny crate `-fake` — do ustalenia.

## Otwarte pytania
- ADR 0020 — czy potok lokalny jest osobnym transportem protokołu (ADR 0019), czy jest ukryty za poleceniami Tauri.
- Kto uruchamia i restartuje `alfa-host`: launcher, watchdog czy okno; przebieg aktualizacji wersji obok siebie
  (ADR 0007) — obszar `watchdog` / `updater`, wymaga przeglądu człowieka.
- Jak okno Brokera pokazuje prośbę, gdy nie ma okna głównego — do sprawdzenia z SPEC `broker-ui`.
- Plan określa moduł jako „binarkę”, a zasada 2 planu v6 wymaga trójki crate'ów — układ crate'ów do rozstrzygnięcia.
- Start hosta na Linuksie (usługa użytkownika) — etap G, poza v0.
