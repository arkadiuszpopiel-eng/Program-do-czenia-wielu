# core-log — SPEC (szkic v0)

## Cel
Log-writer jądra: trwały zapis strumieni zdarzeń z magistrali (wywołania modeli, narzędzia i GUI, głos, diagnostyka) jako append-only NDJSON + indeks SQLite, z szyfrowaniem payloadów kluczem per sesja (crypto-shredding), retencją i limitami dysku (PLAN §13). Strumień **Audyt** docelowo pisze Broker (F3); do tego czasu `core-log` z oznaczeniem `pre-broker`.

## Fala i priorytet
F0 (kontrakt + fake), F1 (logi v1, Oś czasu v0), F3 (przekazanie Audytu Brokerowi). P0.

## Kontrakt (szkic Rust)
```rust
// core-log-contract — SZKIC
pub enum Stream { Audit, ModelCalls, ToolsGui, Voice, Diagnostics }
pub struct LogRecord { pub env: Envelope /* core-bus */, pub stream: Stream,
                       pub payload: EncryptedPayload, pub hash_prev: Option<Hash>, pub schema_version: u16 }
pub trait LogWriter: Send + Sync {
    fn append(&self, stream: Stream, ev: &Envelope) -> Result<PayloadRef, LogError>;
    fn query(&self, q: LogQuery) -> Result<Vec<LogRecord>, LogError>;   // po sesji/przebiegu/rodzaju/czasie
    fn shred_session(&self, session: SessionId) -> Result<(), LogError>;  // kasuje klucz payloadów
    fn diagnostics_bundle(&self, opts: BundleOpts) -> Result<PathBuf, LogError>;
}
pub struct Retention { pub tools_gui_days: u16 /* 7 */, pub disk_limit_mb: u32, pub redact_prompts: bool }
```
Zdarzenia: `log.rotated`, `log.disk_limit_reached`, `log.upcast_applied`, `log.audit_handover` (Broker przejmuje Audyt).

## Zależności
`core-bus-contract`, `core-config-contract`, `platform-windows-contract` (ścieżki, ACL). Baza: SQLite (ADR 8).

## Niezmienniki
- Append-only: brak API modyfikującego/usuwającego rekordy; usuwanie danych = crypto-shredding klucza sesji (łańcuch hashy pozostaje spójny).
- Wersjonowane schematy zdarzeń + upcastery; stary rekord czytany przez łańcuch upcasterów; testy migracji na fixture'ach każdej wersji.
- Redakcja sekretów (klucze, tokeny, pola `IsPassword`) przed zapisem — zawsze, niezależnie od `redact_prompts`.
- Deny-lista aplikacji/okien/URL wyklucza zrzuty i migawki UIA ze strumienia ToolsGui.
- Zapis nie blokuje wydawcy (kolejka + wątek zapisu); przy pełnym dysku: degradacja (najpierw ToolsGui, potem Voice), Audyt nigdy nie jest pomijany po cichu — błąd zapisu Audytu = zdarzenie Diagnostics + baner.

## Zdolności / uprawnienia
`fs.write(%LOCALAPPDATA%\Alfa\logs\**)` dla jądra. Po F3 pliki Audytu są zapisywalne tylko przez konto Brokera (ACL).

## Izolacja
`inproc`, `always`; osobny wątek I/O.

## Budżet zasobów
RAM ≤ 8 MB (bufory); zapis ≤ 1 ms p95 na rekord (bez fsync per rekord, grupowanie); limit dysku domyślnie 2 GB (do potwierdzenia w F1).

## Konfiguracja (klucze TOML)
`[logs] level = "info"`, `disk_limit_mb = 2048`, `retention.tools_gui_days = 7`, `redact_prompts = true`, `[logs.denylist] apps = [...], urls = [...]`.

## Wkład do UI
Dane dla Osi czasu/Replay, pulpitu kosztów/opóźnień, ekranu „co poszło do chmury", paczki diagnostycznej; strona Ustawienia → Logi i prywatność.

## Testy akceptacyjne
- `ACC-F0-core-log-01`: test kontraktowy `append/query/shred` na `-fake` i `-impl`.
- `ACC-F1-core-log-02`: upcaster v1→v2 na zamrożonych fixture'ach; 0 utraty pól.
- `ACC-F1-core-log-03`: po `shred_session` payloady nieczytelne, łańcuch hashy weryfikowalny.
- `ACC-F1-core-log-04`: redakcja — 0 sekretów z fixture'ów w plikach logów (test szpiegowski).

## Fake
`core-log-fake`: zapis do pamięci/tmp, deterministyczne id, bez szyfrowania (flaga), zliczanie rekordów per strumień — do asercji w testach innych modułów.

## Otwarte pytania
- Kotwiczenie głowy łańcucha hashy poza zasięgiem agentów (gdzie: Broker, TPM?) — ADR z THREAT_MODEL, do ustalenia w SPEC v1.
- Format indeksu (osobna baza `logs.db` vs baza per sesja) — do ustalenia po spike (i).
