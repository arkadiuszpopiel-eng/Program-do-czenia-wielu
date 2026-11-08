# core-bus — SPEC (szkic v0)

## Cel
Magistrala zdarzeń jądra: publikacja i subskrypcja typowanych zdarzeń między modułami, UI i procesami potomnymi. Realizuje zasadę „wszystko jest zdarzeniem" (PLAN §2 pkt 4). Nie zawiera logiki domenowej i nie trwale zapisuje zdarzeń — to robi `core-log`.

## Fala i priorytet
F0 (kontrakt + implementacja + fake, pkt 2 w §4.5a). P0. Wzorzec trójki crate'ów dla wszystkich kolejnych modułów.

## Kontrakt (szkic Rust)
```rust
// core-bus-contract — SZKIC
pub struct EventId(pub Ulid);
pub struct Envelope {
    pub id: EventId, pub ts: Timestamp, pub session: Option<SessionId>,
    pub agent: Option<PersonaId>, pub run: Option<RunId>, pub span: Option<SpanId>,
    pub kind: EventKind,          // np. "voice.dialog.state_changed" (namespace = moduł)
    pub level: Level,             // Trace..Audit (PLAN §13)
    pub payload: PayloadRef,      // inline JSON (mały) albo referencja do core-log
    pub cost: Option<Cost>, pub schema_version: u16,
}
pub trait Bus: Send + Sync {
    fn publish(&self, ev: Envelope) -> Result<(), BusError>;
    fn subscribe(&self, filter: Filter) -> Subscription;   // strumień async, backpressure
    fn request(&self, cmd: Command, timeout: Duration) -> impl Future<Output = Result<Reply, BusError>>;
}
pub struct Filter { pub kinds: Vec<KindPattern>, pub session: Option<SessionId>, pub min_level: Level }
```
Zdarzenia własne: `bus.subscriber_lagged`, `bus.dropped` (przepełnienie), `bus.schema_rejected`.

## Zależności
Brak (jądro). Schematy zdarzeń: `packages/schemas/` (JSON Schema per `kind`, wersjonowane).

## Niezmienniki
- Zdarzenie raz opublikowane jest niezmienne; brak edycji, brak usuwania (append-only na poziomie log).
- Kolejność per `(session, run)` zachowana dla jednego wydawcy; globalna kolejność wg `ts` + `id` (ULID monotoniczny).
- Zdarzenie niezgodne ze schematem jest odrzucane w buildzie testowym (panika w testach) i logowane w produkcji.
- Wolny subskrybent nie blokuje wydawcy: bufor per subskrypcja, `subscriber_lagged` zamiast blokady; brak alokacji w ścieżce audio (audio używa własnych kolejek SPSC, publikuje na magistralę spoza callbacku RT).
- Brak przesyłania dużych danych: payload > limitu (wstępnie 64 KiB) idzie jako `PayloadRef` (ścieżka/klucz w `core-log`).

## Zdolności / uprawnienia
Brak tokenów Brokera. Magistrala nie wychodzi poza proces bez `core-registry` (mostkowanie do procesów potomnych przez stdio/pipe).

## Izolacja
`inproc`, `always`. Mostki do procesów potomnych i do UI (IPC Tauri, batch co klatkę — PLAN §14.7) są adapterami w `core-registry`/`ui-shell`, nie w magistrali.

## Budżet zasobów
RAM ≤ 2 MB przy 10 subskrypcjach; publikacja ≤ 10 µs p95 (in-proc); 0% CPU w bezczynności. Progi wstępne, do zaostrzenia po F0.

## Konfiguracja (klucze TOML)
`[core.bus] buffer_per_subscriber = 4096`, `max_inline_payload_bytes = 65536`, `strict_schema = false` (true w testach).

## Wkład do UI
Brak bezpośredni; Oś czasu v0 (F1) jest projekcją strumienia magistrali przez `core-log`.

## Testy akceptacyjne
- `ACC-F0-core-bus-01`: test kontraktowy `-impl` vs `-fake` (publish/subscribe/request, filtry, lag).
- `ACC-F0-core-bus-02`: property-based — kolejność per `(session, run)` zachowana przy 10k zdarzeń i 5 subskrybentach.
- `ACC-F0-core-bus-03`: brak alokacji/blokad w ścieżce RT (test z wirtualnym zegarem, `voice-audio-fake`).

## Fake
`core-bus-fake`: magistrala w pamięci z wirtualnym zegarem, deterministyczna kolejność, nagrywanie do NDJSON i odtwarzanie (replay) — podstawa testów wszystkich modułów.

## Otwarte pytania
- Typ transportu request/reply (kanały tokio vs własny) — do ustalenia w SPEC v1 po ADR (2).
- Czy `Envelope` niesie `hash_prev` (łańcuch), czy dopiero `core-log`/Broker liczy hash przy zapisie (PLAN §13 wskazuje zapis) — do ustalenia w SPEC v1.
