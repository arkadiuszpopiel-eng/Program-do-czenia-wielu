# remote-gateway — SPEC (szkic v0, planowany — plan v6)

## Cel
Brama klientów po stronie hosta: prowadzi sesje klientów (okno lokalne, urządzenia sparowane), sprawdza, które
polecenia wolno wywołać danemu klientowi, numeruje zdarzenia i obsługuje wznowienie po przerwie. Moduł
**nie** szyfruje kanału sam z siebie poza użyciem uzgodnionych kluczy, nie paruje urządzeń (`device-pairing`),
nie zatwierdza działań i nie wytwarza dowodów zgody — tylko je przenosi do Brokera.
Projekt D.0 zatwierdzony12.11.2026. Moduł wykonawczy nadal planowany; pierwszy przyrost
`lib-client-wire` dostarcza tylko czysty format ramki (schema Rust→TS), wersję, jawny limit bajtów
i rosnące ID. Bez sieci/ACL/parowania/dispatchera; [zakres biblioteki](../../../crates/lib-client-wire/README.md).

## Fala i priorytet
Plan v6, **etap D** (zadanie D.1), po B.1–B.3 i B.5 oraz po podpisie ADR 0019–0023 (D.0). Priorytet: do
ustalenia przez właściciela. v0: brama **wewnątrz** procesu aplikacji (krok 1 z ADR 0020). v1: brama w `alfa-host`.

## Kontrakt (szkic Rust)
```rust
// szkic — źródłem prawdy będzie crate remote-gateway-contract
pub struct ClientId(pub Uuid);
pub enum ClientKind { LocalWindow, PairedDevice(DeviceId) }
pub struct ClientSession { pub id: ClientId, pub kind: ClientKind, pub acl: CommandAcl, pub protocol: u16 }
pub enum Frame { Command { id: u64, cmd: String, args: Value }, Reply { id: u64, result: Result<Value, WireError> },
                 Events { seq: u64, events: Vec<Value> }, Resync }
pub struct WireError { pub code: String, pub message: String, pub details: Option<Value> }
pub trait ClientGateway {
    fn open(&self, kind: ClientKind, offered_versions: &[u16]) -> Result<ClientSession, GatewayError>;
    fn handle(&self, client: ClientId, frame: Frame) -> Result<(), GatewayError>;
    fn resume(&self, client: ClientId, after_seq: u64) -> Result<ResumeOutcome, GatewayError>; // Replay | Resync
    fn close(&self, client: ClientId, reason: CloseReason);
}
```
Zdarzenia: `remote-gateway.client.connected`, `.client.closed`, `.command.denied`, `.resync` (bez treści poleceń).

## Zależności
`app-api` (lista poleceń, zdarzenia, DTO), `device-pairing-contract` (lista urządzeń i klucze),
`safety-broker-contract` (przekazanie dowodu zgody), `core-bus-contract`, `core-log-contract`.
Zewnętrzne (wpis w `docs/vendor/` przed kodem): `snow` (wzorzec Noise), klient WebSocket po TLS.

## Niezmienniki
- **Żadnego nasłuchu sieciowego**: brak `TcpListener` / `UdpSocket` w źródłach; host tylko wychodzi do przekaźnika.
- Ramka nieuwierzytelniona jest odrzucana przed jakimkolwiek przetwarzaniem (R9).
- Polecenie spoza ACL klienta kończy się odmową po stronie hosta; polecenie `local_only` nigdy nie jest
  wykonywane dla klienta zdalnego.
- Numery `seq` rosną ściśle; po wznowieniu klient dostaje wszystkie zdarzenia po `after_seq` albo `resync` —
  nigdy lukę bez sygnału.
- Brama nie może wytworzyć ani zmienić dowodu zgody; Broker sprawdza go sam (ADR 0022).
- Sesja urządzenia odwołanego jest zamykana natychmiast; ramki z powtórzonym licznikiem są odrzucane (R2).
- Wersja protokołu spoza obsługiwanych (bieżąca i poprzednia, nie niższa niż wymuszone minimum) = odmowa połączenia.

## Zdolności / uprawnienia
`net.egress(<adres przekaźnika>)`. Brama sama nie dostaje zdolności narzędzi — polecenia klienta trafiają do
tych samych modułów i tego samego Brokera co polecenia z okna.

## Izolacja
v0: `inproc` w procesie aplikacji, `lazy` (startuje po sparowaniu pierwszego urządzenia i włączeniu dostępu).
v1: `inproc` w `alfa-host`, `always`. Bez wątku RT.

## Budżet zasobów
RAM (w tym bufor ostatnich N zdarzeń), CPU szyfrowania, opóźnienie polecenia przez przekaźnik — do pomiaru na
komputerze bazowym; progi i wartość N ustala właściciel.

## Konfiguracja (klucze TOML)
Szkic: `[remote] enabled = false`, `relay_url = "…"`, `event_buffer = <N>`, `min_protocol = 1`,
`max_frame_bytes = <limit>`. Wartości domyślne — do ustalenia; dostęp zdalny domyślnie wyłączony.

## Wkład do UI
Ustawienia → Urządzenia: włącznik dostępu zdalnego, stan połączenia z przekaźnikiem, lista aktywnych sesji.
W oknie rozmowy: znacznik, że polecenie lub zgoda przyszły z urządzenia zdalnego.

## Testy akceptacyjne
- `ACC-V6-remote-gateway-01` — klient bez uprawnienia do polecenia dostaje odmowę po stronie hosta.
- `ACC-V6-remote-gateway-02` — zerwanie i wznowienie połączenia przy trwającej odpowiedzi: żadne zdarzenie nie ginie.
- `ACC-V6-remote-gateway-03` — wznowienie spoza bufora daje `resync`, a klient odtwarza poprawny stan.
- `ACC-V6-remote-gateway-04` — statyczna reguła „żadnego nasłuchu” obejmuje źródła modułu.
- `ACC-V6-remote-gateway-05` — ramka nieuwierzytelniona lub powtórzona nie wywołuje polecenia (0 skutków; liczbę prób ustala właściciel).
- `ACC-V6-remote-gateway-06` — sesja urządzenia odwołanego zamknięta w czasie poniżej progu (próg ustala właściciel).
- `ACC-V6-remote-gateway-07` — klient z poprzednią wersją protokołu działa; z wersją poniżej minimum — odmowa.

## Fake
`remote-gateway-fake`: transport w pamięci (duplex) zamiast przekaźnika, wirtualny zegar, sterowane zrywanie
połączenia i gubienie ramek, klient testowy z dowolną ACL; służy też jako trzeci transport w testach interfejsu.

## Otwarte pytania
- ADR 0019 — rozmiar bufora zdarzeń, limit ramki, lista poleceń `local_only` i ich zdalnych odpowiedników.
- ADR 0021 — wzorzec Noise_XX_25519_ChaChaPoly_SHA256 zatwierdzony w D.0; czy kanał E2E żyje w tym module, czy w osobnej bibliotece
  `lib-*` wspólnej dla hosta i klientów.
- ADR 0022 — jak brama przekazuje `DeviceApprovalProof` do Brokera działającego w osobnym procesie.
- ADR 0020 — przeniesienie bramy z procesu aplikacji do `alfa-host` bez zrywania sparowanych urządzeń.
- Priorytet polecenia STOP (kill-switch zdalny) względem kolejki poleceń — do ustalenia w SPEC v1.
